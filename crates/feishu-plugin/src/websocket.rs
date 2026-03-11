use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use feishu_sdk::core::{Config, LogLevel, new_logger};
use feishu_sdk::event::{Event, EventDispatcher, EventDispatcherConfig, EventHandler, EventHandlerResult};
use feishu_sdk::ws::StreamClient;
use jellyfish_gateway::GatewayService;
use rustls::crypto::{CryptoProvider, ring::default_provider};
use tracing::info;

use crate::config::{FeishuConnectionMode, FeishuPluginConfig};
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
            let envelope = FeishuEventEnvelope {
                event: event
                    .event
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| feishu_sdk::core::Error::InvalidEventFormat(error.to_string()))?,
            };

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
