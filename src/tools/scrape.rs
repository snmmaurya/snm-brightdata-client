// ===== src/tools/scrape.rs - CORRECTED VERSION =====
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{Value, json};
use reqwest::Client;
use std::time::Duration;

pub struct ScrapeMarkdown;

#[async_trait]
impl Tool for ScrapeMarkdown {
    fn name(&self) -> &str {
        "scrape_website"
    }

    fn description(&self) -> &str {
        "Scrape a webpage using BrightData Web Unlocker"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to scrape"
                },
                "format": {
                    "type": "string",
                    "enum": ["raw", "markdown"],
                    "description": "Output format",
                    "default": "raw"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let url = parameters
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'url' parameter".into()))?;

        let format = parameters
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("raw");

        let result = self.scrape_with_brightdata(url, format).await?;
        
        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No content");
        let mcp_content = vec![McpContent::text(format!(
            "🌐 **Scraped from {}**\n\n{}",
            url,
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl ScrapeMarkdown {
    async fn scrape_with_brightdata(&self, url: &str, format: &str) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        // Valid BrightData parameters only
        let mut payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw"  // Always use "raw" format
        });

        // Add markdown conversion if requested
        if format == "markdown" {
            payload["data_format"] = json!("markdown");
        }

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
            .map_err(|e| BrightDataError::ToolError(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData API error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        Ok(json!({
            "content": content,
            "url": url,
            "format": format,
            "success": true
        }))
    }
}