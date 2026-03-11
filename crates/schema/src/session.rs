use serde::{Deserialize, Serialize};

use crate::channel::{ChannelKind, PeerKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionLocator {
    pub channel: ChannelKind,
    pub account_id: String,
    pub peer_kind: PeerKind,
    pub peer_id: String,
    pub thread_id: Option<String>,
}
