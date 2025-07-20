// src/tools/extract.rs
use crate::tool::Tool;
use crate::error::BrightDataError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};
use std::env;

pub struct Extractor;

#[async_trait]
impl Tool for Extractor {
    fn name(&self) -> &str {
        "extract"
    }

    fn description(&self) -> &str {
        "Extract structured data from page using markdown + AI"
    }

    async fn execute(&self, parameters: Value) -> Result<Value, BrightDataError> {
        let url = parameters
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'url'".into()))?;

        // Build proxy URL with auth
        let proxy_username = env::var("BRIGHTDATA_PROXY_USERNAME").unwrap_or_default();
        let proxy_password = env::var("BRIGHTDATA_PROXY_PASSWORD").unwrap_or_default();
        let proxy_host = env::var("BRIGHTDATA_PROXY_HOST").unwrap_or_else(|_| "zproxy.lum-superproxy.io".into());
        let proxy_port = env::var("BRIGHTDATA_PROXY_PORT").unwrap_or_else(|_| "22225".into());

        let proxy_url = format!("http://{}:{}@{}:{}", proxy_username, proxy_password, proxy_host, proxy_port);

        let client = Client::builder()
            .proxy(reqwest::Proxy::http(&proxy_url).map_err(|e| BrightDataError::ToolError(e.to_string()))?)
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let res = client
            .get(url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Request failed: {}", e)))?;

        let status = res.status();
        if !status.is_success() {
            let err = res.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "HTTP {}: {}",
                status, err
            )));
        }

        let html = res.text().await.map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // NOTE: This is where you would parse and extract markdown from HTML if needed
        Ok(json!({ "content": html }))
    }
}
