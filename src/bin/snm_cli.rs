// src/bin/snm_cli.rs - Cleaned up version
use snm_brightdata_client::tools::{scrape::ScrapeMarkdown, search::SearchEngine, extract::Extractor, screenshot::ScreenshotTool};
use snm_brightdata_client::tool::{Tool, ToolResult};
use snm_brightdata_client::error::BrightDataError;
use clap::{Parser, Subcommand};
use serde_json::json;
use dotenv::dotenv;

#[derive(Parser)]
#[command(name = "snm-cli")]
#[command(about = "BrightData MCP Rust CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scrape a website and extract content
    Scrape { 
        url: String,
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// Search the web using various engines
    Search { 
        query: String,
        #[arg(short, long, default_value = "google")]
        engine: String,
    },
    /// Extract structured data from a webpage
    Extract { 
        url: String,
        #[arg(short, long, default_value = "json")]
        format: String,
    },
    /// Take a screenshot of a webpage
    Screenshot {
        url: String,
        #[arg(short, long, default_value_t = 1280)]
        width: i32,
        #[arg(short, long, default_value_t = 720)]
        height: i32,
        #[arg(long)]
        full_page: bool,
    },
}

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv().ok();
    env_logger::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Scrape { url, format } => {
            let result = ScrapeMarkdown
                .execute(json!({"url": url, "format": format}))
                .await;
            handle_result(result);
        },
        Commands::Search { query, engine } => {
            let result = SearchEngine
                .execute(json!({"query": query, "engine": engine}))
                .await;
            handle_result(result);
        },
        Commands::Extract { url, format } => {
            let result = Extractor
                .execute(json!({"url": url, "format": format}))
                .await;
            handle_result(result);
        },
        Commands::Screenshot { url, width, height, full_page } => {
            let result = ScreenshotTool
                .execute(json!({
                    "url": url,
                    "width": width,
                    "height": height,
                    "full_page": full_page
                }))
                .await;
            handle_result(result);
        },
    }
}

fn handle_result(result: Result<ToolResult, BrightDataError>) {
    match result {
        Ok(tool_result) => {
            println!("✅ Tool execution successful!");
            
            // Display content
            for (i, content) in tool_result.content.iter().enumerate() {
                match content.content_type.as_str() {
                    "text" => {
                        if tool_result.content.len() > 1 {
                            println!("\n📄 Content {}:", i + 1);
                        }
                        println!("{}", content.text);
                    },
                    "image" => {
                        println!("\n🖼️  Image Content {}:", i + 1);
                        println!("Description: {}", content.text);
                        if let Some(media_type) = &content.media_type {
                            println!("Media Type: {}", media_type);
                        }
                        if let Some(data) = &content.data {
                            println!("Image Data Length: {} characters", data.len());
                        }
                    },
                    _ => {
                        println!("\n📋 {} Content:", content.content_type);
                        println!("{}", content.text);
                    }
                }
            }
            
            // Show error status if present
            if let Some(is_error) = tool_result.is_error {
                if is_error {
                    println!("\n⚠️  Tool reported an error condition");
                }
            }
        },
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            std::process::exit(1);
        }
    }
}