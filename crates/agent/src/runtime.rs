use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use openclaw_core::{AgentEvent, AppResult, EventKind};

use crate::{prompt::PromptTemplate, traits::AgentRuntime};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRequest {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentResponse {
    pub message: String,
    pub events: Vec<AgentEvent>,
}

#[derive(Debug, Clone)]
pub struct StubAgentRuntime {
    prompt: PromptTemplate,
}

impl StubAgentRuntime {
    pub fn new(prompt: PromptTemplate) -> Self {
        Self { prompt }
    }
}

#[async_trait]
impl AgentRuntime for StubAgentRuntime {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse> {
        Ok(AgentResponse {
            message: format!(
                "Phase 0 runtime is ready. Received input: {}",
                request.input
            ),
            events: vec![AgentEvent {
                kind: EventKind::System,
                message: format!("Loaded prompt: {}", self.prompt.system),
            }],
        })
    }
}
