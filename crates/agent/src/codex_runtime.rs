use futures_util::StreamExt;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Value, json};

use jellyfish_core::{AppError, AppResult};

use crate::codex_auth::{CodexCredentials, refresh_codex_credentials, should_refresh};

const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";

pub async fn run_codex_request(
    credentials: &CodexCredentials,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    retrieval_context: &[String],
) -> AppResult<String> {
    let credentials = if should_refresh(credentials) {
        refresh_codex_credentials(credentials).await?
    } else {
        credentials.clone()
    };

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

    let response = send_request(&client, &credentials, &body).await?;

    match response.status() {
        status if status.is_success() => parse_sse_response(response).await,
        reqwest::StatusCode::UNAUTHORIZED => {
            let refreshed = refresh_codex_credentials(&credentials).await?;
            let retry_response = send_request(&client, &refreshed, &body).await?;
            if !retry_response.status().is_success() {
                return Err(build_http_error(retry_response).await);
            }
            parse_sse_response(retry_response).await
        }
        _ => Err(build_http_error(response).await),
    }
}

async fn send_request(
    client: &reqwest::Client,
    credentials: &CodexCredentials,
    body: &Value,
) -> AppResult<reqwest::Response> {
    client
        .post(resolve_codex_url())
        .headers(build_headers(credentials)?)
        .json(body)
        .send()
        .await
        .map_err(|error| AppError::Runtime(format!("failed to send Codex request: {error}")))
}

async fn build_http_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let detail = parse_error_message(&body).unwrap_or(body);
    AppError::Runtime(format!("Codex request failed with {}: {}", status, detail))
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

            if let Some(done) = parse_sse_frame(&frame, &mut text)? {
                if done {
                    return Ok(text.trim().to_string());
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

fn parse_sse_frame(frame: &str, text: &mut String) -> AppResult<Option<bool>> {
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
                    return Ok(Some(true));
                }
            }
            "response.failed" | "error" => {
                let detail = parse_error_value(&value).unwrap_or_else(|| value.to_string());
                return Err(AppError::Runtime(format!("Codex response failed: {detail}")));
            }
            _ => {}
        }
    }

    Ok(None)
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

    #[test]
    fn parses_output_text_deltas_into_final_message() {
        let mut text = String::new();

        let first = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\" world\"}\n\n"
        );
        assert_eq!(parse_sse_frame(first, &mut text).unwrap(), None);

        let done = "data: {\"type\":\"response.completed\"}\n\n";
        assert_eq!(parse_sse_frame(done, &mut text).unwrap(), Some(true));
        assert_eq!(text, "Hello world");
    }
}
