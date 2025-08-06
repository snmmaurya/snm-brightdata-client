// src/config.rs - Enhanced version with complete BrightData configuration
use anyhow::Result;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct BrightDataConfig {
    pub endpoint: String,
    pub token: String,
    pub base_url: String,
    pub web_unlocker_zone: String,
    pub browser_zone: String,
    pub serp_zone: String,
    pub timeout: Duration,
    pub max_retries: u32,
}

impl BrightDataConfig {
    pub fn new(endpoint: String, token: String) -> Result<Self> {
        Ok(Self {
            endpoint,
            token,
            base_url: "https://api.brightdata.com".to_string(),
            web_unlocker_zone: "default".to_string(),
            browser_zone: "default_browser".to_string(),
            serp_zone: "serp_api2".to_string(),
            timeout: Duration::from_secs(300),
            max_retries: 3,
        })
    }

    pub fn from_env() -> Result<Self> {
        let endpoint = std::env::var("BRIGHTDATA_ENDPOINT")
            .unwrap_or_else(|_| "https://brd.superproxy.io".to_string());
        let token = std::env::var("BRIGHTDATA_TOKEN")?;
        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());
        let web_unlocker_zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());
        let browser_zone = std::env::var("BROWSER_ZONE")
            .unwrap_or_else(|_| "default_browser".to_string());
        let serp_zone = std::env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());
        let timeout = Duration::from_secs(
            std::env::var("REQUEST_TIMEOUT")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300)
        );
        let max_retries = std::env::var("MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3);

        Ok(Self {
            endpoint,
            token,
            base_url,
            web_unlocker_zone,
            browser_zone,
            serp_zone,
            timeout,
            max_retries,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.token.is_empty() {
            return Err(anyhow::anyhow!("BrightData token is required"));
        }
        if self.web_unlocker_zone.is_empty() {
            return Err(anyhow::anyhow!("Web Unlocker zone is required"));
        }
        if self.browser_zone.is_empty() {
            return Err(anyhow::anyhow!("Browser zone is required"));
        }
        if self.serp_zone.is_empty() {
            return Err(anyhow::anyhow!("SERP zone is required"));
        }
        Ok(())
    }
}