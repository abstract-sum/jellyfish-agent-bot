use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "jellyfish",
    version,
    about = "Jellyfish personal assistant CLI"
)]
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
    Repl,
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Doctor,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    Show,
    Reset,
}
