// src/config.rs
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct BrightDataConfig {
    pub endpoint: String,
    pub token: String,
}

impl BrightDataConfig {
    pub fn new(endpoint: String, token: String) -> Result<Self> {
        Ok(Self { endpoint, token })
    }

    pub fn from_env() -> Result<Self> {
        let endpoint = std::env::var("BRIGHTDATA_ENDPOINT")
            .unwrap_or_else(|_| "https://brd.superproxy.io".to_string());
        let token = std::env::var("BRIGHTDATA_TOKEN")?;
        Ok(Self { endpoint, token })
    }
}