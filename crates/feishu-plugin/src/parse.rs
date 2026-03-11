use jellyfish_schema::{
    ChannelKind, ChannelPeer, InboundMessage, MediaRef, MentionTarget, PeerKind,
};

use crate::types::{FeishuEventEnvelope, FeishuTextContent};

pub fn parse_inbound_message(
    envelope: FeishuEventEnvelope,
    bot_open_id: Option<&str>,
    account_id: &str,
    require_mention: bool,
) -> Option<InboundMessage> {
    let event = envelope.event?;
    if event.message.message_type != "text" {
        return None;
    }

    let parsed: FeishuTextContent = serde_json::from_str(&event.message.content).ok()?;
    let is_group = event.message.chat_type == "group";
    let bot_open_id = bot_open_id.unwrap_or_default().trim();

    let mentions = event
        .message
        .mentions
        .unwrap_or_default()
        .into_iter()
        .map(|mention| MentionTarget {
            id: mention
                .id
                .open_id
                .or(mention.id.user_id)
                .unwrap_or_default(),
            name: Some(mention.name),
        })
        .collect::<Vec<_>>();

    let mentioned_bot =
        !bot_open_id.is_empty() && mentions.iter().any(|mention| mention.id == bot_open_id);
    if is_group && require_mention && !mentioned_bot {
        return None;
    }

    let mut text = parsed.text;
    if mentioned_bot {
        for mention in &mentions {
            if let Some(name) = &mention.name {
                text = text.replace(&format!("@{}", name), "").trim().to_string();
            }
        }
    }

    let sender_id = event
        .sender
        .sender_id
        .open_id
        .or(event.sender.sender_id.user_id)
        .unwrap_or_default();

    Some(InboundMessage {
        channel: ChannelKind::Feishu,
        account_id: account_id.to_string(),
        peer: ChannelPeer {
            kind: if is_group {
                PeerKind::Group
            } else {
                PeerKind::Direct
            },
            id: event.message.chat_id,
            thread_id: None,
        },
        sender_id,
        sender_name: None,
        message_id: event.message.message_id,
        reply_to_message_id: None,
        text,
        raw_type: event.message.message_type,
        timestamp_ms: event
            .message
            .create_time
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or_default(),
        mentions,
        media: Vec::<MediaRef>::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct_event() -> FeishuEventEnvelope {
        serde_json::from_str(r#"{"event":{"sender":{"sender_id":{"open_id":"ou_user"}},"message":{"message_id":"om_1","chat_id":"oc_dm","chat_type":"p2p","create_time":"123","message_type":"text","content":"{\"text\":\"hello\"}"}}}"#).unwrap()
    }

    #[test]
    fn parses_direct_text_message_into_inbound_message() {
        let message = parse_inbound_message(direct_event(), None, "main", true).unwrap();
        assert_eq!(message.text, "hello");
        assert_eq!(message.sender_id, "ou_user");
    }

    #[test]
    fn ignores_group_message_without_bot_mention() {
        let envelope: FeishuEventEnvelope = serde_json::from_str(r#"{"event":{"sender":{"sender_id":{"open_id":"ou_user"}},"message":{"message_id":"om_1","chat_id":"oc_group","chat_type":"group","create_time":"123","message_type":"text","content":"{\"text\":\"hello\"}","mentions":[]}}}"#).unwrap();
        assert!(parse_inbound_message(envelope, Some("ou_bot"), "main", true).is_none());
    }

    #[test]
    fn parses_group_message_with_bot_mention() {
        let envelope: FeishuEventEnvelope = serde_json::from_str(r#"{"event":{"sender":{"sender_id":{"open_id":"ou_user"}},"message":{"message_id":"om_1","chat_id":"oc_group","chat_type":"group","create_time":"123","message_type":"text","content":"{\"text\":\"@Jellyfish hello\"}","mentions":[{"name":"Jellyfish","id":{"open_id":"ou_bot"}}]}}}"#).unwrap();
        let message = parse_inbound_message(envelope, Some("ou_bot"), "main", true).unwrap();
        assert_eq!(message.text, "hello");
    }
}
