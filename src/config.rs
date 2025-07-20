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
}