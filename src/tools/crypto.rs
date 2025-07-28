// src/tools/crypto.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

pub struct CryptoDataTool;

#[async_trait]
impl Tool for CryptoDataTool {
    fn name(&self) -> &str {
        "get_crypto_data"
    }

    fn description(&self) -> &str {
        "Get cryptocurrency data including prices, market cap, trading volumes"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Crypto symbol (BTC, ETH, ADA), crypto name (Bitcoin, Ethereum), comparison query (BTC vs ETH), or market overview (crypto market today, top cryptocurrencies)"
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

        let result = self.fetch_crypto_data(query).await?;

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No crypto data found");
        let mcp_content = vec![McpContent::text(format!(
            "₿ **Cryptocurrency Data for {}**\n\n{}",
            query,
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl CryptoDataTool {
    async fn fetch_crypto_data(&self, query: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        // Build crypto-specific search URL
        let search_url = format!(
            "https://www.google.com/search?q={} cryptocurrency price market cap coinmarketcap",
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
            .map_err(|e| BrightDataError::ToolError(format!("Crypto data request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData crypto data error {}: {}",
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