// src/client.rs - Cleaned up version (removed duplicate tool execution)
use crate::{config::BrightDataConfig, error::BrightDataError};
use reqwest::Client;
use serde_json::Value;
use anyhow::{Result, anyhow};

pub struct BrightDataClient {
    config: BrightDataConfig,
    client: Client,
}

impl BrightDataClient {
    pub fn new(config: BrightDataConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    /// Direct BrightData API call for basic scraping
    pub async fn get(&self, target_url: &str) -> Result<Value, BrightDataError> {
        let payload = serde_json::json!({
            "url": target_url,
        });

        let res = self
            .client
            .post(&self.config.endpoint)
            .header("Authorization", format!("Bearer {}", self.config.token))
            .json(&payload)
            .send()
            .await?
            .json::<Value>()
            .await?;

        Ok(res)
    }
}