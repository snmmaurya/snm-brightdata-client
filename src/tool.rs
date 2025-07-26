// Complete ToolResolver implementation with list_tools method
// Update your src/tool.rs file

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
            Ok(serde_json::json!({
                "content": result.content[0].text
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
            "take_screenshot" => Some(Box::new(crate::tools::screenshot::ScreenshotTool)),
            
            // New financial tools
            "get_stock_data" => Some(Box::new(crate::tools::financial::StockDataTool)),
            "get_crypto_data" => Some(Box::new(crate::tools::financial::CryptoDataTool)),
            "get_etf_data" => Some(Box::new(crate::tools::financial::ETFDataTool)),
            "get_bond_data" => Some(Box::new(crate::tools::financial::BondDataTool)),
            "get_mutual_fund_data" => Some(Box::new(crate::tools::financial::MutualFundDataTool)),
            "get_commodity_data" => Some(Box::new(crate::tools::financial::CommodityDataTool)),
            "get_market_overview" => Some(Box::new(crate::tools::financial::MarketOverviewTool)),
            
            _ => None,
        }
    }

    pub fn list_tools(&self) -> Vec<Value> {
        vec![
            // Existing tools
            serde_json::json!({
                "name": "scrape_website",
                "description": "Scrape a webpage and return markdown content using BrightData Web Unlocker",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to scrape"
                        },
                        "format": {
                            "type": "string",
                            "enum": ["markdown", "raw"],
                            "description": "Output format",
                            "default": "markdown"
                        }
                    },
                    "required": ["url"]
                }
            }),
            serde_json::json!({
                "name": "search_web",
                "description": "Search the web using various search engines via BrightData",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        },
                        "engine": {
                            "type": "string",
                            "enum": ["google", "bing", "yandex", "duckduckgo"],
                            "description": "Search engine to use",
                            "default": "google"
                        },
                        "cursor": {
                            "type": "string",
                            "description": "Pagination cursor/page number",
                            "default": "0"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "extract_data",
                "description": "Extract structured data from a webpage using AI analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to extract data from"
                        },
                        "schema": {
                            "type": "object",
                            "description": "Optional schema to guide extraction",
                            "additionalProperties": true
                        }
                    },
                    "required": ["url"]
                }
            }),
            serde_json::json!({
                "name": "take_screenshot",
                "description": "Take a screenshot of a webpage using BrightData Browser",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to screenshot"
                        },
                        "width": {
                            "type": "integer",
                            "description": "Screenshot width",
                            "default": 1280,
                            "minimum": 320,
                            "maximum": 1920
                        },
                        "height": {
                            "type": "integer",
                            "description": "Screenshot height",
                            "default": 720,
                            "minimum": 240,
                            "maximum": 1080
                        },
                        "full_page": {
                            "type": "boolean",
                            "description": "Capture full page height",
                            "default": false
                        }
                    },
                    "required": ["url"]
                }
            }),

            // Financial tools
            serde_json::json!({
                "name": "get_stock_data",
                "description": "Get comprehensive stock data including prices, performance, market cap, volumes. Use for individual stocks, stock comparisons, or stock market overviews",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Stock symbol (e.g. ASHOKLEY, TCS, AAPL), company name, comparison query (AAPL vs MSFT), or market overview request (today's stock performance, Nifty 50 performance)" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian",
                            "description": "Market region - indian for NSE/BSE stocks, us for NASDAQ/NYSE, global for international"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_crypto_data",
                "description": "Get cryptocurrency data including prices, market cap, trading volumes. Use for individual cryptos, crypto comparisons (BTC vs ETH), or overall crypto market analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Crypto symbol (BTC, ETH, ADA), crypto name (Bitcoin, Ethereum), comparison query (BTC vs ETH), or market overview (crypto market today, top cryptocurrencies)" 
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_etf_data",
                "description": "Get ETF and index fund data including NAV, holdings, performance, expense ratios",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "ETF symbol (SPY, NIFTYBEES), ETF name, or ETF market analysis query" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_bond_data",
                "description": "Get bond market data including yields, government bonds, corporate bonds, and bond market trends",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Bond type (government bonds, corporate bonds), yield query (10-year yield), or bond market analysis" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_mutual_fund_data",
                "description": "Get mutual fund data including NAV, performance, portfolio composition, and fund comparisons",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Fund name, fund symbol, fund category (equity funds, debt funds), or fund comparison query" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_commodity_data",
                "description": "Get commodity prices and market data including gold, silver, oil, agricultural commodities",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Commodity name (gold, silver, crude oil), commodity symbol, or commodity market overview" 
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_market_overview",
                "description": "Get comprehensive market overview including major indices, sector performance, market sentiment, and overall market trends",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "market_type": { 
                            "type": "string", 
                            "enum": ["stocks", "crypto", "bonds", "commodities", "overall"], 
                            "default": "overall",
                            "description": "Type of market overview - overall for general market, or specific asset class"
                        },
                        "region": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": []
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
            "get_etf_data",
            "get_bond_data",
            "get_mutual_fund_data",
            "get_commodity_data",
            "get_market_overview"
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





// // src/tool.rs
// use async_trait::async_trait;
// use crate::error::BrightDataError;
// use serde_json::Value;
// use serde::{Deserialize, Serialize};

// // MCP-compatible content structure
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct McpContent {
//     #[serde(rename = "type")]
//     pub content_type: String,
//     pub text: String,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub data: Option<String>, // For base64 encoded data like images
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub media_type: Option<String>, // MIME type for binary content
// }

// impl McpContent {
//     pub fn text(text: String) -> Self {
//         Self {
//             content_type: "text".to_string(),
//             text,
//             data: None,
//             media_type: None,
//         }
//     }

//     pub fn image(data: String, media_type: String) -> Self {
//         Self {
//             content_type: "image".to_string(),
//             text: "Screenshot captured".to_string(),
//             data: Some(data),
//             media_type: Some(media_type),
//         }
//     }
// }

// // Enhanced tool result for MCP compatibility
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct ToolResult {
//     pub content: Vec<McpContent>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub is_error: Option<bool>,
//     // Preserve backward compatibility - raw value for existing integrations
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub raw_value: Option<Value>,
// }

// impl ToolResult {
//     pub fn success(content: Vec<McpContent>) -> Self {
//         Self {
//             content,
//             is_error: Some(false),
//             raw_value: None,
//         }
//     }

//     pub fn success_with_text(text: String) -> Self {
//         Self {
//             content: vec![McpContent::text(text)],
//             is_error: Some(false),
//             raw_value: None,
//         }
//     }

//     pub fn success_with_raw(content: Vec<McpContent>, raw: Value) -> Self {
//         Self {
//             content,
//             is_error: Some(false),
//             raw_value: Some(raw),
//         }
//     }

//     pub fn error(message: String) -> Self {
//         Self {
//             content: vec![McpContent::text(format!("Error: {}", message))],
//             is_error: Some(true),
//             raw_value: None,
//         }
//     }

//     // Backward compatibility method
//     pub fn from_legacy_value(value: Value) -> Self {
//         let text = if let Some(raw_text) = value.get("raw").and_then(|v| v.as_str()) {
//             raw_text.to_string()
//         } else {
//             serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
//         };

//         Self {
//             content: vec![McpContent::text(text)],
//             is_error: Some(false),
//             raw_value: Some(value),
//         }
//     }
// }

// #[async_trait]
// pub trait Tool {
//     fn name(&self) -> &str;
//     fn description(&self) -> &str;
//     fn input_schema(&self) -> Value; // New method for tool schema
    
//     // Enhanced execute method returning MCP-compatible result
//     async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError>;
    
//     // Legacy method for backward compatibility
//     async fn execute_legacy(&self, parameters: Value) -> Result<Value, BrightDataError> {
//         let result = self.execute(parameters).await?;
//         if let Some(raw) = result.raw_value {
//             Ok(raw)
//         } else if !result.content.is_empty() {
//             Ok(serde_json::json!({
//                 "content": result.content[0].text
//             }))
//         } else {
//             Ok(serde_json::json!({}))
//         }
//     }
// }

// // Enhanced tool resolver with schema support
// pub struct ToolResolver;

// impl Default for ToolResolver {
//     fn default() -> Self {
//         Self
//     }
// }


// // In src/tool.rs
// impl ToolResolver {
//     pub fn resolve(&self, name: &str) -> Option<Box<dyn Tool + Send + Sync>> {
//         match name {
//             // Existing tools
//             "scrape_website" => Some(Box::new(crate::tools::scrape::ScrapeMarkdown)),
//             "search_web" => Some(Box::new(crate::tools::search::SearchEngine)),
//             "extract_data" => Some(Box::new(crate::tools::extract::Extractor)),
//             "take_screenshot" => Some(Box::new(crate::tools::screenshot::ScreenshotTool)),
            
//             // New financial tools (all should work now)
//             "get_stock_data" => Some(Box::new(crate::tools::financial::StockDataTool)),
//             "get_crypto_data" => Some(Box::new(crate::tools::financial::CryptoDataTool)),
//             "get_etf_data" => Some(Box::new(crate::tools::financial::ETFDataTool)),
//             "get_bond_data" => Some(Box::new(crate::tools::financial::BondDataTool)),
//             "get_mutual_fund_data" => Some(Box::new(crate::tools::financial::MutualFundDataTool)),
//             "get_commodity_data" => Some(Box::new(crate::tools::financial::CommodityDataTool)),
//             "get_market_overview" => Some(Box::new(crate::tools::financial::MarketOverviewTool)),
            
//             _ => None,
//         }
//     }
// }


// // impl ToolResolver {
// //     pub fn resolve(&self, name: &str) -> Option<Box<dyn Tool + Send + Sync>> {
// //         match name {
// //             "scrape_website" => Some(Box::new(crate::tools::scrape::ScrapeMarkdown)),
// //             "search_web" => Some(Box::new(crate::tools::search::SearchEngine)),
// //             "extract_data" => Some(Box::new(crate::tools::extract::Extractor)),
// //             "take_screenshot" => Some(Box::new(crate::tools::screenshot::ScreenshotTool)),
// //             _ => None,
// //         }
// //     }

// //     pub fn list_tools(&self) -> Vec<Value> {
// //         vec![
// //             serde_json::json!({
// //                 "name": "scrape_website",
// //                 "description": "Scrape a webpage and return markdown content using BrightData Web Unlocker",
// //                 "inputSchema": {
// //                     "type": "object",
// //                     "properties": {
// //                         "url": {
// //                             "type": "string",
// //                             "description": "The URL to scrape"
// //                         },
// //                         "format": {
// //                             "type": "string",
// //                             "enum": ["markdown", "raw"],
// //                             "description": "Output format",
// //                             "default": "markdown"
// //                         }
// //                     },
// //                     "required": ["url"]
// //                 }
// //             }),
// //             serde_json::json!({
// //                 "name": "search_web",
// //                 "description": "Search the web using various search engines via BrightData",
// //                 "inputSchema": {
// //                     "type": "object",
// //                     "properties": {
// //                         "query": {
// //                             "type": "string",
// //                             "description": "Search query"
// //                         },
// //                         "engine": {
// //                             "type": "string",
// //                             "enum": ["google", "bing", "yandex", "duckduckgo"],
// //                             "description": "Search engine to use",
// //                             "default": "google"
// //                         },
// //                         "cursor": {
// //                             "type": "string",
// //                             "description": "Pagination cursor/page number",
// //                             "default": "0"
// //                         }
// //                     },
// //                     "required": ["query"]
// //                 }
// //             }),
// //             serde_json::json!({
// //                 "name": "extract_data",
// //                 "description": "Extract structured data from a webpage using AI analysis",
// //                 "inputSchema": {
// //                     "type": "object",
// //                     "properties": {
// //                         "url": {
// //                             "type": "string",
// //                             "description": "The URL to extract data from"
// //                         },
// //                         "schema": {
// //                             "type": "object",
// //                             "description": "Optional schema to guide extraction",
// //                             "additionalProperties": true
// //                         }
// //                     },
// //                     "required": ["url"]
// //                 }
// //             }),
// //             serde_json::json!({
// //                 "name": "take_screenshot",
// //                 "description": "Take a screenshot of a webpage using BrightData Browser",
// //                 "inputSchema": {
// //                     "type": "object",
// //                     "properties": {
// //                         "url": {
// //                             "type": "string",
// //                             "description": "The URL to screenshot"
// //                         },
// //                         "width": {
// //                             "type": "integer",
// //                             "description": "Screenshot width",
// //                             "default": 1280,
// //                             "minimum": 320,
// //                             "maximum": 1920
// //                         },
// //                         "height": {
// //                             "type": "integer",
// //                             "description": "Screenshot height",
// //                             "default": 720,
// //                             "minimum": 240,
// //                             "maximum": 1080
// //                         },
// //                         "full_page": {
// //                             "type": "boolean",
// //                             "description": "Capture full page height",
// //                             "default": false
// //                         }
// //                     },
// //                     "required": ["url"]
// //                 }
// //             })
// //         ]
// //     }
// // }