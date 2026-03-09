use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "openclaw", version, about = "OpenClaw CLI scaffold")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Chat {
        #[arg(default_value = "hello")]
        input: String,
    },
    Doctor,
}
