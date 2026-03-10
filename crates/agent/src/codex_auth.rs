use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use jellyfish_core::{AppError, AppResult};

const OPENAI_AUTH_CLAIM: &str = "https://api.openai.com/auth";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub account_id: String,
}

#[derive(Debug, Deserialize)]
struct CodexAuthFile {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    tokens: Option<CodexTokens>,
}

#[derive(Debug, Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
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
        return Ok(Some(CodexCredentials {
            access_token: api_key,
            refresh_token: None,
            account_id: String::new(),
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

    Ok(Some(CodexCredentials {
        access_token,
        refresh_token: tokens
            .refresh_token
            .filter(|value| !value.trim().is_empty()),
        account_id,
    }))
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
}
