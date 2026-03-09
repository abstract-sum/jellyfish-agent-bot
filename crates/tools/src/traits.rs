use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use jellyfish_core::AppResult;

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

impl ToolOutput {
    pub fn truncated(mut self, max_chars: usize) -> Self {
        if self.content.chars().count() <= max_chars {
            return self;
        }

        let truncated = self.content.chars().take(max_chars).collect::<String>();
        self.content = format!(
            "{}\n[truncated to {} chars]",
            truncated, max_chars
        );
        self
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    async fn call(&self, input: Value) -> AppResult<ToolOutput>;
}
