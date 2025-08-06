// src/client.rs - Enhanced version with improved config and error handling
use crate::{config::BrightDataConfig, error::BrightDataError};
use reqwest::Client;
use serde_json::Value;

pub struct BrightDataClient {
    config: BrightDataConfig,
    client: Client,
}

impl BrightDataClient {
    pub fn new(config: BrightDataConfig) -> Result<Self, BrightDataError> {
        // Validate configuration
        config.validate().map_err(|e| BrightDataError::ConfigError(e.to_string()))?;
        
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| BrightDataError::NetworkError(e.to_string()))?;

        Ok(Self { config, client })
    }

    /// Direct BrightData API call for basic scraping
    pub async fn get(&self, target_url: &str) -> Result<Value, BrightDataError> {
        let payload = serde_json::json!({
            "url": target_url,
            "zone": self.config.web_unlocker_zone,
            "format": "raw",
        });

        let res = self
            .client
            .post(&format!("{}/request", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::Request(e))?;

        let status = res.status();
        if !status.is_success() {
            let error_text = res.text().await.unwrap_or_default();
            return Err(BrightDataError::ApiError(format!(
                "BrightData API error {}: {}",
                status.as_u16(),
                error_text
            )));
        }

        let result = res.json::<Value>().await
            .map_err(|e| BrightDataError::ParseError(e.to_string()))?;

        Ok(result)
    }

    /// Search using BrightData SERP API
    pub async fn search(&self, query: &str, engine: &str) -> Result<Value, BrightDataError> {
        let search_url = self.build_search_url(engine, query);
        
        let payload = serde_json::json!({
            "url": search_url,
            "zone": self.config.serp_zone,
            "format": "raw",
            "data_format": "markdown"
        });

        let res = self
            .client
            .post(&format!("{}/request", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::Request(e))?;

        let status = res.status();
        if !status.is_success() {
            let error_text = res.text().await.unwrap_or_default();
            return Err(BrightDataError::ApiError(format!(
                "BrightData SERP API error {}: {}",
                status.as_u16(),
                error_text
            )));
        }

        let result = res.json::<Value>().await
            .map_err(|e| BrightDataError::ParseError(e.to_string()))?;

        Ok(result)
    }

    /// Take screenshot using BrightData Browser
    pub async fn screenshot(&self, url: &str, width: u32, height: u32) -> Result<Value, BrightDataError> {
        let payload = serde_json::json!({
            "url": url,
            "zone": self.config.browser_zone,
            "format": "raw",
            "data_format": "screenshot",
            "viewport": {
                "width": width,
                "height": height
            }
        });

        let res = self
            .client
            .post(&format!("{}/request", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::Request(e))?;

        let status = res.status();
        if !status.is_success() {
            let error_text = res.text().await.unwrap_or_default();
            return Err(BrightDataError::ApiError(format!(
                "BrightData Browser API error {}: {}",
                status.as_u16(),
                error_text
            )));
        }

        let result = res.json::<Value>().await
            .map_err(|e| BrightDataError::ParseError(e.to_string()))?;

        Ok(result)
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