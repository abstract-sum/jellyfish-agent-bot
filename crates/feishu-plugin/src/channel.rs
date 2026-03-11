use anyhow::Result;
use jellyfish_gateway::GatewayService;
use jellyfish_schema::OutboundMessage;

use crate::config::FeishuPluginConfig;
use crate::send::send_text;

pub struct FeishuChannelPlugin;

impl FeishuChannelPlugin {
    pub async fn dispatch_reply(
        _config: &FeishuPluginConfig,
        gateway: &dyn GatewayService,
        msg: jellyfish_schema::InboundMessage,
    ) -> Result<OutboundMessage> {
        gateway.handle_inbound(msg).await
    }

    pub async fn send_outbound(config: &FeishuPluginConfig, message: &OutboundMessage) -> Result<()> {
        send_text(config, &message.peer.id, &message.text).await
    }
}
