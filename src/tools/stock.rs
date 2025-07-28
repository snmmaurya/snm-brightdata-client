// src/tools/stock.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

pub struct StockDataTool;

#[async_trait]
impl Tool for StockDataTool {
    fn name(&self) -> &str {
        "get_stock_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive stock data including prices, performance, market cap, volumes with intelligent URL selection"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Stock symbol (e.g. TATAMOTORS, TCS, AAPL), company name, comparison query, or market overview request"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian",
                    "description": "Market region - indian for NSE/BSE stocks, us for NASDAQ/NYSE, global for international"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;
        
        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        // Try multiple approaches with fallbacks
        let result = self.fetch_stock_data_with_fallbacks(query, market).await?;

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No stock data found");
        let source_used = result.get("source_used").and_then(|s| s.as_str()).unwrap_or("Unknown");
        let url_used = result.get("url_used").and_then(|u| u.as_str()).unwrap_or("");

        let mcp_content = vec![McpContent::text(format!(
            "📈 **Stock Data for {}**\n\nMarket: {}\nData Source: {}\nURL: {}\n\n{}",
            query,
            market.to_uppercase(),
            source_used,
            url_used,
            content_text
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl StockDataTool {
    async fn fetch_stock_data_with_fallbacks(&self, query: &str, market: &str) -> Result<Value, BrightDataError> {
        let urls_to_try = self.build_prioritized_urls(query, market);
        let mut last_error = None;

        for (url, source_name) in urls_to_try {
            match self.try_fetch_url(&url, query, market, &source_name).await {
                Ok(mut result) => {
                    // Add metadata about which source worked
                    result["source_used"] = json!(source_name);
                    result["url_used"] = json!(url);
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    // Log and continue to next URL
                    log::warn!("Failed to fetch from {}: {:?}", source_name, last_error);
                }
            }
        }

        // If all URLs failed, return the last error
        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("All stock data sources failed".into())))
    }

    fn build_prioritized_urls(&self, query: &str, market: &str) -> Vec<(String, String)> {
        let mut urls = Vec::new();
        let clean_query = query.trim().to_uppercase();

        // Priority 1: Direct Yahoo Finance URLs for stock symbols
        if self.is_likely_stock_symbol(&clean_query) {
            match market {
                "indian" => {
                    // Try common Indian stock symbol formats
                    let symbols_to_try = vec![
                        format!("{}.NS", clean_query),  // NSE
                        format!("{}.BO", clean_query),  // BSE
                        clean_query.clone(),            // Raw symbol
                    ];
                    
                    for symbol in symbols_to_try {
                        urls.push((
                            format!("https://finance.yahoo.com/quote/{}/", symbol),
                            format!("Yahoo Finance ({})", symbol)
                        ));
                    }
                }
                "us" => {
                    urls.push((
                        format!("https://finance.yahoo.com/quote/{}/", clean_query),
                        format!("Yahoo Finance ({})", clean_query)
                    ));
                }
                "global" => {
                    urls.push((
                        format!("https://finance.yahoo.com/quote/{}/", clean_query),
                        format!("Yahoo Finance Global ({})", clean_query)
                    ));
                }
                _ => {}
            }
        }

        // Priority 2: Targeted search on Yahoo Finance
        urls.push((
            format!("https://finance.yahoo.com/lookup?s={}", urlencoding::encode(query)),
            "Yahoo Finance Search".to_string()
        ));

        // Priority 3: Google search with Yahoo Finance site filter
        let yahoo_search = match market {
            "indian" => format!(
                "https://www.google.com/search?q={} stock price NSE BSE india site:finance.yahoo.com",
                urlencoding::encode(query)
            ),
            "us" => format!(
                "https://www.google.com/search?q={} stock price NASDAQ NYSE site:finance.yahoo.com",
                urlencoding::encode(query)
            ),
            _ => format!(
                "https://www.google.com/search?q={} stock price site:finance.yahoo.com",
                urlencoding::encode(query)
            )
        };
        urls.push((yahoo_search, "Google Search (Yahoo Finance)".to_string()));

        // Priority 4: General Google search
        let general_search = match market {
            "indian" => format!(
                "https://www.google.com/search?q={} stock price NSE BSE india financial data",
                urlencoding::encode(query)
            ),
            "us" => format!(
                "https://www.google.com/search?q={} stock price NASDAQ NYSE financial data",
                urlencoding::encode(query)
            ),
            _ => format!(
                "https://www.google.com/search?q={} stock price financial data",
                urlencoding::encode(query)
            )
        };
        urls.push((general_search, "Google Search (General)".to_string()));

        urls
    }

    async fn try_fetch_url(&self, url: &str, query: &str, market: &str, source_name: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        let payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown"
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(90)) // Shorter timeout for faster fallbacks
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Request failed for {}: {}", source_name, e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "{} returned {}: {}",
                source_name, status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Basic validation: check if content contains stock-related keywords
        if !self.validate_stock_content(&content) {
            return Err(BrightDataError::ToolError(format!(
                "{} returned non-stock content",
                source_name
            )));
        }

        Ok(json!({
            "content": content,
            "query": query,
            "market": market,
            "success": true
        }))
    }

    fn is_likely_stock_symbol(&self, query: &str) -> bool {
        let clean = query.trim();
        
        // Must be reasonable length and mostly alphanumeric
        if clean.len() < 1 || clean.len() > 15 {
            return false;
        }

        // Should be mostly letters/numbers, allow dots for exchanges
        let valid_chars = clean.chars().all(|c| c.is_alphanumeric() || c == '.');
        
        // Should have at least some alphabetic characters
        let has_letters = clean.chars().any(|c| c.is_alphabetic());
        
        valid_chars && has_letters
    }

    fn validate_stock_content(&self, content: &str) -> bool {
        let content_lower = content.to_lowercase();
        
        // Check for stock-related keywords
        let stock_keywords = [
            "stock", "share", "price", "market", "trading", "volume",
            "market cap", "dividend", "earnings", "pe ratio", "financial",
            "nasdaq", "nyse", "nse", "bse", "ticker", "quote"
        ];

        stock_keywords.iter().any(|keyword| content_lower.contains(keyword))
    }

    // Helper method for debugging - shows what URLs would be tried
    pub fn debug_urls(&self, query: &str, market: &str) -> Vec<String> {
        self.build_prioritized_urls(query, market)
            .into_iter()
            .map(|(url, source)| format!("{}: {}", source, url))
            .collect()
    }
}