use serde::{Deserialize, Serialize};

use crate::channel::{ChannelKind, PeerKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelPeer {
    pub kind: PeerKind,
    pub id: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MentionTarget {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaRef {
    pub kind: String,
    pub url: Option<String>,
    pub local_path: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InboundMessage {
    pub channel: ChannelKind,
    pub account_id: String,
    pub peer: ChannelPeer,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub message_id: String,
    pub reply_to_message_id: Option<String>,
    pub text: String,
    pub raw_type: String,
    pub timestamp_ms: i64,
    pub mentions: Vec<MentionTarget>,
    pub media: Vec<MediaRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutboundMessage {
    pub channel: ChannelKind,
    pub account_id: String,
    pub peer: ChannelPeer,
    pub reply_to_message_id: Option<String>,
    pub text: String,
}
