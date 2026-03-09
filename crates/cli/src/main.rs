mod args;
mod memory;
mod output;
mod session_store;

use anyhow::Result;
use clap::Parser;
use jellyfish_agent::{AgentRequest, build_runtime};
use jellyfish_core::{AppConfig, MessageRole, ProviderKind};
use std::env;
use std::io::{self, Write};
use tracing_subscriber::EnvFilter;

use crate::args::{Cli, Commands, SessionCommands};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.log_filter.clone()))
        .init();

    let cli = Cli::parse();
    let runtime = build_runtime(config.clone())?;

    match cli.command {
        Commands::Chat { input } => {
            run_chat_turn(&*runtime, &config, &input).await?;
        }
        Commands::Repl => {
            println!("Jellyfish REPL started. Type 'exit' or 'quit' to leave.");
            loop {
                print!("jellyfish> ");
                io::stdout().flush()?;

                let mut input = String::new();
                let bytes = io::stdin().read_line(&mut input)?;
                if bytes == 0 {
                    break;
                }

                let input = input.trim();
                if input.is_empty() {
                    continue;
                }
                if matches!(input, "exit" | "quit") {
                    break;
                }

                run_chat_turn(&*runtime, &config, input).await?;
            }
        }
        Commands::Session { command } => match command {
            SessionCommands::Show => {
                let session = session_store::load_or_create(&config.workspace_root)?;
                println!("Session id: {:?}", session.id);
                println!("Display name: {:?}", session.profile.display_name);
                println!("Locale: {:?}", session.profile.locale);
                println!("Timezone: {:?}", session.profile.timezone);
                println!("Preferences: {}", session.profile.preferences.len());
                println!("Memories: {}", session.memories.len());
                println!("Messages: {}", session.messages.len());
                println!("Events: {}", session.events.len());
                for memory in session.memory_summary(10) {
                    println!("- {}", memory);
                }
            }
            SessionCommands::Reset => {
                let path = session_store::session_file_path(&config.workspace_root);
                if path.exists() {
                    std::fs::remove_file(&path)?;
                    println!("Reset session file at {}", path.display());
                } else {
                    println!("No session file to reset at {}", path.display());
                }
            }
        }
        Commands::Doctor => {
            let runtime_status = "ready".to_string();
            let credential_status = match config.provider {
                ProviderKind::OpenAi => match env::var("OPENAI_API_KEY") {
                    Ok(value) if !value.trim().is_empty() => "OPENAI_API_KEY detected".to_string(),
                    _ => "OPENAI_API_KEY missing".to_string(),
                },
                ProviderKind::Mock => "no external credentials required".to_string(),
                ProviderKind::Anthropic => "anthropic provider is not wired yet".to_string(),
            };

            println!("Jellyfish Phase 2 scaffold is healthy.");
            println!("Provider: {}", config.provider.as_str());
            println!("Model: {}", config.model);
            println!("Workspace root: {}", config.workspace_root.display());
            println!("Runtime: {}", runtime_status);
            println!("Credentials: {}", credential_status);
            println!("Repo tools enabled: {}", config.enable_repo_tools);
            println!("Tool timeout (secs): {}", config.tool_timeout_secs);
            println!("Tool output max chars: {}", config.tool_output_max_chars);
            println!(
                "Session file: {}",
                session_store::session_file_path(&config.workspace_root).display()
            );
        }
    }

    Ok(())
}

async fn run_chat_turn(
    runtime: &dyn jellyfish_agent::AgentRuntime,
    config: &AppConfig,
    input: &str,
) -> Result<()> {
    let mut session = session_store::load_or_create(&config.workspace_root)?;
    if session.profile.display_name.is_none() {
        session.set_display_name("User");
    }
    if session.profile.preferences.is_empty() {
        session.set_preference("assistant_style", "concise");
    }

    session.push_message(MessageRole::User, input.to_string());
    let memory_updates = memory::apply_memory_updates(&mut session, input);

    let response = runtime
        .run(AgentRequest {
            input: input.to_string(),
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
    Ok(())
}
