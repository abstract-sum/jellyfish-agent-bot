use futures_util::StreamExt;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Value, json};

use jellyfish_core::{AppError, AppResult};

use crate::codex_auth::CodexCredentials;

const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";

pub async fn run_codex_request(
    credentials: &CodexCredentials,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    retrieval_context: &[String],
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| AppError::Runtime(format!("failed to build Codex HTTP client: {error}")))?;

    let mut instructions = system_prompt.to_string();
    if !retrieval_context.is_empty() {
        instructions.push_str("\n\nRetrieved context:\n");
        instructions.push_str(&retrieval_context.join("\n"));
    }

    let body = json!({
        "model": model,
        "store": false,
        "stream": true,
        "instructions": instructions,
        "input": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": user_input,
                    }
                ]
            }
        ]
    });

    let response = client
        .post(resolve_codex_url())
        .headers(build_headers(credentials)?)
        .json(&body)
        .send()
        .await
        .map_err(|error| AppError::Runtime(format!("failed to send Codex request: {error}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let detail = parse_error_message(&body).unwrap_or(body);
        return Err(AppError::Runtime(format!(
            "Codex request failed with {}: {}",
            status, detail
        )));
    }

    parse_sse_response(response).await
}

fn resolve_codex_url() -> String {
    format!("{}/codex/responses", DEFAULT_CODEX_BASE_URL)
}

fn build_headers(credentials: &CodexCredentials) -> AppResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", credentials.access_token))
            .map_err(|error| AppError::Config(format!("invalid authorization header: {error}")))?,
    );
    headers.insert(
        "chatgpt-account-id",
        HeaderValue::from_str(&credentials.account_id)
            .map_err(|error| AppError::Config(format!("invalid chatgpt-account-id header: {error}")))?,
    );
    headers.insert(
        "OpenAI-Beta",
        HeaderValue::from_static("responses=experimental"),
    );
    headers.insert("originator", HeaderValue::from_static("jellyfish"));
    headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

async fn parse_sse_response(response: reqwest::Response) -> AppResult<String> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut text = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|error| AppError::Runtime(format!("failed to read Codex SSE chunk: {error}")))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(index) = buffer.find("\n\n") {
            let frame = buffer[..index].to_string();
            buffer = buffer[index + 2..].to_string();

            for data in frame
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim)
                .filter(|line| !line.is_empty() && *line != "[DONE]")
            {
                let value: Value = serde_json::from_str(data)?;
                match value.get("type").and_then(Value::as_str).unwrap_or_default() {
                    "response.output_text.delta" => {
                        if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                            text.push_str(delta);
                        }
                    }
                    "response.completed" | "response.done" => {
                        if !text.trim().is_empty() {
                            return Ok(text.trim().to_string());
                        }
                    }
                    "response.failed" | "error" => {
                        let detail = parse_error_value(&value)
                            .unwrap_or_else(|| value.to_string());
                        return Err(AppError::Runtime(format!("Codex response failed: {detail}")));
                    }
                    _ => {}
                }
            }
        }
    }

    if text.trim().is_empty() {
        return Err(AppError::Runtime(
            "Codex SSE stream ended without assistant text".to_string(),
        ));
    }

    Ok(text.trim().to_string())
}

fn parse_error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(body).ok()?;
    parse_error_value(&value)
}

fn parse_error_value(value: &Value) -> Option<String> {
    value
        .get("error")
        .and_then(|error| error.get("message").or_else(|| Some(error)))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| value.get("message").and_then(Value::as_str).map(ToString::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_error_message() {
        let message = parse_error_message(r#"{"error":{"message":"denied"}}"#).unwrap();
        assert_eq!(message, "denied");
    }
}
