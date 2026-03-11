use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeishuDomain {
    Feishu,
    Lark,
}

impl FeishuDomain {
    pub fn open_base_url(&self) -> &'static str {
        match self {
            Self::Feishu => "https://open.feishu.cn",
            Self::Lark => "https://open.larksuite.com",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeishuConnectionMode {
    Websocket,
    Webhook,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeishuAccountConfig {
    pub enabled: bool,
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeishuPluginConfig {
    pub enabled: bool,
    pub domain: FeishuDomain,
    pub connection_mode: FeishuConnectionMode,
    pub default_account: String,
    pub require_mention: bool,
    pub account: FeishuAccountConfig,
}

impl FeishuPluginConfig {
    pub fn from_value(value: &Value) -> Result<Self> {
        let enabled = value
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let domain = match value
            .get("domain")
            .and_then(Value::as_str)
            .unwrap_or("feishu")
        {
            "feishu" => FeishuDomain::Feishu,
            "lark" => FeishuDomain::Lark,
            other => bail!("unsupported Feishu domain: {other}"),
        };
        let connection_mode = match value
            .get("connection_mode")
            .or_else(|| value.get("connectionMode"))
            .and_then(Value::as_str)
            .unwrap_or("websocket")
        {
            "websocket" => FeishuConnectionMode::Websocket,
            "webhook" => FeishuConnectionMode::Webhook,
            other => bail!("unsupported Feishu connection mode: {other}"),
        };
        let default_account = value
            .get("default_account")
            .or_else(|| value.get("defaultAccount"))
            .and_then(Value::as_str)
            .unwrap_or("main")
            .to_string();
        let require_mention = value
            .get("require_mention")
            .or_else(|| value.get("requireMention"))
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let account_value = value
            .get("accounts")
            .and_then(|accounts| accounts.get(&default_account))
            .or_else(|| value.get("account"))
            .ok_or_else(|| {
                anyhow::anyhow!("missing Feishu account config for '{default_account}'")
            })?;

        let app_id = account_value
            .get("app_id")
            .or_else(|| account_value.get("appId"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing Feishu app_id/appId"))?
            .to_string();
        let app_secret = account_value
            .get("app_secret")
            .or_else(|| account_value.get("appSecret"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing Feishu app_secret/appSecret"))?
            .to_string();

        Ok(Self {
            enabled,
            domain,
            connection_mode,
            default_account,
            require_mention,
            account: FeishuAccountConfig {
                enabled: account_value
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                app_id,
                app_secret,
            },
        })
    }

    pub fn from_env() -> Result<Self> {
        let app_id = env::var("FEISHU_APP_ID")
            .or_else(|_| env::var("LARK_APP_ID"))
            .map_err(|_| anyhow::anyhow!("missing FEISHU_APP_ID or LARK_APP_ID"))?;
        let app_secret = env::var("FEISHU_APP_SECRET")
            .or_else(|_| env::var("LARK_APP_SECRET"))
            .map_err(|_| anyhow::anyhow!("missing FEISHU_APP_SECRET or LARK_APP_SECRET"))?;
        let domain = match env::var("FEISHU_DOMAIN")
            .or_else(|_| env::var("LARK_DOMAIN"))
            .unwrap_or_else(|_| "feishu".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "feishu" => FeishuDomain::Feishu,
            "lark" => FeishuDomain::Lark,
            other => bail!("unsupported Feishu domain from env: {other}"),
        };
        let connection_mode = match env::var("FEISHU_CONNECTION_MODE")
            .unwrap_or_else(|_| "websocket".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "websocket" => FeishuConnectionMode::Websocket,
            "webhook" => FeishuConnectionMode::Webhook,
            other => bail!("unsupported Feishu connection mode from env: {other}"),
        };

        Ok(Self {
            enabled: true,
            domain,
            connection_mode,
            default_account: "main".to_string(),
            require_mention: env::var("FEISHU_REQUIRE_MENTION")
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(true),
            account: FeishuAccountConfig {
                enabled: true,
                app_id,
                app_secret,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_feishu_and_lark_domains() {
        let config = FeishuPluginConfig::from_value(&json!({
            "domain": "lark",
            "accounts": {"main": {"appId": "cli_xxx", "appSecret": "secret"}}
        }))
        .unwrap();
        assert_eq!(config.domain, FeishuDomain::Lark);
    }

    #[test]
    fn rejects_missing_app_id() {
        assert!(FeishuPluginConfig::from_value(&json!({
            "accounts": {"main": {"appSecret": "secret"}}
        }))
        .is_err());
    }

    #[test]
    fn rejects_missing_app_secret() {
        assert!(FeishuPluginConfig::from_value(&json!({
            "accounts": {"main": {"appId": "cli_xxx"}}
        }))
        .is_err());
    }
}
