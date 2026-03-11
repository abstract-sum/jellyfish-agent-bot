use jellyfish_schema::{InboundMessage, SessionLocator};

pub fn session_locator_for_message(message: &InboundMessage) -> SessionLocator {
    SessionLocator {
        channel: message.channel.clone(),
        account_id: message.account_id.clone(),
        peer_kind: message.peer.kind.clone(),
        peer_id: message.peer.id.clone(),
        thread_id: message.peer.thread_id.clone(),
    }
}
