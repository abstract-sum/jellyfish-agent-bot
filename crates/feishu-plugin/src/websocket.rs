use std::pin::Pin;
use std::sync::Arc;
use std::collections::HashMap;

use anyhow::{Result, anyhow};
use feishu_sdk::core::{Config, LogLevel, new_logger};
use feishu_sdk::event::{Event, EventDispatcher, EventDispatcherConfig, EventHandler, EventHandlerResult};
use feishu_sdk::ws::StreamClient;
use jellyfish_gateway::GatewayService;
use rustls::crypto::{CryptoProvider, ring::default_provider};
use tokio::sync::Mutex;
use tracing::info;

use crate::config::{FeishuConnectionMode, FeishuPluginConfig};
use crate::dedup::{DedupStore, default_dedup_path};
use crate::plugin::FeishuPluginRuntime;
use crate::types::FeishuEventEnvelope;

pub async fn start_websocket_listener(
    config: &FeishuPluginConfig,
    gateway: Arc<dyn GatewayService>,
    bot_open_id: Option<String>,
    dry_run: bool,
) -> Result<()> {
    ensure_websocket_mode(config)?;
    ensure_rustls_provider();

    let logger = new_logger(LogLevel::Info);
    let dispatcher = EventDispatcher::new(EventDispatcherConfig::new(), logger.clone());
    dispatcher
        .register_handler(Box::new(FeishuMessageHandler {
            config: config.clone(),
            gateway,
            bot_open_id,
            dry_run,
            dedup_store: Arc::new(Mutex::new(DedupStore::load(default_dedup_path()?)?)),
            chat_locks: Arc::new(Mutex::new(HashMap::new())),
        }))
        .await;

    let sdk_config = Config::builder(&config.account.app_id, &config.account.app_secret)
        .base_url(config.domain.open_base_url())
        .build();

    info!(
        domain = %config.domain.open_base_url(),
        account = %config.default_account,
        "Feishu websocket listener configured"
    );

    StreamClient::builder(sdk_config)
        .event_dispatcher(dispatcher)
        .build()?
        .start()
        .await
        .map_err(|error| anyhow!("Feishu websocket listener failed: {error}"))
}

fn ensure_rustls_provider() {
    let _ = CryptoProvider::get_default().or_else(|| {
        default_provider().install_default().ok()?;
        CryptoProvider::get_default()
    });
}

pub fn ensure_websocket_mode(config: &FeishuPluginConfig) -> Result<()> {
    if matches!(config.connection_mode, FeishuConnectionMode::Websocket) {
        Ok(())
    } else {
        Err(anyhow!("Feishu websocket listener requires websocket mode"))
    }
}

struct FeishuMessageHandler {
    config: FeishuPluginConfig,
    gateway: Arc<dyn GatewayService>,
    bot_open_id: Option<String>,
    dry_run: bool,
    dedup_store: Arc<Mutex<DedupStore>>,
    chat_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl FeishuMessageHandler {
    async fn should_process_message(&self, event_id: Option<&str>, message_id: &str) -> Result<bool> {
        let mut store = self.dedup_store.lock().await;
        store.should_process(&self.config.default_account, event_id, message_id)
    }

    async fn chat_lock(&self, chat_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.chat_locks.lock().await;
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
        locks
            .entry(chat_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

impl EventHandler for FeishuMessageHandler {
    fn event_type(&self) -> &str {
        "im.message.receive_v1"
    }

    fn handle(
        &self,
        event: Event,
    ) -> Pin<Box<dyn std::future::Future<Output = EventHandlerResult> + Send + '_>> {
        Box::pin(async move {
            let event_id = event.event_id().map(ToString::to_string);
            let envelope = FeishuEventEnvelope {
                event: event
                    .event
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| feishu_sdk::core::Error::InvalidEventFormat(error.to_string()))?,
            };

            let Some(message_id) = envelope
                .event
                .as_ref()
                .map(|event| event.message.message_id.as_str())
            else {
                return Ok(None);
            };
            let Some(chat_id) = envelope
                .event
                .as_ref()
                .map(|event| event.message.chat_id.as_str())
            else {
                return Ok(None);
            };

            if !self
                .should_process_message(event_id.as_deref(), message_id)
                .await
                .map_err(|error| feishu_sdk::core::Error::WebSocketError(error.to_string()))?
            {
                info!(event_id = ?event_id, message_id = %message_id, "Feishu duplicate message ignored");
                return Ok(None);
            }

            let chat_lock = self.chat_lock(chat_id).await;
            let _guard = chat_lock.lock().await;

            FeishuPluginRuntime::handle_event(
                &self.config,
                self.gateway.as_ref(),
                self.bot_open_id.as_deref(),
                envelope,
                self.dry_run,
            )
            .await
            .map_err(|error| feishu_sdk::core::Error::WebSocketError(error.to_string()))?;

            Ok(None)
        })
    }
}
