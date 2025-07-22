// ===== src/tools/search.rs - CORRECTED VERSION =====
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;

pub struct SearchEngine;

#[async_trait]
impl Tool for SearchEngine {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search the web using various search engines via BrightData"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "engine": {
                    "type": "string",
                    "enum": ["google", "bing", "yandex", "duckduckgo"],
                    "description": "Search engine to use",
                    "default": "google"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let engine = parameters
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("google");

        let result = self.search_with_brightdata(query, engine).await?;

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No results");
        let mcp_content = vec![McpContent::text(format!(
            "🔍 **Search Results for '{}'**\n\n{}",
            query,
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl SearchEngine {
    async fn search_with_brightdata(&self, query: &str, engine: &str) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let search_url = self.build_search_url(engine, query);
        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        // Valid BrightData parameters only
        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown"  // This is valid according to docs
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Search request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData API error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to read response: {}", e)))?;

        Ok(json!({
            "content": content,
            "query": query,
            "engine": engine,
            "search_url": search_url,
            "success": true
        }))
    }

    fn build_search_url(&self, engine: &str, query: &str) -> String {
        let encoded_query = urlencoding::encode(query);
        match engine {
            "bing" => format!("https://www.bing.com/search?q={}", encoded_query),
            "yandex" => format!("https://yandex.com/search/?text={}", encoded_query),
            "duckduckgo" => format!("https://duckduckgo.com/?q={}", encoded_query),
            _ => format!("https://www.google.com/search?q={}", encoded_query),
        }
    }
}
