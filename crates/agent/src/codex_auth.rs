use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use jellyfish_core::{AppError, AppResult};

const OPENAI_AUTH_CLAIM: &str = "https://api.openai.com/auth";
const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub account_id: String,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CodexAuthFile {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    tokens: Option<CodexTokens>,
    #[allow(dead_code)]
    last_refresh: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
    expires_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

pub fn load_bearer_token() -> AppResult<Option<String>> {
    if let Some(api_key) = env::var("OPENAI_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(Some(api_key));
    }

    Ok(load_codex_credentials()?.map(|credentials| credentials.access_token))
}

pub fn load_codex_credentials() -> AppResult<Option<CodexCredentials>> {
    let path = auth_file_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    let auth_file: CodexAuthFile = serde_json::from_str(&content)?;

    if let Some(api_key) = auth_file
        .openai_api_key
        .filter(|value| !value.trim().is_empty())
    {
        let expires_at = extract_expiry(&api_key).ok();
        return Ok(Some(CodexCredentials {
            access_token: api_key,
            refresh_token: None,
            account_id: String::new(),
            expires_at,
        }));
    }

    let Some(tokens) = auth_file.tokens else {
        return Ok(None);
    };
    let Some(access_token) = tokens.access_token.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };

    let account_id = tokens
        .account_id
        .filter(|value| !value.trim().is_empty())
        .or_else(|| extract_account_id(&access_token).ok())
        .ok_or_else(|| {
            AppError::Config("failed to extract Codex account_id from auth cache".to_string())
        })?;
    let expires_at = tokens.expires_at.or_else(|| extract_expiry(&access_token).ok());

    Ok(Some(CodexCredentials {
        access_token,
        refresh_token: tokens
            .refresh_token
            .filter(|value| !value.trim().is_empty()),
        account_id,
        expires_at,
    }))
}

pub fn should_refresh(credentials: &CodexCredentials) -> bool {
    let Some(expires_at) = credentials.expires_at else {
        return false;
    };
    expires_at <= unix_timestamp() + 60
}

pub async fn refresh_codex_credentials(
    credentials: &CodexCredentials,
) -> AppResult<CodexCredentials> {
    let refresh_token = credentials
        .refresh_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Config("codex credentials do not contain a refresh token".to_string()))?;

    let client = Client::new();
    let response = client
        .post(OPENAI_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", OPENAI_CODEX_CLIENT_ID),
        ])
        .send()
        .await
        .map_err(|error| AppError::Runtime(format!("failed to refresh Codex token: {error}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Runtime(format!(
            "failed to refresh Codex token with {}: {}",
            status, body
        )));
    }

    let refreshed: RefreshResponse = response
        .json()
        .await
        .map_err(|error| AppError::Runtime(format!("failed to parse Codex refresh response: {error}")))?;
    let account_id = extract_account_id(&refreshed.access_token)?;

    let refreshed_credentials = CodexCredentials {
        access_token: refreshed.access_token,
        refresh_token: Some(refreshed.refresh_token),
        account_id,
        expires_at: Some(unix_timestamp() + refreshed.expires_in),
    };
    persist_refreshed_credentials(&refreshed_credentials)?;
    Ok(refreshed_credentials)
}

pub fn extract_account_id(token: &str) -> AppResult<String> {
    let payload = decode_jwt_payload(token)?;
    payload
        .get(OPENAI_AUTH_CLAIM)
        .and_then(|value| value.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| AppError::Config("failed to extract account_id from token".to_string()))
}

fn decode_jwt_payload(token: &str) -> AppResult<Value> {
    let mut parts = token.split('.');
    let _header = parts.next();
    let payload = parts
        .next()
        .ok_or_else(|| AppError::Config("invalid JWT payload".to_string()))?;
    let normalized = format!("{}{}", payload, "=".repeat((4 - payload.len() % 4) % 4));
    let decoded = decode_base64_urlsafe(&normalized)?;
    serde_json::from_slice(&decoded).map_err(AppError::from)
}

fn extract_expiry(token: &str) -> AppResult<u64> {
    decode_jwt_payload(token)?
        .get("exp")
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::Config("failed to extract token expiry".to_string()))
}

fn decode_base64_urlsafe(input: &str) -> AppResult<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut acc: u32 = 0;
    let mut bits: u8 = 0;

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => break,
            _ => {
                return Err(AppError::Config(
                    "invalid base64url character in token".to_string(),
                ))
            }
        } as u32;

        acc = (acc << 6) | value;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            buffer.push(((acc >> bits) & 0xFF) as u8);
        }
    }

    Ok(buffer)
}

pub fn auth_file_path() -> AppResult<PathBuf> {
    let home = env::var("HOME").map_err(|_| {
        AppError::Config("HOME is not set; cannot locate ~/.codex/auth.json".to_string())
    })?;
    Ok(PathBuf::from(home).join(".codex").join("auth.json"))
}

fn persist_refreshed_credentials(credentials: &CodexCredentials) -> AppResult<()> {
    let path = auth_file_path()?;
    let mut value = if path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?
    } else {
        Value::Object(Default::default())
    };

    let object = value
        .as_object_mut()
        .ok_or_else(|| AppError::Config("invalid codex auth cache structure".to_string()))?;
    let tokens = object
        .entry("tokens")
        .or_insert_with(|| Value::Object(Default::default()));
    let tokens_object = tokens
        .as_object_mut()
        .ok_or_else(|| AppError::Config("invalid codex tokens structure".to_string()))?;

    tokens_object.insert(
        "access_token".to_string(),
        Value::String(credentials.access_token.clone()),
    );
    if let Some(refresh_token) = &credentials.refresh_token {
        tokens_object.insert(
            "refresh_token".to_string(),
            Value::String(refresh_token.clone()),
        );
    }
    tokens_object.insert(
        "account_id".to_string(),
        Value::String(credentials.account_id.clone()),
    );
    if let Some(expires_at) = credentials.expires_at {
        tokens_object.insert(
            "expires_at".to_string(),
            Value::Number(serde_json::Number::from(expires_at)),
        );
    }
    object.insert(
        "last_refresh".to_string(),
        Value::Number(serde_json::Number::from(unix_timestamp())),
    );

    fs::write(path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_file_path_uses_home() {
        let path = auth_file_path();
        assert!(path.is_ok());
        assert!(path.unwrap().ends_with(".codex/auth.json"));
    }

    #[test]
    fn decodes_base64_urlsafe_payload() {
        let payload = decode_base64_urlsafe("eyJmb28iOiJiYXIifQ==").unwrap();
        assert_eq!(String::from_utf8(payload).unwrap(), r#"{"foo":"bar"}"#);
    }

    #[test]
    fn extracts_account_id_from_access_token_claim() {
        let token = concat!(
            "eyJhbGciOiJub25lIn0.",
            "eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjdF8xMjMifX0.",
            "signature"
        );

        let account_id = extract_account_id(token).unwrap();
        assert_eq!(account_id, "acct_123");
    }
}
