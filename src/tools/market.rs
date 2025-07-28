// src/tools/market.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

pub struct MarketOverviewTool;

#[async_trait]
impl Tool for MarketOverviewTool {
    fn name(&self) -> &str {
        "get_market_overview"
    }

    fn description(&self) -> &str {
        "Get comprehensive market overview including major indices, sector performance, market sentiment, and overall market trends"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "market_type": {
                    "type": "string",
                    "enum": ["stocks", "crypto", "bonds", "commodities", "overall"],
                    "default": "overall",
                    "description": "Type of market overview - overall for general market, or specific asset class"
                },
                "region": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian",
                    "description": "Market region"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let market_type = parameters
            .get("market_type")
            .and_then(|v| v.as_str())
            .unwrap_or("overall");
        
        let region = parameters
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let result = self.fetch_market_overview(market_type, region).await?;

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No market data found");
        let mcp_content = vec![McpContent::text(format!(
            "📊 **Market Overview - {} Market ({})**\n\n{}",
            market_type.to_uppercase(),
            region.to_uppercase(),
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl MarketOverviewTool {
    async fn fetch_market_overview(&self, market_type: &str, region: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        // Build market overview search URL
        let search_url = match (region, market_type) {
            ("indian", "stocks") => "https://www.google.com/search?q=indian stock market today nifty sensex BSE NSE performance".to_string(),
            ("indian", "crypto") => "https://www.google.com/search?q=cryptocurrency market india bitcoin ethereum price today".to_string(),
            ("indian", "bonds") => "https://www.google.com/search?q=indian bond market government bonds yield RBI today".to_string(),
            ("indian", "commodities") => "https://www.google.com/search?q=commodity market india gold silver oil prices today".to_string(),
            ("indian", "overall") => "https://www.google.com/search?q=indian financial market overview today nifty sensex rupee".to_string(),
            
            ("us", "stocks") => "https://www.google.com/search?q=US stock market today dow jones s&p nasdaq performance".to_string(),
            ("us", "crypto") => "https://www.google.com/search?q=cryptocurrency market USA bitcoin ethereum price today".to_string(),
            ("us", "bonds") => "https://www.google.com/search?q=US bond market treasury yield federal reserve today".to_string(),
            ("us", "commodities") => "https://www.google.com/search?q=commodity market USA gold oil prices futures today".to_string(),
            ("us", "overall") => "https://www.google.com/search?q=US financial market overview today wall street performance".to_string(),
            
            ("global", "stocks") => "https://www.google.com/search?q=global stock market overview world indices performance today".to_string(),
            ("global", "crypto") => "https://www.google.com/search?q=global cryptocurrency market bitcoin ethereum worldwide today".to_string(),
            ("global", "bonds") => "https://www.google.com/search?q=global bond market sovereign yields worldwide today".to_string(),
            ("global", "commodities") => "https://www.google.com/search?q=global commodity market gold oil silver prices worldwide".to_string(),
            ("global", "overall") => "https://www.google.com/search?q=global financial market overview world economy today".to_string(),
            
            _ => format!("https://www.google.com/search?q={} {} market overview today", region, market_type)
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
            .map_err(|e| BrightDataError::ToolError(format!("Market overview request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData market overview error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        Ok(json!({
            "content": content,
            "market_type": market_type,
            "region": region,
            "success": true
        }))
    }
}