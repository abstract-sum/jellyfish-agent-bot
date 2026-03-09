use serde::{Deserialize, Serialize};

use crate::{event::AgentEvent, types::SessionId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: SessionId,
    pub messages: Vec<Message>,
    pub events: Vec<AgentEvent>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: SessionId::new(),
            messages: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn push_message(&mut self, role: MessageRole, content: impl Into<String>) {
        self.messages.push(Message {
            role,
            content: content.into(),
        });
    }

    pub fn push_event(&mut self, event: AgentEvent) {
        self.events.push(event);
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
