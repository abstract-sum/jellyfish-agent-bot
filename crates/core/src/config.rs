use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

use crate::types::ProviderKind;
use crate::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub provider: ProviderKind,
    pub model: String,
    pub workspace_root: PathBuf,
    pub log_filter: String,
    pub enable_repo_tools: bool,
    pub tool_timeout_secs: u64,
    pub tool_output_max_chars: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::OpenAi,
            model: "gpt-4o-mini".to_string(),
            workspace_root: PathBuf::from("."),
            log_filter: "info".to_string(),
            enable_repo_tools: false,
            tool_timeout_secs: 10,
            tool_output_max_chars: 4_000,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> AppResult<Self> {
        let default = Self::default();

        let provider = env::var("RIG_PROVIDER")
            .ok()
            .map(|value| value.parse())
            .transpose()
            .map_err(AppError::Config)?
            .unwrap_or(default.provider);

        let model = env::var("RIG_MODEL").unwrap_or(default.model);
        let log_filter = env::var("RIG_LOG").unwrap_or(default.log_filter);
        let enable_repo_tools = env::var("RIG_ENABLE_REPO_TOOLS")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(default.enable_repo_tools);
        let tool_timeout_secs = env::var("RIG_TOOL_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(default.tool_timeout_secs);
        let tool_output_max_chars = env::var("RIG_TOOL_OUTPUT_MAX_CHARS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(default.tool_output_max_chars);
        let workspace_root = env::var("RIG_WORKSPACE_ROOT")
            .map(PathBuf::from)
            .unwrap_or(default.workspace_root);

        if !workspace_root.exists() {
            return Err(AppError::Config(format!(
                "workspace root does not exist: {}",
                workspace_root.display()
            )));
        }

        if !Path::new(&workspace_root).is_dir() {
            return Err(AppError::Config(format!(
                "workspace root is not a directory: {}",
                workspace_root.display()
            )));
        }

        Ok(Self {
            provider,
            model,
            workspace_root,
            log_filter,
            enable_repo_tools,
            tool_timeout_secs,
            tool_output_max_chars,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_relative_workspace() {
        let config = AppConfig::default();
        assert_eq!(config.provider, ProviderKind::OpenAi);
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.workspace_root, PathBuf::from("."));
        assert!(!config.enable_repo_tools);
        assert_eq!(config.tool_timeout_secs, 10);
        assert_eq!(config.tool_output_max_chars, 4_000);
    }
}
