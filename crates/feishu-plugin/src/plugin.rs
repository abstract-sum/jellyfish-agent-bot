use anyhow::Result;
use jellyfish_gateway::GatewayService;
use std::sync::Arc;
use tracing::{info, warn};

use crate::channel::FeishuChannelPlugin;
use crate::config::FeishuPluginConfig;
use crate::parse::parse_inbound_message;
use crate::types::FeishuEventEnvelope;
use crate::websocket::{ensure_websocket_mode, start_websocket_listener};

pub struct FeishuPluginRuntime;

impl FeishuPluginRuntime {
    pub async fn start(
        config: &FeishuPluginConfig,
        gateway: Arc<dyn GatewayService>,
        bot_open_id: Option<String>,
        dry_run: bool,
    ) -> Result<()> {
        info!(
            domain = %config.domain.open_base_url(),
            account = %config.default_account,
            require_mention = config.require_mention,
            dry_run,
            "starting Feishu plugin runtime"
        );
        ensure_websocket_mode(config)?;
        start_websocket_listener(config, gateway, bot_open_id, dry_run).await
    }

    pub async fn handle_event(
        config: &FeishuPluginConfig,
        gateway: &dyn GatewayService,
        bot_open_id: Option<&str>,
        event: FeishuEventEnvelope,
        dry_run: bool,
    ) -> Result<()> {
        if let Some(message) = parse_inbound_message(
            event,
            bot_open_id,
            &config.default_account,
            config.require_mention,
        ) {
            info!(
                chat_id = %message.peer.id,
                sender_id = %message.sender_id,
                peer_kind = ?message.peer.kind,
                raw_type = %message.raw_type,
                text = %message.text,
                "Feishu inbound message accepted"
            );
            let outbound = FeishuChannelPlugin::dispatch_reply(config, gateway, message).await?;
            if dry_run {
                info!(
                    target = %outbound.peer.id,
                    text = %outbound.text,
                    "Feishu dry-run: outbound reply suppressed"
                );
            } else {
                FeishuChannelPlugin::send_outbound(config, &outbound).await?;
            }
        } else {
            warn!("Feishu event ignored because it did not match the Milestone 1 message rules or was authored by the bot itself");
        }

        Ok(())
    }
}
