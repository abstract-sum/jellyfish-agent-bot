use crate::config::{FeishuAccountConfig, FeishuPluginConfig};

pub fn resolve_account(config: &FeishuPluginConfig) -> &FeishuAccountConfig {
    &config.account
}
