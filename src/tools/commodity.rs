// src/tools/commodity.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

pub struct CommodityDataTool;

#[async_trait]
impl Tool for CommodityDataTool {
    fn name(&self) -> &str {
        "get_commodity_data"
    }

    fn description(&self) -> &str {
        "Get commodity prices and market data including gold, silver, oil, agricultural commodities"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Commodity name (gold, silver, crude oil), commodity symbol, or commodity market overview"
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

        let result = self.fetch_commodity_data(query).await?;

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No commodity data found");
        let mcp_content = vec![McpContent::text(format!(
            "🥇 **Commodity Market Data for {}**\n\n{}",
            query,
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl CommodityDataTool {
    async fn fetch_commodity_data(&self, query: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        // Build commodity-specific search URL
        let search_url = format!(
            "https://www.google.com/search?q={} commodity price market futures trading today",
            urlencoding::encode(query)
        );

        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown"
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Commodity data request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData commodity data error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        Ok(json!({
            "content": content,
            "query": query,
            "success": true
        }))
    }
}