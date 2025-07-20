// src/rpc_client.rs

use crate::types::{ToolCallRequest, ToolCallResponse};
use crate::error::BrightDataError;
use crate::tool::Tool;
use serde_json::Value;

pub struct RpcClient;

impl RpcClient {
    /// Directly dispatch to the Rust-native tool implementation instead of spawning Node.js.
    pub async fn call_tool(tool_name: &str, parameters: Value) -> Result<Value, BrightDataError> {
        match tool_name {
            "scrape_website" => {
                crate::tools::scrape::ScrapeMarkdown
                    .execute(parameters)
                    .await
            }
            "search_web" => {
                crate::tools::search::SearchEngine
                    .execute(parameters)
                    .await
            }
            "extract_data" => {
                crate::tools::extract::Extractor
                    .execute(parameters)
                    .await
            }
            _ => Err(BrightDataError::ToolError(format!(
                "Unknown tool: {}",
                tool_name
            ))),
        }
    }
}
