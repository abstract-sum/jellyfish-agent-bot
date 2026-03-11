use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuEventEnvelope {
    pub event: Option<FeishuMessageEventContainer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuMessageEventContainer {
    pub sender: FeishuSender,
    pub message: FeishuMessage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuSender {
    pub sender_id: FeishuSenderId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuSenderId {
    pub open_id: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuMessage {
    pub message_id: String,
    pub chat_id: String,
    pub chat_type: String,
    pub create_time: Option<String>,
    pub message_type: String,
    pub content: String,
    pub mentions: Option<Vec<FeishuMention>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuMention {
    pub name: String,
    pub id: FeishuSenderId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeishuTextContent {
    pub text: String,
}
