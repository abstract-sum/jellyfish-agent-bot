use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use jellyfish_schema::OutboundMessage;
use reqwest::Client;
use serde_json::json;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::FeishuPluginConfig;

const OUTBOUND_IDEMPOTENCY_TTL_SECS: u64 = 30 * 60;
const OUTBOUND_IDEMPOTENCY_MAX_SIZE: usize = 1_000;
const BOT_INFO_CACHE_TTL_SECS: u64 = 30 * 60;

pub const BOT_INFO_REFRESH_INTERVAL_SECS: u64 = 30 * 60;

static OUTBOUND_IDEMPOTENCY: OnceLock<Mutex<OutboundIdempotencyStore>> = OnceLock::new();
static BOT_INFO_CACHE: OnceLock<Mutex<BotInfoCache>> = OnceLock::new();

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct OutboundIdempotencyFile {
    entries: HashMap<String, u64>,
}

#[derive(Debug, Default)]
struct OutboundIdempotencyStore {
    path: PathBuf,
    entries: HashMap<String, u64>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct BotInfoCacheFile {
    entries: HashMap<String, BotInfoCacheEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BotInfoCacheEntry {
    open_id: String,
    updated_at: u64,
}

#[derive(Debug, Default)]
struct BotInfoCache {
    path: PathBuf,
    entries: HashMap<String, BotInfoCacheEntry>,
}

impl OutboundIdempotencyStore {
    fn load(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                entries: HashMap::new(),
            });
        }

        let content = fs::read_to_string(&path)?;
        let file: OutboundIdempotencyFile = serde_json::from_str(&content)?;
        Ok(Self {
            path,
            entries: file.entries,
        })
    }

    fn should_send(&mut self, account_id: &str, chat_id: &str, reply_to: Option<&str>, text: &str) -> Result<bool> {
        let now = unix_timestamp();
        self.entries
            .retain(|_, timestamp| now.saturating_sub(*timestamp) < OUTBOUND_IDEMPOTENCY_TTL_SECS);

        let key = format!(
            "{}:{}:{}:{}",
            account_id,
            chat_id,
            reply_to.unwrap_or("-"),
            text
        );

        if self.entries.contains_key(&key) {
            return Ok(false);
        }

        while self.entries.len() >= OUTBOUND_IDEMPOTENCY_MAX_SIZE {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, timestamp)| *timestamp)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest_key);
            } else {
                break;
            }
        }

        self.entries.insert(key, now);
        self.persist()?;
        Ok(true)
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OutboundIdempotencyFile {
            entries: self.entries.clone(),
        };
        fs::write(&self.path, serde_json::to_string_pretty(&file)?)?;
        Ok(())
    }
}

impl BotInfoCache {
    fn load(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                entries: HashMap::new(),
            });
        }

        let content = fs::read_to_string(&path)?;
        let file: BotInfoCacheFile = serde_json::from_str(&content)?;
        Ok(Self {
            path,
            entries: file.entries,
        })
    }

    fn get_fresh(&mut self, account_id: &str) -> Result<Option<String>> {
        let now = unix_timestamp();
        self.entries
            .retain(|_, entry| now.saturating_sub(entry.updated_at) < BOT_INFO_CACHE_TTL_SECS);
        self.persist()?;
        Ok(self.entries.get(account_id).map(|entry| entry.open_id.clone()))
    }

    fn set(&mut self, account_id: &str, open_id: String) -> Result<()> {
        self.entries.insert(
            account_id.to_string(),
            BotInfoCacheEntry {
                open_id,
                updated_at: unix_timestamp(),
            },
        );
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = BotInfoCacheFile {
            entries: self.entries.clone(),
        };
        fs::write(&self.path, serde_json::to_string_pretty(&file)?)?;
        Ok(())
    }
}

fn outbound_idempotency_store() -> &'static Mutex<OutboundIdempotencyStore> {
    OUTBOUND_IDEMPOTENCY.get_or_init(|| {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".jellyfish")
            .join("feishu-outbound-dedup.json");
        Mutex::new(OutboundIdempotencyStore::load(path).unwrap_or_default())
    })
}

fn bot_info_cache() -> &'static Mutex<BotInfoCache> {
    BOT_INFO_CACHE.get_or_init(|| {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".jellyfish")
            .join("feishu-bot-info.json");
        Mutex::new(BotInfoCache::load(path).unwrap_or_default())
    })
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

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

pub async fn send_text(config: &FeishuPluginConfig, message: &OutboundMessage) -> Result<()> {
    let chat_id = &message.peer.id;
    let text = &message.text;
    let should_send = {
        let mut store = outbound_idempotency_store().lock().await;
        store.should_send(
            &message.account_id,
            chat_id,
            message.reply_to_message_id.as_deref(),
            text,
        )?
    };
    if !should_send {
        info!(chat_id = %chat_id, text = %text, "Feishu outbound message suppressed by idempotency store");
        return Ok(());
    }

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

pub async fn fetch_bot_open_id(config: &FeishuPluginConfig) -> Result<String> {
    if let Some(open_id) = {
        let mut cache = bot_info_cache().lock().await;
        cache.get_fresh(&config.default_account)?
    } {
        info!(account = %config.default_account, open_id = %open_id, "Feishu bot open_id loaded from cache");
        return Ok(open_id);
    }

    fetch_bot_open_id_force(config).await
}

pub async fn fetch_bot_open_id_force(config: &FeishuPluginConfig) -> Result<String> {
    let open_id = fetch_bot_open_id_from_api(config).await?;

    {
        let mut cache = bot_info_cache().lock().await;
        cache.set(&config.default_account, open_id.clone())?;
    }

    info!(account = %config.default_account, open_id = %open_id, "Feishu bot open_id fetched from API");
    Ok(open_id)
}

async fn fetch_bot_open_id_from_api(config: &FeishuPluginConfig) -> Result<String> {
    let token = fetch_tenant_access_token(config).await?;
    let response = Client::new()
        .get(format!(
            "{}/open-apis/bot/v3/info",
            config.domain.open_base_url()
        ))
        .bearer_auth(token)
        .send()
        .await?;

    let status = response.status();
    let value: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow!("failed to fetch Feishu bot info with {}: {}", status, value));
    }

    let open_id = value
        .get("bot")
        .and_then(|bot| bot.get("open_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing bot.open_id in Feishu bot info response"))?;

    Ok(open_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn outbound_idempotency_blocks_duplicate_reply() {
        let path = temp_store_path("jellyfish-feishu-outbound-idempotency.json");
        let _ = fs::remove_file(&path);
        let mut store = OutboundIdempotencyStore::load(path.clone()).unwrap();

        assert!(store.should_send("main", "oc_chat", Some("om_1"), "hello").unwrap());
        assert!(!store.should_send("main", "oc_chat", Some("om_1"), "hello").unwrap());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn bot_info_cache_returns_fresh_entry() {
        let path = temp_store_path("jellyfish-feishu-bot-info.json");
        let _ = fs::remove_file(&path);
        let mut cache = BotInfoCache::load(path.clone()).unwrap();

        cache.set("main", "ou_bot".to_string()).unwrap();
        assert_eq!(cache.get_fresh("main").unwrap().as_deref(), Some("ou_bot"));

        let _ = fs::remove_file(path);
    }
}
