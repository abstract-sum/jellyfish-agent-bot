use futures_util::{SinkExt, StreamExt};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use rustls::crypto::{CryptoProvider, ring::default_provider};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest, tungstenite::protocol::Message};

use jellyfish_core::{AppError, AppResult, CodexTransport};

use crate::codex_auth::{CodexCredentials, refresh_codex_credentials, should_refresh};

const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexTransportUsed {
    Sse,
    Websocket,
}

impl CodexTransportUsed {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sse => "sse",
            Self::Websocket => "websocket",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexRunResult {
    pub message: String,
    pub transport: CodexTransportUsed,
    pub refreshed: bool,
}

pub async fn run_codex_request(
    credentials: &CodexCredentials,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    retrieval_context: &[String],
    transport: &CodexTransport,
) -> AppResult<CodexRunResult> {
    let (credentials, preemptively_refreshed) = if should_refresh(credentials) {
        (refresh_codex_credentials(credentials).await?, true)
    } else {
        (credentials.clone(), false)
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

    if matches!(transport, CodexTransport::Auto | CodexTransport::Websocket) {
        match run_codex_websocket_request(&credentials, &body).await {
            Ok(message) => {
                return Ok(CodexRunResult {
                    message,
                    transport: CodexTransportUsed::Websocket,
                    refreshed: preemptively_refreshed,
                })
            }
            Err(error) if matches!(transport, CodexTransport::Auto) => {
                tracing::warn!("Codex websocket transport failed, falling back to SSE: {}", error);
            }
            Err(error) => return Err(error),
        }
    }

    let (message, refreshed_on_retry) = run_codex_sse_request(&client, &credentials, &body).await?;
    Ok(CodexRunResult {
        message,
        transport: CodexTransportUsed::Sse,
        refreshed: preemptively_refreshed || refreshed_on_retry,
    })
}

pub async fn run_codex_text_request(
    credentials: &CodexCredentials,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    retrieval_context: &[String],
    transport: &CodexTransport,
) -> AppResult<String> {
    Ok(
        run_codex_request(
            credentials,
            model,
            system_prompt,
            user_input,
            retrieval_context,
            transport,
        )
        .await?
        .message,
    )
}

async fn run_codex_sse_request(
    client: &reqwest::Client,
    credentials: &CodexCredentials,
    body: &Value,
) -> AppResult<(String, bool)> {
    let response = send_request(client, credentials, body).await?;

    match response.status() {
        status if status.is_success() => Ok((parse_sse_response(response).await?, false)),
        reqwest::StatusCode::UNAUTHORIZED => {
            let refreshed = refresh_codex_credentials(credentials).await?;
            let retry_response = send_request(client, &refreshed, body).await?;
            if !retry_response.status().is_success() {
                return Err(build_http_error(retry_response).await);
            }
            Ok((parse_sse_response(retry_response).await?, true))
        }
        _ => Err(build_http_error(response).await),
    }
}

async fn run_codex_websocket_request(
    credentials: &CodexCredentials,
    body: &Value,
) -> AppResult<String> {
    ensure_rustls_provider();
    let url = resolve_codex_websocket_url();
    let mut request = url
        .into_client_request()
        .map_err(|error| AppError::Runtime(format!("failed to build Codex websocket request: {error}")))?;

    {
        let headers = request.headers_mut();
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
            HeaderValue::from_static("responses_websockets=2026-02-06"),
        );
        headers.insert("originator", HeaderValue::from_static("jellyfish"));
    }

    let (mut socket, _) = connect_async(request)
        .await
        .map_err(|error| AppError::Runtime(format!("failed to connect Codex websocket: {error}")))?;

    let message = json!({
        "type": "response.create",
        "model": body.get("model").cloned().unwrap_or(Value::Null),
        "store": body.get("store").cloned().unwrap_or(Value::Bool(false)),
        "stream": body.get("stream").cloned().unwrap_or(Value::Bool(true)),
        "instructions": body.get("instructions").cloned().unwrap_or(Value::Null),
        "input": body.get("input").cloned().unwrap_or(Value::Null),
    });

    socket
        .send(Message::Text(message.to_string().into()))
        .await
        .map_err(|error| AppError::Runtime(format!("failed to send Codex websocket request: {error}")))?;

    let mut text = String::new();
    while let Some(frame) = socket.next().await {
        let frame = frame
            .map_err(|error| AppError::Runtime(format!("failed to receive Codex websocket frame: {error}")))?;
        match frame {
            Message::Text(payload) => {
                if let Some(done) = parse_sse_frame_like(&payload, &mut text)? {
                    if done {
                        return Ok(text.trim().to_string());
                    }
                }
            }
            Message::Binary(payload) => {
                let payload = String::from_utf8_lossy(&payload);
                if let Some(done) = parse_sse_frame_like(&payload, &mut text)? {
                    if done {
                        return Ok(text.trim().to_string());
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    if text.trim().is_empty() {
        return Err(AppError::Runtime(
            "Codex websocket stream ended without assistant text".to_string(),
        ));
    }

    Ok(text.trim().to_string())
}

fn ensure_rustls_provider() {
    let _ = CryptoProvider::get_default()
        .or_else(|| default_provider().install_default().ok().and_then(|_| CryptoProvider::get_default()));
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

fn resolve_codex_websocket_url() -> String {
    resolve_codex_url().replace("https://", "wss://")
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

fn parse_sse_frame_like(frame: &str, text: &mut String) -> AppResult<Option<bool>> {
    let value: Value = serde_json::from_str(frame)?;
    match value.get("type").and_then(Value::as_str).unwrap_or_default() {
        "response.output_text.delta" => {
            if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                text.push_str(delta);
            }
            Ok(None)
        }
        "response.completed" | "response.done" => {
            if !text.trim().is_empty() {
                Ok(Some(true))
            } else {
                Ok(None)
            }
        }
        "response.failed" | "error" => {
            let detail = parse_error_value(&value).unwrap_or_else(|| value.to_string());
            Err(AppError::Runtime(format!("Codex response failed: {detail}")))
        }
        _ => Ok(None),
    }
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

    #[test]
    fn parses_websocket_json_events_into_final_message() {
        let mut text = String::new();
        let delta = r#"{"type":"response.output_text.delta","delta":"Hello ws"}"#;
        assert_eq!(parse_sse_frame_like(delta, &mut text).unwrap(), None);
        let done = r#"{"type":"response.completed"}"#;
        assert_eq!(parse_sse_frame_like(done, &mut text).unwrap(), Some(true));
        assert_eq!(text, "Hello ws");
    }
}
