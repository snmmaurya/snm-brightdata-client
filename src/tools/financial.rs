// Fixed financial tools with proper imports and missing methods
// Update your src/tools/financial.rs

use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;
use log::{info, warn, error, debug};
use chrono::{Utc, Datelike, Timelike}; // Fixed imports

// Stock Data Tool with Enhanced Logging
pub struct StockDataTool;

#[async_trait]
impl Tool for StockDataTool {
    fn name(&self) -> &str {
        "get_stock_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive stock data including prices, performance, market cap, volumes"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Stock symbol, company name, comparison query, or market overview request"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
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

        info!("🔍 StockDataTool EXECUTION STARTED");
        info!("📊 Input Query: '{}'", query);
        info!("🌍 Market: '{}'", market);

        // Use the smart resolver instead of simple URL building
        let result = self.resolve_and_fetch(query, market).await?;

        // Rest of your execute method...
        let mcp_content = vec![McpContent::text(format!(
            "📈 **Stock Data for: {}**\n\nMarket: {}\n\n{}",
            query,
            market,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }

    // async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
    //     let query = parameters
    //         .get("query")
    //         .and_then(|v| v.as_str())
    //         .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

    //     let market = parameters
    //         .get("market")
    //         .and_then(|v| v.as_str())
    //         .unwrap_or("indian");

    //     info!("🔍 StockDataTool EXECUTION STARTED");
    //     info!("📊 Input Query: '{}'", query);
    //     info!("🌍 Market: '{}'", market);

    //     // Intelligent URL selection based on query
    //     let target_url = self.select_best_url_for_query(query, market);
    //     info!("🎯 Selected Target URL: {}", target_url);

    //     let result = self.scrape_via_brightdata(&target_url, query).await?;

    //     let content_preview = result.get("content")
    //         .and_then(|c| c.as_str())
    //         .map(|s| if s.len() > 200 { &s[..200] } else { s })
    //         .unwrap_or("No content");

    //     info!("📈 StockDataTool EXECUTION COMPLETED");
    //     info!("📝 Content Preview: {}", content_preview);

    //     let mcp_content = vec![McpContent::text(format!(
    //         "📈 **Stock Data for: {}**\n\nMarket: {}\nSource: {}\n\n{}",
    //         query,
    //         market,
    //         target_url,
    //         result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
    //     ))];

    //     Ok(ToolResult::success_with_raw(mcp_content, result))
    // }
}

// Dynamic stock symbol resolution approach

// impl StockDataTool {
//     fn select_best_url_for_query(&self, query: &str, market: &str) -> String {
//         let query_lower = query.to_lowercase();
//         debug!("🔍 Analyzing query: '{}'", query_lower);
        
//         // Strategy 1: Use Yahoo Finance search/lookup endpoint first
//         if self.looks_like_symbol(&query) {
//             // Direct symbol lookup
//             return self.build_quote_url(&query, market);
//         }
        
//         // Strategy 2: Use Yahoo Finance search to resolve company names
//         let search_url = format!("https://finance.yahoo.com/lookup?s={}", 
//             urlencoding::encode(query));
//         info!("🔍 Using Yahoo search to resolve: {}", search_url);
//         return search_url;
//     }
    
//     fn looks_like_symbol(&self, query: &str) -> bool {
//         // Check if query looks like a stock symbol (3-5 uppercase letters)
//         query.len() >= 2 && query.len() <= 6 && 
//         query.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
//     }
    
//     fn build_quote_url(&self, symbol: &str, market: &str) -> String {
//         let clean_symbol = symbol.to_uppercase();
        
//         match market {
//             "indian" => {
//                 if clean_symbol.contains('.') {
//                     // Already has suffix
//                     format!("https://finance.yahoo.com/quote/{}", clean_symbol)
//                 } else {
//                     // Try NSE first (.NS), fallback to BSE (.BO) if needed
//                     format!("https://finance.yahoo.com/quote/{}.NS", clean_symbol)
//                 }
//             },
//             "us" => {
//                 format!("https://finance.yahoo.com/quote/{}", clean_symbol)
//             },
//             _ => {
//                 format!("https://finance.yahoo.com/quote/{}", clean_symbol)
//             }
//         }
//     }
    
//     // Alternative approach: Multi-step resolution
//     async fn resolve_and_fetch(&self, query: &str, market: &str) -> Result<Value, BrightDataError> {
//         // Step 1: Try direct symbol lookup if it looks like a symbol
//         if self.looks_like_symbol(query) {
//             let direct_url = self.build_quote_url(query, market);
//             info!("🎯 Trying direct symbol lookup: {}", direct_url);
            
//             match self.scrape_via_brightdata(&direct_url, query).await {
//                 Ok(result) => {
//                     // Check if we got valid data or a "not found" page
//                     if self.is_valid_stock_data(&result) {
//                         return Ok(result);
//                     }
//                 }
//                 Err(_) => {
//                     info!("❌ Direct lookup failed, trying search...");
//                 }
//             }
//         }
        
//         // Step 2: Use search endpoint to find the correct symbol
//         let search_url = format!("https://finance.yahoo.com/lookup?s={}", 
//             urlencoding::encode(query));
//         info!("🔍 Searching for stock: {}", search_url);
        
//         let search_result = self.scrape_via_brightdata(&search_url, query).await?;
        
//         // Try to extract the correct symbol from search results
//         if let Some(symbol) = self.extract_symbol_from_search_results(&search_result) {
//             let final_url = self.build_quote_url(&symbol, market);
//             info!("✅ Found symbol '{}', fetching data: {}", symbol, final_url);
//             return self.scrape_via_brightdata(&final_url, query).await;
//         }
        
//         // Step 3: Fallback to search results
//         Ok(search_result)
//     }
    
//     fn is_valid_stock_data(&self, result: &Value) -> bool {
//         if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
//             let content_lower = content.to_lowercase();
//             // Check for indicators of valid stock data
//             content_lower.contains("price") || 
//             content_lower.contains("market cap") ||
//             content_lower.contains("volume") ||
//             content_lower.contains("last price") ||
//             (content_lower.contains("₹") || content_lower.contains("$"))
//         } else {
//             false
//         }
//     }
    
//     fn extract_symbol_from_search_results(&self, result: &Value) -> Option<String> {
//         if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
//             // Look for patterns like: "ASHOKLEY.NS" or similar in the search results
//             // This is a simplified regex-like approach
//             for line in content.lines().take(50) { // Check first 50 lines
//                 if line.contains("quote/") {
//                     // Extract symbol from URLs like "/quote/ASHOKLEY.NS"
//                     if let Some(start) = line.find("quote/") {
//                         let after_quote = &line[start + 6..];
//                         if let Some(end) = after_quote.find(&['"', '\'', ' ', '?'][..]) {
//                             let symbol = &after_quote[..end];
//                             if symbol.len() >= 2 && symbol.len() <= 15 {
//                                 info!("🎯 Extracted symbol from search: {}", symbol);
//                                 return Some(symbol.to_string());
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//         None
//     }
// }


// Complete integration - update your existing StockDataTool implementation

impl StockDataTool {
    // Your existing execute method - UPDATE THIS to use the new resolver
    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        info!("🔍 StockDataTool EXECUTION STARTED");
        info!("📊 Input Query: '{}'", query);
        info!("🌍 Market: '{}'", market);

        // USE THE NEW SMART RESOLVER instead of select_best_url_for_query
        let result = self.resolve_and_fetch(query, market).await?;

        let content_preview = result.get("content")
            .and_then(|c| c.as_str())
            .map(|s| if s.len() > 200 { &s[..200] } else { s })
            .unwrap_or("No content");

        info!("📈 StockDataTool EXECUTION COMPLETED");
        info!("📝 Content Preview: {}", content_preview);

        let mcp_content = vec![McpContent::text(format!(
            "📈 **Stock Data for: {}**\n\nMarket: {}\n\n{}",
            query,
            market,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }

    // NEW METHODS (add these to your existing impl block):
    
    async fn resolve_and_fetch(&self, query: &str, market: &str) -> Result<Value, BrightDataError> {
        // Step 1: Try direct symbol lookup if it looks like a symbol
        if self.looks_like_symbol(query) {
            let direct_url = self.build_quote_url(query, market);
            info!("🎯 Trying direct symbol lookup: {}", direct_url);
            
            match self.scrape_via_brightdata(&direct_url, query).await {
                Ok(result) => {
                    // Check if we got valid data or a "not found" page
                    if self.is_valid_stock_data(&result) {
                        return Ok(result);
                    }
                }
                Err(_) => {
                    info!("❌ Direct lookup failed, trying search...");
                }
            }
        }
        
        // Step 2: Use search endpoint to find the correct symbol
        let search_url = format!("https://finance.yahoo.com/lookup?s={}", 
            urlencoding::encode(query));
        info!("🔍 Searching for stock: {}", search_url);
        
        let search_result = self.scrape_via_brightdata(&search_url, query).await?;
        
        // Try to extract the correct symbol from search results
        if let Some(symbol) = self.extract_symbol_from_search_results(&search_result) {
            let final_url = self.build_quote_url(&symbol, market);
            info!("✅ Found symbol '{}', fetching data: {}", symbol, final_url);
            return self.scrape_via_brightdata(&final_url, query).await;
        }
        
        // Step 3: Fallback to search results
        Ok(search_result)
    }
    
    fn looks_like_symbol(&self, query: &str) -> bool {
        // Check if query looks like a stock symbol (3-5 uppercase letters)
        query.len() >= 2 && query.len() <= 6 && 
        query.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
    }
    
    fn build_quote_url(&self, symbol: &str, market: &str) -> String {
        let clean_symbol = symbol.to_uppercase();
        
        match market {
            "indian" => {
                println!("I am here INDIA: {:?}", clean_symbol);
                if clean_symbol.contains('.') {
                    // Already has suffix
                    format!("https://finance.yahoo.com/quote/{}.BS", clean_symbol)
                } else {
                    // Try NSE first (.NS), fallback to BSE (.BO) if needed
                    format!("https://finance.yahoo.com/quote/{}.NS", clean_symbol)
                }

            },
            "us" => {
                println!("I am here US: {:?}", clean_symbol);
                format!("https://finance.yahoo.com/quote/{}", clean_symbol)
                 
            },
            _ => {
                println!("I am here NONE: {:?}", clean_symbol);
                format!("https://finance.yahoo.com/quote/{}", clean_symbol)
                 
            }
        }
    }
    
    fn is_valid_stock_data(&self, result: &Value) -> bool {
        if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
            let content_lower = content.to_lowercase();
            // Check for indicators of valid stock data
            content_lower.contains("price") || 
            content_lower.contains("market cap") ||
            content_lower.contains("volume") ||
            content_lower.contains("last price") ||
            (content_lower.contains("₹") || content_lower.contains("$"))
        } else {
            false
        }
    }
    
    fn extract_symbol_from_search_results(&self, result: &Value) -> Option<String> {
        if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
            // Look for patterns like: "ASHOKLEY.NS" or similar in the search results
            // This is a simplified regex-like approach
            for line in content.lines().take(50) { // Check first 50 lines
                if line.contains("quote/") {
                    // Extract symbol from URLs like "/quote/ASHOKLEY.NS"
                    if let Some(start) = line.find("quote/") {
                        let after_quote = &line[start + 6..];
                        if let Some(end) = after_quote.find(&['"', '\'', ' ', '?'][..]) {
                            let symbol = &after_quote[..end];
                            if symbol.len() >= 2 && symbol.len() <= 15 {
                                info!("🎯 Extracted symbol from search: {}", symbol);
                                return Some(symbol.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    // KEEP YOUR EXISTING scrape_via_brightdata method (the one that works with the API)
    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        info!("🌐 BRIGHT DATA REQUEST STARTED");
        info!("🔗 Target URL: {}", url);
        info!("⏰ Request Time: {}", Utc::now().to_rfc3339());
        
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        info!("⚙️ Bright Data Config:");
        info!("   Base URL: {}", base_url);
        info!("   Zone: {}", zone);
        info!("   API Token: {}***", &api_token[..8.min(api_token.len())]);

        // USE THE WORKING PAYLOAD FORMAT (from your successful tests)
        let payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown"
        });

        info!("📦 Request Payload: {}", serde_json::to_string_pretty(&payload).unwrap_or_default());

        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let bright_data_url = format!("{}/request", base_url);
        info!("🚀 Sending request to: {}", bright_data_url);

        let response = client
            .post(&bright_data_url)
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("❌ Request failed: {}", e);
                BrightDataError::ToolError(format!("Request failed: {}", e))
            })?;

        let status = response.status();
        info!("📊 Response Status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("❌ HTTP Error {}: {}", status, error_text);
            return Err(BrightDataError::ToolError(format!(
                "HTTP error {}: {}", status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| {
                error!("❌ Failed to read response: {}", e);
                BrightDataError::ToolError(e.to_string())
            })?;

        info!("✅ BRIGHT DATA REQUEST COMPLETED");
        info!("📏 Content Length: {} characters", content.len());
        
        Ok(json!({
            "content": content,
            "url": url,
            "query": query,
            "success": true,
            "content_length": content.len(),
            "timestamp": Utc::now().to_rfc3339()
        }))
    }

    // REMOVE OR COMMENT OUT the old select_best_url_for_query method
    // since we're now using resolve_and_fetch instead
}

// impl StockDataTool {
//     fn select_best_url_for_query(&self, query: &str, market: &str) -> String {
//         let query_lower = query.to_lowercase();
//         debug!("🔍 Analyzing query: '{}'", query_lower);
        
//         // Add cache busting timestamp
//         let timestamp = Utc::now().timestamp();
//         let cache_buster = format!("t={}", timestamp);
        
//         // Check if it's a market overview query
//         if query_lower.contains("market") && (query_lower.contains("today") || query_lower.contains("performance")) {
//             let url = match market {
//                 "us" => format!("https://finance.yahoo.com/markets/stocks?{}", cache_buster),
//                 "indian" => format!("https://www.moneycontrol.com/markets/indian-indices/?{}", cache_buster),
//                 _ => format!("https://finance.yahoo.com/world-indices?{}", cache_buster),
//             };
//             info!("🌍 Market overview query detected → {}", url);
//             return url;
//         }
//         // Check for specific company names and map to symbols
//         else if query_lower.contains("ashok leyland") || query_lower.contains("ashokley") {
//             let url = match market {
//                 "us" => format!("https://finance.yahoo.com/quote/ASHOKLEY?{}", cache_buster),
//                 "indian" => format!("https://finance.yahoo.com/quote/ASHOKLEY.NS?{}", cache_buster),
//                 _ => format!("https://finance.yahoo.com/quote/ASHOKLEY.NS?{}", cache_buster),
//             };
//             info!("🚛 Ashok Leyland company detected → {}", url);
//             return url;
//         }
//         else if query_lower.contains("tcs") || query_lower.contains("tata consultancy") {
//             let url = format!("https://finance.yahoo.com/quote/TCS.NS?{}", cache_buster);
//             info!("💻 TCS company detected → {}", url);
//             return url;
//         }
//         else if query_lower.contains("reliance") && !query_lower.contains("mutual") {
//             let url = format!("https://finance.yahoo.com/quote/RELIANCE.NS?{}", cache_buster);
//             info!("⛽ Reliance company detected → {}", url);
//             return url;
//         }
//         // Check if it's a specific symbol
//         else if let Some(symbol) = self.extract_symbol_from_query(query) {
//             let url = match market {
//                 "us" => format!("https://finance.yahoo.com/quote/{}?{}", symbol, cache_buster),
//                 "indian" => format!("https://finance.yahoo.com/quote/{}.NS?{}", symbol, cache_buster),
//                 _ => format!("https://finance.yahoo.com/quote/{}?{}", symbol, cache_buster),
//             };
//             info!("💼 Symbol '{}' detected → {}", symbol, url);
//             return url;
//         }
//         // Default to search-based approach
//         else {
//             let url = format!("https://finance.yahoo.com/lookup?s={}&{}", urlencoding::encode(query), cache_buster);
//             warn!("❓ No specific pattern matched, using search → {}", url);
//             return url;
//         }
//     }

//     // Fixed: Add the missing extract_symbol_from_query method
//     fn extract_symbol_from_query(&self, query: &str) -> Option<String> {
//         // Simple symbol extraction - look for 3-5 uppercase letters
//         let words: Vec<&str> = query.split_whitespace().collect();
//         for word in words {
//             if word.len() >= 3 && word.len() <= 5 && word.chars().all(|c| c.is_ascii_uppercase()) {
//                 debug!("🔤 Symbol extracted: '{}'", word);
//                 return Some(word.to_string());
//             }
//         }
//         debug!("🚫 No symbol pattern found in query");
//         None
//     }

//     async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
//         info!("🌐 BRIGHT DATA REQUEST STARTED");
//         info!("🔗 Target URL: {}", url);
//         info!("⏰ Request Time: {}", Utc::now().to_rfc3339());
        
//         let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
//             .or_else(|_| std::env::var("API_TOKEN"))
//             .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

//         let base_url = std::env::var("BRIGHTDATA_BASE_URL")
//             .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

//         let zone = std::env::var("WEB_UNLOCKER_ZONE")
//             .unwrap_or_else(|_| "default".to_string());

//         info!("⚙️ Bright Data Config:");
//         info!("   Base URL: {}", base_url);
//         info!("   Zone: {}", zone);
//         info!("   API Token: {}***", &api_token[..8.min(api_token.len())]);

//         // Enhanced payload with real-time data options
//         let payload = json!({
//             "url": url,
//             "zone": zone,
//             "format": "raw",
//             "data_format": "markdown"
//         });
//         // let payload = json!({
//         //     "url": url,
//         //     "zone": zone,
//         //     "format": "raw",
//         //     "data_format": "markdown",
//         //     "session": format!("stock_data_{}", Utc::now().timestamp()),
//         //     // "country": "IN", // or "US" based on market
//         //     "render_js": true, // Ensure JavaScript is rendered for dynamic content
//         //     "wait_for": 2000, // Wait 2 seconds for dynamic content to load
//         //     // "block_resources": ["image", "media", "font"], // Block unnecessary resources for faster loading
//         //     // "custom_headers": {
//         //     //     "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
//         //     //     "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
//         //     //     "Accept-Language": "en-US,en;q=0.5",
//         //     //     "Accept-Encoding": "gzip, deflate",
//         //     //     "Cache-Control": "no-cache, no-store, must-revalidate",
//         //     //     "Pragma": "no-cache"
//         //     // }
//         // });

//         info!("📦 Enhanced Request Payload: {}", serde_json::to_string_pretty(&payload).unwrap_or_default());

//         let client = Client::builder()
//             .timeout(Duration::from_secs(90)) // Increased timeout for real-time data
//             .build()
//             .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

//         let bright_data_url = format!("{}/request", base_url);
//         info!("🚀 Sending request to: {}", bright_data_url);

//         let response = client
//             .post(&bright_data_url)
//             .header("Authorization", format!("Bearer {}", api_token))
//             .header("Content-Type", "application/json")
//             .header("X-Real-Time", "true") // Custom header to indicate real-time request
//             .json(&payload)
//             .send()
//             .await
//             .map_err(|e| {
//                 error!("❌ Request failed: {}", e);
//                 BrightDataError::ToolError(format!("Request failed: {}", e))
//             })?;

//         let status = response.status();
//         info!("📊 Response Status: {}", status);

//         if !status.is_success() {
//             let error_text = response.text().await.unwrap_or_default();
//             error!("❌ HTTP Error {}: {}", status, error_text);
//             return Err(BrightDataError::ToolError(format!(
//                 "HTTP error {}: {}", status, error_text
//             )));
//         }

//         let content = response.text().await
//             .map_err(|e| {
//                 error!("❌ Failed to read response: {}", e);
//                 BrightDataError::ToolError(e.to_string())
//             })?;

//         info!("✅ BRIGHT DATA REQUEST COMPLETED");
//         info!("📏 Content Length: {} characters", content.len());
//         info!("⏰ Response Time: {}", Utc::now().to_rfc3339());
        
//         // Enhanced data validation
//         let data_freshness = self.validate_data_freshness(&content);
//         info!("🔄 Data Freshness Assessment: {}", data_freshness);
        
//         // Check if we got actual data or just HTML structure
//         if content.contains("<!DOCTYPE html>") || content.contains("<html") {
//             warn!("⚠️ Received HTML response - might be search page instead of data");
//         }
//         if content.contains("Skip to main content") || content.contains("Google Search") {
//             warn!("⚠️ Received Google search page - not actual stock data");
//         }
//         if content.contains("stock") || content.contains("price") || content.contains("$") || content.contains("₹") {
//             info!("✅ Content appears to contain financial data");
//         } else {
//             warn!("⚠️ Content might not contain relevant financial data");
//         }
        
//         Ok(json!({
//             "content": content,
//             "url": url,
//             "query": query,
//             "success": true,
//             "content_length": content.len(),
//             "timestamp": Utc::now().to_rfc3339(),
//             "data_freshness": data_freshness,
//             "cache_busted": url.contains("t=")
//         }))
//     }

//     fn validate_data_freshness(&self, content: &str) -> String {
//         let content_lower = content.to_lowercase();
        
//         // Check for real-time indicators
//         if content_lower.contains("real time") || content_lower.contains("live") {
//             return "Real-time".to_string();
//         }
        
//         // Check for recent timestamps
//         if content_lower.contains("minute ago") || content_lower.contains("seconds ago") {
//             return "Very Fresh (< 1 minute)".to_string();
//         }
        
//         if content_lower.contains("minutes ago") {
//             return "Fresh (< 5 minutes)".to_string();
//         }
        
//         // Check if market is open
//         let is_market_hours = self.is_market_hours_now();
//         if is_market_hours && (content_lower.contains("price") || content_lower.contains("$") || content_lower.contains("₹")) {
//             return "Current Session Data".to_string();
//         }
        
//         // Check for today's date
//         let today = Utc::now().format("%Y-%m-%d").to_string();
//         if content.contains(&today) {
//             return "Today's Data".to_string();
//         }
        
//         "Unknown Freshness".to_string()
//     }

//     // Fixed: Simplified market hours check without chrono_tz dependency
//     fn is_market_hours_now(&self) -> bool {
//         let now = Utc::now();
        
//         // Convert UTC to IST (UTC + 5:30)
//         let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
//         let ist_now = now.with_timezone(&ist_offset);
        
//         let ist_hour = ist_now.hour();
//         let ist_minute = ist_now.minute();
        
//         // Market hours: 9:15 AM - 3:30 PM IST
//         let market_start = 9 * 60 + 15; // 9:15 AM in minutes
//         let market_end = 15 * 60 + 30;  // 3:30 PM in minutes
//         let current_time = ist_hour * 60 + ist_minute;
        
//         // Check if it's a weekday
//         let weekday = ist_now.weekday();
//         let is_weekday = weekday.num_days_from_monday() < 5;
        
//         is_weekday && current_time >= market_start && current_time <= market_end
//     }
// }

// Crypto Data Tool with Enhanced Logging
pub struct CryptoDataTool;

#[async_trait]
impl Tool for CryptoDataTool {
    fn name(&self) -> &str {
        "get_crypto_data"
    }

    fn description(&self) -> &str {
        "Get cryptocurrency data including prices, market cap, trading volumes"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Crypto symbol, name, comparison query, or market overview"
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

        info!("🔍 CryptoDataTool EXECUTION STARTED");
        info!("💰 Input Query: '{}'", query);

        let target_url = self.select_crypto_url_for_query(query);
        info!("🎯 Selected Target URL: {}", target_url);

        let result = self.scrape_via_brightdata(&target_url, query).await?;

        let content_preview = result.get("content")
            .and_then(|c| c.as_str())
            .map(|s| if s.len() > 200 { &s[..200] } else { s })
            .unwrap_or("No content");

        info!("💰 CryptoDataTool EXECUTION COMPLETED");
        info!("📝 Content Preview: {}", content_preview);

        let mcp_content = vec![McpContent::text(format!(
            "💰 **Crypto Data for: {}**\n\nSource: {}\n\n{}",
            query,
            target_url,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl CryptoDataTool {
    fn select_crypto_url_for_query(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();
        debug!("🔍 Analyzing crypto query: '{}'", query_lower);
        
        // Add cache busting timestamp for crypto (24/7 market)
        let timestamp = Utc::now().timestamp();
        let cache_buster = format!("t={}", timestamp);
        
        if query_lower.contains("vs") || query_lower.contains("comparison") {
            let url = format!("https://coinmarketcap.com/?{}", cache_buster);
            info!("⚖️ Comparison query detected → {}", url);
            url
        } else if query_lower.contains("market") && query_lower.contains("overview") {
            let url = format!("https://coinmarketcap.com/?{}", cache_buster);
            info!("🌍 Market overview query detected → {}", url);
            url
        } else if query_lower.contains("bitcoin") || query_lower.contains("btc") {
            let url = format!("https://coinmarketcap.com/currencies/bitcoin/?{}", cache_buster);
            info!("₿ Bitcoin detected → {}", url);
            url
        } else if query_lower.contains("ethereum") || query_lower.contains("eth") {
            let url = format!("https://coinmarketcap.com/currencies/ethereum/?{}", cache_buster);
            info!("⟠ Ethereum detected → {}", url);
            url
        } else {
            let url = format!("https://coinmarketcap.com/search/?q={}&{}", urlencoding::encode(query), cache_buster);
            info!("🔍 Generic crypto search → {}", url);
            url
        }
    }

    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        // Same implementation as StockDataTool but with crypto-specific logging
        info!("🌐 CRYPTO BRIGHT DATA REQUEST STARTED");
        info!("🔗 Target URL: {}", url);
        
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        let payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown"
        });

        info!("📦 Crypto Request Payload: {}", serde_json::to_string_pretty(&payload).unwrap_or_default());

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("❌ Crypto request failed: {}", e);
                BrightDataError::ToolError(format!("Request failed: {}", e))
            })?;

        let status = response.status();
        info!("📊 Crypto Response Status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("❌ Crypto HTTP Error {}: {}", status, error_text);
            return Err(BrightDataError::ToolError(format!(
                "HTTP error {}: {}", status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        info!("✅ CRYPTO BRIGHT DATA REQUEST COMPLETED");
        info!("📏 Crypto Content Length: {} characters", content.len());

        Ok(json!({
            "content": content,
            "url": url,
            "query": query,
            "success": true
        }))
    }
}

// Add the remaining financial tools with the same pattern
// ETF Data Tool
pub struct ETFDataTool;

#[async_trait]
impl Tool for ETFDataTool {
    fn name(&self) -> &str {
        "get_etf_data"
    }

    fn description(&self) -> &str {
        "Get ETF and index fund data including NAV, holdings, performance, expense ratios"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "ETF symbol, name, or ETF market analysis query"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
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

        let target_url = self.select_etf_url_for_query(query, market);
        let result = self.scrape_via_brightdata(&target_url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "📊 **ETF Data for: {}**\n\nMarket: {}\nSource: {}\n\n{}",
            query,
            market,
            target_url,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl ETFDataTool {
    fn select_etf_url_for_query(&self, query: &str, market: &str) -> String {
        let timestamp = Utc::now().timestamp();
        let cache_buster = format!("t={}", timestamp);
        
        let base_url = match market {
            "us" => format!("https://finance.yahoo.com/quote/{}?{}", query, cache_buster),
            "indian" => format!("https://finance.yahoo.com/quote/{}.NS?{}", query, cache_buster),
            _ => format!("https://finance.yahoo.com/quote/{}?{}", query, cache_buster),
        };
        
        info!("📊 ETF URL generated: {}", base_url);
        base_url
    }

    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        // Simplified implementation
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing API token".into()))?;

        let payload = json!({
            "url": url,
            "zone": "default",
            "format": "raw",
            "data_format": "markdown"
        });

        let client = Client::new();
        let response = client
            .post("https://api.brightdata.com/request")
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&payload)
            .send()
            .await?;

        let content = response.text().await?;

        Ok(json!({
            "content": content,
            "url": url,
            "success": true
        }))
    }
}

// Bond Data Tool
pub struct BondDataTool;

#[async_trait]
impl Tool for BondDataTool {
    fn name(&self) -> &str {
        "get_bond_data"
    }

    fn description(&self) -> &str {
        "Get bond market data including yields, government bonds, corporate bonds, and bond market trends"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Bond type, yield query, or bond market analysis"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
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

        let target_url = format!("https://www.investing.com/rates-bonds/?t={}", Utc::now().timestamp());
        let result = self.scrape_via_brightdata(&target_url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "🏛️ **Bond Data for: {}**\n\nMarket: {}\nSource: {}\n\n{}",
            query,
            market,
            target_url,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl BondDataTool {
    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        // Simplified implementation - same pattern as ETFDataTool
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing API token".into()))?;

        let payload = json!({
            "url": url,
            "zone": "default",
            "format": "raw"
        });

        let client = Client::new();
        let response = client
            .post("https://api.brightdata.com/request")
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&payload)
            .send()
            .await?;

        let content = response.text().await?;

        Ok(json!({
            "content": content,
            "url": url,
            "success": true
        }))
    }
}

// Mutual Fund Data Tool
pub struct MutualFundDataTool;

#[async_trait]
impl Tool for MutualFundDataTool {
    fn name(&self) -> &str {
        "get_mutual_fund_data"
    }

    fn description(&self) -> &str {
        "Get mutual fund data including NAV, performance, portfolio composition, and fund comparisons"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Fund name, symbol, category, or fund comparison query"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
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

        let target_url = format!("https://www.valueresearchonline.com/funds/?t={}", Utc::now().timestamp());
        let result = self.scrape_via_brightdata(&target_url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "💼 **Mutual Fund Data for: {}**\n\nSource: {}\n\n{}",
            query,
            target_url,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl MutualFundDataTool {
    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        // Same simplified pattern
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing API token".into()))?;

        let payload = json!({ "url": url, "zone": "default", "format": "raw" });
        let client = Client::new();
        let response = client
            .post("https://api.brightdata.com/request")
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&payload)
            .send()
            .await?;

        let content = response.text().await?;
        Ok(json!({ "content": content, "url": url, "success": true }))
    }
}

// Commodity Data Tool
pub struct CommodityDataTool;

#[async_trait]
impl Tool for CommodityDataTool {
    fn name(&self) -> &str {
        "get_commodity_data"
    }

    fn description(&self) -> &str {
        "Get commodity prices and market data including gold, silver, oil, agricultural commodities"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Commodity name, symbol, or commodity market overview"
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

        let target_url = format!("https://www.investing.com/commodities/?t={}", Utc::now().timestamp());
        let result = self.scrape_via_brightdata(&target_url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "🥇 **Commodity Data for: {}**\n\nSource: {}\n\n{}",
            query,
            target_url,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl CommodityDataTool {
    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing API token".into()))?;

        let payload = json!({ "url": url, "zone": "default", "format": "raw" });
        let client = Client::new();
        let response = client
            .post("https://api.brightdata.com/request")
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&payload)
            .send()
            .await?;

        let content = response.text().await?;
        Ok(json!({ "content": content, "url": url, "success": true }))
    }
}

// Market Overview Tool
pub struct MarketOverviewTool;

#[async_trait]
impl Tool for MarketOverviewTool {
    fn name(&self) -> &str {
        "get_market_overview"
    }

    fn description(&self) -> &str {
        "Get comprehensive market overview including major indices, sector performance, market sentiment, and overall market trends"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "market_type": {
                    "type": "string",
                    "enum": ["stocks", "crypto", "bonds", "commodities", "overall"],
                    "default": "overall",
                    "description": "Type of market overview"
                },
                "region": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let market_type = parameters
            .get("market_type")
            .and_then(|v| v.as_str())
            .unwrap_or("overall");

        let region = parameters
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let target_url = format!("https://www.moneycontrol.com/?t={}", Utc::now().timestamp());
        let result = self.scrape_via_brightdata(&target_url, &format!("{} market overview", market_type)).await?;

        let mcp_content = vec![McpContent::text(format!(
            "🌍 **Market Overview: {} Market ({})**\n\nSource: {}\n\n{}",
            market_type.to_uppercase(),
            region.to_uppercase(),
            target_url,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl MarketOverviewTool {
    async fn scrape_via_brightdata(&self, url: &str, query: &str) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing API token".into()))?;

        let payload = json!({ "url": url, "zone": "default", "format": "raw" });
        let client = Client::new();
        let response = client
            .post("https://api.brightdata.com/request")
            .header("Authorization", format!("Bearer {}", api_token))
            .json(&payload)
            .send()
            .await?;

        let content = response.text().await?;
        Ok(json!({ "content": content, "url": url, "success": true }))
    }
}







