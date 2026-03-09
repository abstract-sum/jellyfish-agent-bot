use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    UserMessage,
    AgentMessage,
    ToolRequested,
    ToolStarted,
    ToolCompleted,
    ToolFailed,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentEvent {
    pub kind: EventKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolEvent {
    pub tool_name: String,
    pub message: String,
}
