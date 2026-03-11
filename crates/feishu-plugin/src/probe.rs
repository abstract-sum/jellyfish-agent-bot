use anyhow::{Result, anyhow};

use crate::config::FeishuPluginConfig;
use crate::send::fetch_tenant_access_token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeishuProbeResult {
    pub domain: String,
    pub connection_mode: &'static str,
    pub account_id: String,
    pub app_id_prefix: String,
}

pub async fn probe_feishu(config: &FeishuPluginConfig) -> Result<FeishuProbeResult> {
    if !config.enabled {
        return Err(anyhow!("Feishu plugin is disabled"));
    }
    let _ = fetch_tenant_access_token(config).await?;

    Ok(FeishuProbeResult {
        domain: config.domain.open_base_url().to_string(),
        connection_mode: match config.connection_mode {
            crate::config::FeishuConnectionMode::Websocket => "websocket",
            crate::config::FeishuConnectionMode::Webhook => "webhook",
        },
        account_id: config.default_account.clone(),
        app_id_prefix: config.account.app_id.chars().take(8).collect(),
    })
}
