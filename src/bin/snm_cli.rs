// src/bin/snm_cli.rs
use snm_brightdata_client::tools::{scrape::ScrapeMarkdown, search::SearchEngine, extract::Extractor};
use snm_brightdata_client::tool::Tool;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "snm-cli")]
#[command(about = "BrightData MCP Rust CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Scrape { url: String },
    Search { query: String },
    Extract { url: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scrape { url } => {
            let result = ScrapeMarkdown.execute(json!({"url": url})).await;
            handle_result(result);
        },
        Commands::Search { query } => {
            let result = SearchEngine.execute(json!({"query": query})).await;
            handle_result(result);
        },
        Commands::Extract { url } => {
            let result = Extractor.execute(json!({"url": url})).await;
            handle_result(result);
        },
    }
}

fn handle_result(result: Result<serde_json::Value, snm_brightdata_client::error::BrightDataError>) {
    match result {
        Ok(output) => println!("{:#?}", output),
        Err(e) => eprintln!("Error: {}", e),
    }
}








