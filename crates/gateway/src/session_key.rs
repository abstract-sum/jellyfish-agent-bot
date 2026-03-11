use jellyfish_schema::{ChannelKind, PeerKind, SessionLocator};

pub fn build_session_key(locator: &SessionLocator) -> String {
    let channel = match &locator.channel {
        ChannelKind::Feishu => "feishu".to_string(),
        ChannelKind::Custom(value) => value.clone(),
    };
    let peer_kind = match locator.peer_kind {
        PeerKind::Direct => "direct",
        PeerKind::Group => "group",
    };

    match &locator.thread_id {
        Some(thread_id) => format!(
            "{}:{}:{}:{}:{}",
            channel, locator.account_id, peer_kind, locator.peer_id, thread_id
        ),
        None => format!(
            "{}:{}:{}:{}",
            channel, locator.account_id, peer_kind, locator.peer_id
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_direct_session_key() {
        let key = build_session_key(&SessionLocator {
            channel: ChannelKind::Feishu,
            account_id: "main".to_string(),
            peer_kind: PeerKind::Direct,
            peer_id: "ou_xxx".to_string(),
            thread_id: None,
        });

        assert_eq!(key, "feishu:main:direct:ou_xxx");
    }

    #[test]
    fn builds_group_session_key() {
        let key = build_session_key(&SessionLocator {
            channel: ChannelKind::Feishu,
            account_id: "main".to_string(),
            peer_kind: PeerKind::Group,
            peer_id: "oc_xxx".to_string(),
            thread_id: None,
        });

        assert_eq!(key, "feishu:main:group:oc_xxx");
    }
}
