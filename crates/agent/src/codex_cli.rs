use std::path::Path;
use std::process::Command;

use serde_json::Value;

use jellyfish_core::{AppError, AppResult};

pub fn codex_cli_available() -> bool {
    Command::new("codex")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn codex_auth_cache_exists() -> bool {
    crate::codex_auth::auth_file_path()
        .map(|path| path.exists())
        .unwrap_or(false)
}

pub fn run_codex_exec(model: &str, prompt: &str, workspace_root: &Path) -> AppResult<String> {
    let output = Command::new("codex")
        .arg("exec")
        .arg("--json")
        .arg("--color")
        .arg("never")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--skip-git-repo-check")
        .arg("--model")
        .arg(model)
        .arg(prompt)
        .current_dir(workspace_root)
        .output()
        .map_err(|error| AppError::Runtime(format!("failed to launch codex CLI: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("codex CLI exited with status {}", output.status)
        };
        return Err(AppError::Runtime(message));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_codex_jsonl(&stdout)
}

fn parse_codex_jsonl(stdout: &str) -> AppResult<String> {
    let mut last_text = None;

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if let Some(text) = extract_text(&value) {
            if !text.trim().is_empty() {
                last_text = Some(text);
            }
        }
    }

    last_text.ok_or_else(|| {
        AppError::Runtime("codex CLI returned no assistant text in JSON output".to_string())
    })
}

fn extract_text(value: &Value) -> Option<String> {
    if let Some(msg_type) = value.get("type").and_then(Value::as_str) {
        if matches!(msg_type, "assistant" | "message" | "response.output_text") {
            if let Some(text) = value.get("text").and_then(Value::as_str) {
                return Some(text.to_string());
            }
            if let Some(text) = value.get("content").and_then(Value::as_str) {
                return Some(text.to_string());
            }
        }
    }

    if let Some(text) = value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
    {
        return Some(text.to_string());
    }

    if let Some(text) = value
        .get("delta")
        .and_then(|delta| delta.get("text"))
        .and_then(Value::as_str)
    {
        return Some(text.to_string());
    }

    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_last_assistant_text_from_jsonl() {
        let stdout = r#"{"type":"assistant","text":"first"}
{"type":"assistant","text":"second"}
"#;

        let parsed = parse_codex_jsonl(stdout).unwrap();
        assert_eq!(parsed, "second");
    }

    #[test]
    fn parses_nested_message_text() {
        let stdout = r#"{"message":{"content":"hello"}}"#;
        let parsed = parse_codex_jsonl(stdout).unwrap();
        assert_eq!(parsed, "hello");
    }
}
