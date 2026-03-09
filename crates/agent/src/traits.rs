use async_trait::async_trait;

use jellyfish_core::AppResult;

use crate::runtime::{AgentRequest, AgentResponse};

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn run(&self, request: AgentRequest) -> AppResult<AgentResponse>;
}
