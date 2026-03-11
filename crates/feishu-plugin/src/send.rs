use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::json;
use tracing::{info, warn};

use crate::config::FeishuPluginConfig;

pub async fn fetch_tenant_access_token(config: &FeishuPluginConfig) -> Result<String> {
    let response = Client::new()
        .post(format!(
            "{}/open-apis/auth/v3/tenant_access_token/internal",
            config.domain.open_base_url()
        ))
        .json(&json!({
            "app_id": config.account.app_id,
            "app_secret": config.account.app_secret,
        }))
        .send()
        .await?;

    let value: serde_json::Value = response.json().await?;
    value
        .get("tenant_access_token")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing tenant_access_token in Feishu response"))
}

pub async fn send_text(config: &FeishuPluginConfig, chat_id: &str, text: &str) -> Result<()> {
    let token = fetch_tenant_access_token(config).await?;
    let response = Client::new()
        .post(format!(
            "{}/open-apis/im/v1/messages?receive_id_type=chat_id",
            config.domain.open_base_url()
        ))
        .bearer_auth(token)
        .json(&json!({
            "receive_id": chat_id,
            "msg_type": "text",
            "content": serde_json::to_string(&json!({"text": text}))?,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        warn!(chat_id = %chat_id, status = %status, body = %body, "Feishu send failed");
        return Err(anyhow!("Feishu send failed with {}: {}", status, body));
    }

    info!(chat_id = %chat_id, text = %text, "Feishu outbound message sent");
    Ok(())
}
