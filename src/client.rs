// src/client.rs
use crate::{config::BrightDataConfig, error::BrightDataError};
use reqwest::Client;
use serde_json::Value;

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
