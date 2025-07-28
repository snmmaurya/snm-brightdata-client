// src/tools/bond.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

pub struct BondDataTool;

#[async_trait]
impl Tool for BondDataTool {
    fn name(&self) -> &str {
        "get_bond_data"
    }

    fn description(&self) -> &str {
        "Get bond market data including yields, government bonds, corporate bonds, and bond market trends"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Bond type (government bonds, corporate bonds), yield query (10-year yield), or bond market analysis"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian",
                    "description": "Market region"
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
        
        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let result = self.fetch_bond_data(query, market).await?;

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No bond data found");
        let mcp_content = vec![McpContent::text(format!(
            "🏛️ **Bond Market Data for {}**\n\nMarket: {}\n\n{}",
            query,
            market.to_uppercase(),
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl BondDataTool {
    async fn fetch_bond_data(&self, query: &str, market: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        // Build bond-specific search URL
        let search_url = match market {
            "indian" => format!("https://www.google.com/search?q={} bond yield india government corporate RBI", urlencoding::encode(query)),
            "us" => format!("https://www.google.com/search?q={} bond yield treasury corporate federal reserve", urlencoding::encode(query)),
            "global" => format!("https://www.google.com/search?q={} bond yield global market sovereign", urlencoding::encode(query)),
            _ => format!("https://www.google.com/search?q={} bond yield market", urlencoding::encode(query))
        };

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
            .map_err(|e| BrightDataError::ToolError(format!("Bond data request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData bond data error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        Ok(json!({
            "content": content,
            "query": query,
            "market": market,
            "success": true
        }))
    }
}