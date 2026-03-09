use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use openclaw_core::{AppError, AppResult};

use crate::traits::Tool;

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T>(&mut self, tool: T)
    where
        T: Tool + 'static,
    {
        let definition = tool.definition();
        self.tools.insert(definition.name, Arc::new(tool));
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names = self.tools.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn definitions(&self) -> Vec<crate::ToolDefinition> {
        let mut definitions = self
            .tools
            .values()
            .map(|tool| tool.definition())
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.name.cmp(&right.name));
        definitions
    }

    pub async fn call(&self, name: &str, input: Value) -> AppResult<crate::ToolOutput> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| AppError::Tool(format!("unknown tool: {name}")))?;

        tool.call(input).await
    }
}
