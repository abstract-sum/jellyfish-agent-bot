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
        #[arg(long, help = "Allow dangerous file-edit tools for this run")]
        yes: bool,
    },
    Repl {
        #[arg(long, help = "Allow dangerous file-edit tools for this run")]
        yes: bool,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Channel {
        #[command(subcommand)]
        command: ChannelCommands,
    },
    Recall {
        query: String,
    },
    Doctor,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    Show,
    Reset,
}

#[derive(Debug, Subcommand)]
pub enum ChannelCommands {
    FeishuProbe,
    FeishuDoctor,
    FeishuStart {
        #[arg(long)]
        bot_open_id: Option<String>,
        #[arg(
            long,
            help = "Process inbound events but do not send replies back to Feishu"
        )]
        dry_run: bool,
    },
}
