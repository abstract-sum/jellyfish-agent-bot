mod args;
mod output;

use anyhow::Result;
use clap::Parser;
use openclaw_agent::{AgentRequest, AgentRuntime, PromptTemplate, StubAgentRuntime};
use openclaw_core::AppConfig;
use tracing_subscriber::EnvFilter;

use crate::args::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::default();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.log_filter.clone()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Chat { input } => {
            let runtime = StubAgentRuntime::new(PromptTemplate::coding_assistant());
            let response = runtime.run(AgentRequest { input }).await?;
            output::print_agent_response(&response);
        }
        Commands::Doctor => {
            println!("OpenClaw Phase 0 scaffold is healthy.");
            println!("Provider: {:?}", config.provider);
            println!("Model: {}", config.model);
            println!("Workspace root: {}", config.workspace_root);
        }
    }

    Ok(())
}
