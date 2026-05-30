use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "lontarax", version, about = "Lightweight AI API router CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,

    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Init,
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    Models(ModelsArgs),
    Credits(ProviderSelectionArgs),
    Chat(ChatArgs),
    Image(ImageArgs),
    Video(VideoArgs),
    History(HistoryArgs),
}

#[derive(Debug, Args)]
pub struct ProviderSelectionArgs {
    #[arg(long)]
    pub provider: Option<String>,
}

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[arg(long)]
    pub provider: Option<String>,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[arg(long, default_value_t = 25)]
    pub limit: i64,
}

#[derive(Debug, Args)]
pub struct ChatArgs {
    pub prompt: String,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Debug, Args)]
pub struct ImageArgs {
    pub prompt: String,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Debug, Args)]
pub struct VideoArgs {
    pub prompt: String,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
    Add {
        name: String,
        #[arg(long)]
        api_key: Option<String>,
        #[arg(long)]
        base_url: Option<String>,
    },
    List,
    Remove {
        name: String,
    },
}
