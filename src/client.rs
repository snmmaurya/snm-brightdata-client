// src/client.rs
use crate::{config::BrightDataConfig, error::BrightDataError};
use reqwest::Client;
use serde_json::Value;
use anyhow::{Result, anyhow};
use crate::tool::ToolResolver;

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

    pub async fn run(&self, tool_name: &str, input: Value) -> Result<Value> {
        let resolver = ToolResolver::default();
        let tool = resolver
            .resolve(tool_name)
            .ok_or_else(|| anyhow!("Tool `{tool_name}` not found"))?;
        
        // Use legacy method for backward compatibility
        tool.execute_legacy(input).await.map_err(|e| anyhow!(e))
    }
}