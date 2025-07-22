// src/tools/search.rs
use crate::tool::Tool;
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;

pub struct SearchEngine;

#[async_trait]
impl Tool for SearchEngine {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search via engine (google, bing, yandex, duckduckgo) and return markdown results"
    }

    async fn execute(&self, parameters: Value) -> Result<Value, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query'".into()))?;

        let engine = parameters.get("engine").and_then(|v| v.as_str()).unwrap_or("google");
        let cursor = parameters.get("cursor").and_then(|v| v.as_str()).unwrap_or("0");

        let url = build_search_url(engine, query, cursor);

        let body = json!({
            "url": url,
            "zone": std::env::var("WEB_UNLOCKER_ZONE").unwrap_or_else(|_| "default".into()),
            "format": "raw",
            "data_format": "markdown"
        });

        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;
        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_BASE_URL".into()))?;

        let url = format!("{}/request", base_url);

        let client = Client::new();

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Request failed: {}", e)))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Invalid response: {}", e)))?;

        if !status.is_success() {
            return Err(BrightDataError::ToolError(format!(
                "BrightData error {}: {}",
                status, text
            )));
        }

        Ok(json!({ "raw": text }))
    }
}

fn build_search_url(engine: &str, query: &str, cursor: &str) -> String {
    let encoded = urlencoding::encode(query);
    let page: usize = cursor.parse().unwrap_or(0);
    let start = page * 10;

    match engine {
        "bing" => format!("https://www.bing.com/search?q={}&first={}", encoded, start + 1),
        "yandex" => format!("https://yandex.com/search/?text={}&p={}", encoded, page),
        "duckduckgo" => format!("https://duckduckgo.com/?q={}&s={}", encoded, start),
        _ => format!("https://www.google.com/search?q={}&start={}", encoded, start),
    }
}
