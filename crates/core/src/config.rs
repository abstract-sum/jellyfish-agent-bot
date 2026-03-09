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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::OpenAi,
            model: "gpt-4o-mini".to_string(),
            workspace_root: PathBuf::from("."),
            log_filter: "info".to_string(),
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
    }
}
