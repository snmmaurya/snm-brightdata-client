// src/tool.rs

use async_trait::async_trait;
use crate::error::BrightDataError;
use serde_json::Value;
use serde::{Deserialize, Serialize};

// MCP-compatible content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>, // For base64 encoded data like images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>, // MIME type for binary content
}

impl McpContent {
    pub fn text(text: String) -> Self {
        Self {
            content_type: "text".to_string(),
            text,
            data: None,
            media_type: None,
        }
    }

    pub fn image(data: String, media_type: String) -> Self {
        Self {
            content_type: "image".to_string(),
            text: "Screenshot captured".to_string(),
            data: Some(data),
            media_type: Some(media_type),
        }
    }
}

// Enhanced tool result for MCP compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<McpContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    // Preserve backward compatibility - raw value for existing integrations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<Value>,
}

impl ToolResult {
    pub fn success(content: Vec<McpContent>) -> Self {
        Self {
            content,
            is_error: Some(false),
            raw_value: None,
        }
    }

    pub fn success_with_text(text: String) -> Self {
        Self {
            content: vec![McpContent::text(text)],
            is_error: Some(false),
            raw_value: None,
        }
    }

    pub fn success_with_raw(content: Vec<McpContent>, raw: Value) -> Self {
        Self {
            content,
            is_error: Some(false),
            raw_value: Some(raw),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            content: vec![McpContent::text(format!("Error: {}", message))],
            is_error: Some(true),
            raw_value: None,
        }
    }

    // Backward compatibility method
    pub fn from_legacy_value(value: Value) -> Self {
        let text = if let Some(raw_text) = value.get("raw").and_then(|v| v.as_str()) {
            raw_text.to_string()
        } else {
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        };

        Self {
            content: vec![McpContent::text(text)],
            is_error: Some(false),
            raw_value: Some(value),
        }
    }
}

#[async_trait]
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value; // New method for tool schema
    
    // Enhanced execute method returning MCP-compatible result
    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError>;
    
    // Legacy method for backward compatibility
    async fn execute_legacy(&self, parameters: Value) -> Result<Value, BrightDataError> {
        let result = self.execute(parameters).await?;
        if let Some(raw) = result.raw_value {
            Ok(raw)
        } else if !result.content.is_empty() {
            let content_text = &result.content[0].text;
            Ok(serde_json::json!({
                "content": content_text
            }))
        } else {
            Ok(serde_json::json!({}))
        }
    }
}

// Enhanced tool resolver with schema support
pub struct ToolResolver;

impl Default for ToolResolver {
    fn default() -> Self {
        Self
    }
}

impl ToolResolver {
    pub fn resolve(&self, name: &str) -> Option<Box<dyn Tool + Send + Sync>> {
        match name {
            // Existing tools
            "scrape_website" => Some(Box::new(crate::tools::scrape::ScrapeMarkdown)),
            "search_web" => Some(Box::new(crate::tools::search::SearchEngine)),
            "extract_data" => Some(Box::new(crate::tools::extract::Extractor)),
            "get_crypto_data" => Some(Box::new(crate::tools::search::SearchEngine)),
            "take_screenshot" => Some(Box::new(crate::tools::screenshot::ScreenshotTool)),
            "multi_zone_search" => Some(Box::new(crate::tools::multi_zone_search::MultiZoneSearch)),
            // Add these missing tools:
            "get_stock_data" => Some(Box::new(crate::tools::stock::StockDataTool)),
            "get_crypto_data" => Some(Box::new(crate::tools::crypto::CryptoDataTool)),
            "get_etf_data" => Some(Box::new(crate::tools::etf::ETFDataTool)),
            "get_bond_data" => Some(Box::new(crate::tools::bond::BondDataTool)),
            "get_mutual_fund_data" => Some(Box::new(crate::tools::mutual_fund::MutualFundDataTool)),
            "get_commodity_data" => Some(Box::new(crate::tools::commodity::CommodityDataTool)),
            "get_market_overview" => Some(Box::new(crate::tools::market::MarketOverviewTool)),
            _ => None,
        }
    }

    pub fn list_tools(&self) -> Vec<Value> {
        vec![
            serde_json::json!({
                "name": "scrape_website",
                "description": "Scrape a webpage into markdown",
                "inputSchema": {
                    "type": "object",
                    "required": ["url"],
                    "properties": {
                        "url": { "type": "string" },
                        "format": { "type": "string", "enum": ["markdown", "raw"] }
                    }
                }
            }),
            serde_json::json!({
                "name": "search_web",
                "description": "Search the web using BrightData",
                "inputSchema": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string" },
                        "engine": {
                            "type": "string",
                            "enum": ["google", "bing", "yandex", "duckduckgo"]
                        },
                        "cursor": { "type": "string" }
                    }
                }
            }),
            serde_json::json!({
                "name": "extract_data",
                "description": "Extract structured data from a webpage",
                "inputSchema": {
                    "type": "object",
                    "required": ["url"],
                    "properties": {
                        "url": { "type": "string" },
                        "schema": { "type": "object" }
                    }
                }
            }),
            serde_json::json!({
                "name": "take_screenshot",
                "description": "Take a screenshot of a webpage",
                "inputSchema": {
                    "type": "object",
                    "required": ["url"],
                    "properties": {
                        "url": { "type": "string" },
                        "width": { "type": "integer" },
                        "height": { "type": "integer" },
                        "full_page": { "type": "boolean" }
                    }
                }
            }),
            serde_json::json!({
                "name": "get_crypto_data",
                "description": "Crypto search using BrightData",
                "inputSchema": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string" },
                        "engine": {
                            "type": "string",
                            "enum": ["google", "bing", "yandex", "duckduckgo"]
                        },
                        "cursor": { "type": "string" }
                    }
                }
            }),
            serde_json::json!({
                "name": "get_stock_data",
                "description": "Stock search using BrightData",
                "inputSchema": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string" },
                        "engine": {
                            "type": "string",
                            "enum": ["google", "bing", "yandex", "duckduckgo"]
                        },
                        "cursor": { "type": "string" }
                    }
                }
            }),
            serde_json::json!({
                "name": "multi_zone_search",
                "description": "Multi-region search across zones",
                "inputSchema": {
                    "type": "object",
                    "required": ["query", "zones"],
                    "properties": {
                        "query": { "type": "string" },
                        "engine": {
                            "type": "string",
                            "enum": ["google", "bing", "yandex", "duckduckgo"]
                        },
                        "zones": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    }
                }
            })
        ]
    }


    // Helper method to get all available tool names
    pub fn get_available_tool_names(&self) -> Vec<&'static str> {
        vec![
            "scrape_website",
            "search_web", 
            "extract_data",
            "take_screenshot",
            "get_stock_data",
            "get_crypto_data",
            "multi_zone_search"
        ]
    }

    // Helper method to check if a tool exists
    pub fn tool_exists(&self, name: &str) -> bool {
        self.get_available_tool_names().contains(&name)
    }

    // Helper method to get tool count
    pub fn tool_count(&self) -> usize {
        self.get_available_tool_names().len()
    }
}

