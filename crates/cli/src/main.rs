mod args;
mod memory;
mod output;
mod session_store;

use anyhow::Result;
use clap::Parser;
use jellyfish_agent::{AgentRequest, build_runtime};
use jellyfish_core::{AppConfig, MessageRole, ProviderKind};
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
            let mut session = session_store::load_or_create(&config.workspace_root)?;
            if session.profile.display_name.is_none() {
                session.set_display_name("User");
            }
            if session.profile.preferences.is_empty() {
                session.set_preference("assistant_style", "concise");
            }

            session.push_message(MessageRole::User, input.clone());
            let memory_updates = memory::apply_memory_updates(&mut session, &input);

            let response = runtime
                .run(AgentRequest {
                    input,
                    session: Some(session.clone()),
                })
                .await?;

            session.push_message(MessageRole::Assistant, response.message.clone());
            for event in &response.events {
                session.push_event(event.clone());
            }
            let session_path = session_store::save(&config.workspace_root, &session)?;

            output::print_agent_response(&response);
            if !memory_updates.is_empty() {
                for update in memory_updates {
                    println!("- [Memory] {}", update);
                }
            }
            println!("- [Session] Saved to {}", session_path.display());
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

            println!("Jellyfish Phase 1 scaffold is healthy.");
            println!("Provider: {}", config.provider.as_str());
            println!("Model: {}", config.model);
            println!("Workspace root: {}", config.workspace_root.display());
            println!("Runtime: {}", runtime_status);
            println!("Credentials: {}", credential_status);
            println!("Repo tools enabled: {}", config.enable_repo_tools);
            println!(
                "Session file: {}",
                session_store::session_file_path(&config.workspace_root).display()
            );
        }
    }

    Ok(())
}
