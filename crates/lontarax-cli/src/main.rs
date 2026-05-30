mod cli;
mod output;
mod runtime;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .without_time()
        .init();

    let cli = Cli::parse();
    let json = cli.json;

    if let Err(error) = runtime::run(cli).await {
        if json {
            let payload = output::ErrorOutput {
                error: error.to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
        } else {
            eprintln!("error: {error}");
        }
        std::process::exit(1);
    }
}
