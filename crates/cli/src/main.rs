mod args;
mod output;

use anyhow::Result;
use clap::Parser;
use openclaw_agent::{AgentRequest, build_runtime};
use openclaw_core::{AppConfig, ProviderKind};
use std::env;
use tracing_subscriber::EnvFilter;

use crate::args::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.log_filter.clone()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Chat { input } => {
            let runtime = build_runtime(config.clone())?;
            let response = runtime.run(AgentRequest { input }).await?;
            output::print_agent_response(&response);
        }
        Commands::Doctor => {
            let runtime_status = build_runtime(config.clone())
                .map(|_| "ready".to_string())
                .unwrap_or_else(|error| format!("not ready ({error})"));
            let credential_status = match config.provider {
                ProviderKind::OpenAi => match env::var("OPENAI_API_KEY") {
                    Ok(value) if !value.trim().is_empty() => "OPENAI_API_KEY detected".to_string(),
                    _ => "OPENAI_API_KEY missing".to_string(),
                },
                ProviderKind::Mock => "no external credentials required".to_string(),
                ProviderKind::Anthropic => "anthropic provider is not wired yet".to_string(),
            };

            println!("OpenClaw Phase 1 scaffold is healthy.");
            println!("Provider: {}", config.provider.as_str());
            println!("Model: {}", config.model);
            println!("Workspace root: {}", config.workspace_root.display());
            println!("Runtime: {}", runtime_status);
            println!("Credentials: {}", credential_status);
        }
    }

    Ok(())
}
