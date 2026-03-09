use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use openclaw_core::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    async fn call(&self, input: Value) -> AppResult<ToolOutput>;
}
