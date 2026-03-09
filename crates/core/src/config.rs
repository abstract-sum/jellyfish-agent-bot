use serde::{Deserialize, Serialize};

use crate::types::ProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub provider: ProviderKind,
    pub model: String,
    pub workspace_root: String,
    pub log_filter: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::OpenAi,
            model: "gpt-4o-mini".to_string(),
            workspace_root: ".".to_string(),
            log_filter: "info".to_string(),
        }
    }
}
