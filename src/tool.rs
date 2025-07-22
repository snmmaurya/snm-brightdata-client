// src/tool.rs
use async_trait::async_trait;
use crate::error::BrightDataError;
use serde_json::Value;

#[async_trait]
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, parameters: Value) -> Result<Value, BrightDataError>;
}



pub struct ToolResolver;

impl Default for ToolResolver {
    fn default() -> Self {
        Self
    }
}

impl ToolResolver {
    pub fn resolve(&self, name: &str) -> Option<Box<dyn Tool + Send + Sync>> {
        match name {
            "scrape_website" => Some(Box::new(crate::tools::scrape::ScrapeMarkdown)),
            "search_web" => Some(Box::new(crate::tools::search::SearchEngine)),
            "extract_data" => Some(Box::new(crate::tools::extract::Extractor)),
            _ => None,
        }
    }
}