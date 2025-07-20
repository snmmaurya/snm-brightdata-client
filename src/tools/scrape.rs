// src/tools/scrape.rs
use crate::tool::Tool;
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{Value, json};
use reqwest::Client;

pub struct ScrapeMarkdown;

#[async_trait]
impl Tool for ScrapeMarkdown {
    fn name(&self) -> &str {
        "scrape_website"
    }

    fn description(&self) -> &str {
        "Scrape a webpage and return markdown"
    }

    async fn execute(&self, parameters: Value) -> Result<Value, BrightDataError> {
        let url = parameters
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'url'".into()))?;

        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "snm_rustacean_scraper_unlocker".into());

        let client = Client::new();

        let proxy_username = std::env::var("BRIGHTDATA_PROXY_USERNAME").unwrap_or_default();
        let proxy_password = std::env::var("BRIGHTDATA_PROXY_PASSWORD").unwrap_or_default();
        let session_id = uuid::Uuid::new_v4().to_string();

        let full_url = format!(
            "http://{}-session-{}:{}@{}:{}/scrape",
            proxy_username,
            session_id,
            proxy_password,
            std::env::var("BRIGHTDATA_PROXY_HOST").unwrap_or_else(|_| "zproxy.lum-superproxy.io".into()),
            std::env::var("BRIGHTDATA_PROXY_PORT").unwrap_or_else(|_| "22225".into()),
        );

        let payload = json!({
            "url": url,
            "zone": zone,
            "render": false,
            "markdown": true
        });

        let res = client
            .post(&full_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("HTTP error: {}", e)))?;

        let status = res.status();

        if !status.is_success() {
            let err = res.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData scrape error ({}): {}",
                status,
                err
            )));
        }


        let data: Value = res.json().await.map_err(|e| {
            BrightDataError::ToolError(format!("Failed to parse response: {}", e))
        })?;

        Ok(data)
    }
}
