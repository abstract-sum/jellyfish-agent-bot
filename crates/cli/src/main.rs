mod args;
mod memory;
mod output;
mod retrieval;
mod session_store;

use anyhow::Result;
use clap::Parser;
use jellyfish_agent::{AgentRequest, build_runtime};
use jellyfish_core::{AppConfig, MessageRole, ProviderKind};
use jellyfish_feishu_plugin::{FeishuPluginConfig, plugin::FeishuPluginRuntime, probe::probe_feishu};
use jellyfish_gateway::JellyfishGateway;
use std::env;
use std::io::{self, Write};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use crate::args::{ChannelCommands, Cli, Commands, SessionCommands};
use crate::retrieval::RetrievalSnapshot;

fn codex_auth_cache_exists() -> bool {
    jellyfish_agent::codex_cli::codex_auth_cache_exists()
}

fn codex_cli_available() -> bool {
    jellyfish_agent::codex_cli::codex_cli_available()
}

fn codex_native_ready() -> bool {
    jellyfish_agent::codex_auth::load_codex_credentials()
        .map(|credentials| credentials.is_some())
        .unwrap_or(false)
}

fn feishu_env_status() -> (bool, bool) {
    let has_app_id = env::var("FEISHU_APP_ID").is_ok() || env::var("LARK_APP_ID").is_ok();
    let has_app_secret =
        env::var("FEISHU_APP_SECRET").is_ok() || env::var("LARK_APP_SECRET").is_ok();
    (has_app_id, has_app_secret)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.log_filter.clone()))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Chat { input, yes } => {
            let runtime_config = config.with_file_edits_allowed(yes);
            let runtime = build_runtime(runtime_config.clone())?;
            run_chat_turn(&*runtime, &runtime_config, &input).await?;
        }
        Commands::Repl { yes } => {
            let runtime_config = config.with_file_edits_allowed(yes);
            let runtime = build_runtime(runtime_config.clone())?;
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

                run_chat_turn(&*runtime, &runtime_config, input).await?;
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
        Commands::Channel { command } => match command {
            ChannelCommands::FeishuProbe => {
                let config = FeishuPluginConfig::from_env()?;
                let probe = probe_feishu(&config).await?;
                println!("Feishu probe succeeded.");
                println!("Domain: {}", probe.domain);
                println!("Connection mode: {}", probe.connection_mode);
                println!("Default account: {}", probe.account_id);
                println!("App ID prefix: {}", probe.app_id_prefix);
                println!("Require mention in groups: {}", config.require_mention);
            }
            ChannelCommands::FeishuDoctor => {
                let (has_app_id, has_app_secret) = feishu_env_status();
                println!("Feishu/Lark doctor");
                println!("FEISHU_APP_ID/LARK_APP_ID present: {}", has_app_id);
                println!("FEISHU_APP_SECRET/LARK_APP_SECRET present: {}", has_app_secret);
                match FeishuPluginConfig::from_env() {
                    Ok(config) => {
                        println!("Domain: {}", config.domain.open_base_url());
                        println!(
                            "Connection mode: {}",
                            match config.connection_mode {
                                jellyfish_feishu_plugin::FeishuConnectionMode::Websocket => "websocket",
                                jellyfish_feishu_plugin::FeishuConnectionMode::Webhook => "webhook",
                            }
                        );
                        println!("Default account: {}", config.default_account);
                        println!("Require mention in groups: {}", config.require_mention);
                        match probe_feishu(&config).await {
                            Ok(_) => println!("Probe: ok"),
                            Err(error) => println!("Probe: failed ({error})"),
                        }
                    }
                    Err(error) => {
                        println!("Config: invalid ({error})");
                    }
                }
            }
            ChannelCommands::FeishuStart { bot_open_id, dry_run } => {
                let feishu_config = FeishuPluginConfig::from_env()?;
                let gateway = Arc::new(JellyfishGateway::new(config.clone()));
                println!("Starting Feishu websocket listener...");
                println!("Domain: {}", feishu_config.domain.open_base_url());
                println!("Default account: {}", feishu_config.default_account);
                println!("Require mention in groups: {}", feishu_config.require_mention);
                println!("Dry run: {}", dry_run);
                FeishuPluginRuntime::start(&feishu_config, gateway, bot_open_id, dry_run).await?;
            }
        },
        Commands::Recall { query } => {
            let session = session_store::load_or_create(&config.workspace_root)?;
            let snapshot = RetrievalSnapshot::load(&config.workspace_root, &session)?;
            let hits = snapshot.search(&query, 10);

            if hits.is_empty() {
                println!("No retrieval matches found for '{}'", query);
            } else {
                println!("Retrieval matches for '{}':", query);
                for hit in hits {
                    println!("- [{}] {} (score {})", hit.source, hit.content, hit.score);
                }
            }
        }
        Commands::Doctor => {
            let runtime = build_runtime(config.clone())?;
            let runtime_status = "ready".to_string();
            let credential_status = match config.provider {
                ProviderKind::OpenAi => match env::var("OPENAI_API_KEY") {
                    Ok(value) if !value.trim().is_empty() => "OPENAI_API_KEY detected".to_string(),
                    _ => "OPENAI_API_KEY missing".to_string(),
                },
                ProviderKind::Codex => {
                    if codex_native_ready() {
                        "codex OAuth credentials detected".to_string()
                    } else {
                        "codex OAuth credentials missing or invalid".to_string()
                    }
                }
                ProviderKind::CodexCli => {
                    if codex_auth_cache_exists() {
                        "codex auth cache detected".to_string()
                    } else {
                        "codex auth cache missing".to_string()
                    }
                }
                ProviderKind::Mock => "no external credentials required".to_string(),
                ProviderKind::Anthropic => "anthropic provider is not wired yet".to_string(),
            };

            let session = session_store::load_or_create(&config.workspace_root)?;
            let snapshot = RetrievalSnapshot::load(&config.workspace_root, &session)?;

            println!("Jellyfish Phase 4 scaffold is healthy.");
            println!("Provider: {}", config.provider.as_str());
            println!("Model: {}", config.model);
            println!("Workspace root: {}", config.workspace_root.display());
            println!("Runtime: {}", runtime_status);
            println!("Credentials: {}", credential_status);
            println!("Repo tools enabled: {}", config.enable_repo_tools);
            println!("File edits allowed: {}", config.allow_file_edits);
            println!("Tool timeout (secs): {}", config.tool_timeout_secs);
            println!("Tool output max chars: {}", config.tool_output_max_chars);
            println!("Codex transport: {}", config.codex_transport.as_str());
            println!("Retrieval entries: {}", snapshot.len());
            println!(
                "Session file: {}",
                session_store::session_file_path(&config.workspace_root).display()
            );
            if matches!(config.provider, ProviderKind::Codex) {
                println!("Codex auth cache detected: {}", codex_auth_cache_exists());
                println!("Codex native credentials ready: {}", codex_native_ready());
                println!("Codex token refresh support: enabled");
                println!(
                    "Codex auth mode: Jellyfish reads OAuth credentials from ~/.codex/auth.json and calls chatgpt.com/backend-api/codex/responses"
                );
            }
            if matches!(config.provider, ProviderKind::CodexCli) {
                println!("Codex CLI detected: {}", codex_cli_available());
                println!("Codex auth cache detected: {}", codex_auth_cache_exists());
                println!(
                    "Codex auth mode: Jellyfish shells out to codex CLI and relies on the CLI's own login state"
                );
            }
            drop(runtime);
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
    let snapshot = RetrievalSnapshot::load(&config.workspace_root, &session)?;
    let retrieval_context = snapshot.context_lines(input, 8);

    let response = runtime
        .run(AgentRequest {
            input: input.to_string(),
            session: Some(session.clone()),
            retrieval_context,
        })
        .await?;

    session.push_message(MessageRole::Assistant, response.message.clone());
    for event in &response.events {
        session.push_event(event.clone());
    }
    let session_path = session_store::save(&config.workspace_root, &session)?;

    output::print_agent_response(&response, &memory_updates, &session_path);
    Ok(())
}
