// src/tools/financial.rs - PATCHED: Enhanced with priority-aware filtering and token budget management
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;
use std::collections::HashMap;
use log::{info, warn, error};
use chrono::Utc;

// ENHANCED: Priority-aware shared BrightData client for all financial tools
async fn scrape_via_brightdata_with_priority(
    url: &str, 
    query: &str,
    tool_type: &str,
    priority: crate::filters::strategy::QueryPriority,
    token_budget: usize,
    execution_id: &str
) -> Result<Value, BrightDataError> {
    info!("🌐 Priority {} BrightData request: {} (execution: {})", 
          format!("{:?}", priority), url, execution_id);
    
    let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
        .or_else(|_| std::env::var("API_TOKEN"))
        .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

    let base_url = std::env::var("BRIGHTDATA_BASE_URL")
        .unwrap_or_else(|| "https://api.brightdata.com".to_string());

    let zone = std::env::var("WEB_UNLOCKER_ZONE")
        .unwrap_or_else(|| "default".to_string());

    let mut payload = json!({
        "url": url,
        "zone": zone,
        "format": "raw",
        "data_format": "markdown"
    });

    // Add priority processing hints
    if std::env::var("TRUNCATE_FILTER")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false) {
        
        payload["processing_priority"] = json!(format!("{:?}", priority));
        payload["token_budget"] = json!(token_budget);
        payload["tool_type"] = json!(tool_type);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(90))
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
            error!("❌ Request failed: {}", e);
            BrightDataError::ToolError(format!("Request failed: {}", e))
        })?;

    let status = response.status().as_u16();
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    // Log BrightData request
    if let Err(e) = JSON_LOGGER.log_brightdata_request(
        execution_id,
        &zone,
        url,
        payload.clone(),
        status,
        response_headers,
        "markdown"
    ).await {
        warn!("Failed to log BrightData request: {}", e);
    }

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        error!("❌ HTTP Error {}: {}", status, error_text);
        return Err(BrightDataError::ToolError(format!(
            "HTTP error {}: {}", status, error_text
        )));
    }

    let raw_content = response.text().await
        .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

    // Apply filters conditionally based on environment variable with priority awareness
    let filtered_content = if std::env::var("TRUNCATE_FILTER")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false) {
        
        if ResponseFilter::is_error_page(&raw_content) {
            return Err(BrightDataError::ToolError("Request returned error page".into()));
        } else if ResponseStrategy::should_try_next_source(&raw_content) {
            return Err(BrightDataError::ToolError("Content quality too low".into()));
        } else {
            // Use token budget aware extraction
            let max_tokens = token_budget / 2; // Reserve tokens for formatting
            ResponseFilter::extract_high_value_financial_data(&raw_content, max_tokens)
        }
    } else {
        raw_content.clone()
    };

    info!("✅ Request completed - {} chars (filtered: {} chars)", 
          raw_content.len(), filtered_content.len());
    
    Ok(json!({
        "content": filtered_content,
        "raw_content": raw_content,
        "url": url,
        "query": query,
        "tool_type": tool_type,
        "priority": format!("{:?}", priority),
        "token_budget": token_budget,
        "execution_id": execution_id,
        "success": true,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

// Legacy wrapper for compatibility
async fn scrape_via_brightdata(url: &str, query: &str) -> Result<Value, BrightDataError> {
    let execution_id = format!("legacy_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));
    let priority = crate::filters::strategy::QueryPriority::Medium;
    let token_budget = 5000;
    scrape_via_brightdata_with_priority(url, query, "legacy", priority, token_budget, &execution_id).await
}

// Stock Data Tool
pub struct StockDataTool;

#[async_trait]
impl Tool for StockDataTool {
    fn name(&self) -> &str {
        "get_stock_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive stock data including prices, performance, market cap, volumes with intelligent filtering"
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

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, "stock", "validation", json!({}), response_type));
            }

            // Budget check for stock queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "stock", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("stock_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        info!("📊 Priority {} stock query: '{}' (market: {}, execution: {})", 
              format!("{:?}", query_priority), query, market, execution_id);

        let url = self.build_stock_url(query, market);
        
        match scrape_via_brightdata_with_priority(&url, query, "stock", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "stock", query, "stock", market, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "📈 **Stock Data for: {}**\n\nMarket: {} | Priority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        query, market, query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl StockDataTool {
    fn build_stock_url(&self, query: &str, market: &str) -> String {
        let query_lower = query.to_lowercase();
        let timestamp = Utc::now().timestamp();
        
        // Market overview queries
        if query_lower.contains("market") || query_lower.contains("overview") {
            return match market {
                "us" => format!("https://finance.yahoo.com/markets/stocks?t={}", timestamp),
                "indian" => format!("https://www.moneycontrol.com/markets/indian-indices/?t={}", timestamp),
                _ => format!("https://finance.yahoo.com/world-indices?t={}", timestamp),
            };
        }
        
        // Specific company mappings
        if query_lower.contains("ashok leyland") || query_lower.contains("ashokley") {
            return format!("https://finance.yahoo.com/quote/ASHOKLEY.NS?t={}", timestamp);
        }
        if query_lower.contains("tcs") || query_lower.contains("tata consultancy") {
            return format!("https://finance.yahoo.com/quote/TCS.NS?t={}", timestamp);
        }
        if query_lower.contains("reliance") {
            return format!("https://finance.yahoo.com/quote/RELIANCE.NS?t={}", timestamp);
        }
        
        // Symbol-based lookup
        let clean_query = query.to_uppercase();
        match market {
            "us" => format!("https://finance.yahoo.com/quote/{}?t={}", clean_query, timestamp),
            "indian" => format!("https://finance.yahoo.com/quote/{}.NS?t={}", clean_query, timestamp),
            _ => format!("https://finance.yahoo.com/quote/{}?t={}", clean_query, timestamp),
        }
    }
}

// Crypto Data Tool
pub struct CryptoDataTool;

#[async_trait]
impl Tool for CryptoDataTool {
    fn name(&self) -> &str {
        "get_crypto_data"
    }

    fn description(&self) -> &str {
        "Get cryptocurrency data including prices, market cap, trading volumes with intelligent filtering"
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

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, "crypto", "validation", json!({}), response_type));
            }

            // Budget check for crypto queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "crypto", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("crypto_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        info!("💰 Priority {} crypto query: '{}' (execution: {})", 
              format!("{:?}", query_priority), query, execution_id);

        let url = self.build_crypto_url(query);
        
        match scrape_via_brightdata_with_priority(&url, query, "crypto", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "crypto", query, "crypto", "CoinMarketCap", content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "💰 **Crypto Data for: {}**\n\nPriority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        query, query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl CryptoDataTool {
    fn build_crypto_url(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();
        let timestamp = Utc::now().timestamp();
        
        if query_lower.contains("bitcoin") || query_lower.contains("btc") {
            format!("https://coinmarketcap.com/currencies/bitcoin/?t={}", timestamp)
        } else if query_lower.contains("ethereum") || query_lower.contains("eth") {
            format!("https://coinmarketcap.com/currencies/ethereum/?t={}", timestamp)
        } else if query_lower.contains("market") || query_lower.contains("overview") {
            format!("https://coinmarketcap.com/?t={}", timestamp)
        } else {
            format!("https://coinmarketcap.com/search/?q={}&t={}", urlencoding::encode(query), timestamp)
        }
    }
}

// ETF Data Tool
pub struct ETFDataTool;

#[async_trait]
impl Tool for ETFDataTool {
    fn name(&self) -> &str {
        "get_etf_data"
    }

    fn description(&self) -> &str {
        "Get ETF and index fund data including NAV, holdings, performance, expense ratios with intelligent filtering"
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

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, "etf", "validation", json!({}), response_type));
            }

            // Budget check for ETF queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "etf", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("etf_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        let url = self.build_etf_url(query, market);
        
        match scrape_via_brightdata_with_priority(&url, query, "etf", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "etf", query, "etf", market, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "📊 **ETF Data for: {}**\n\nMarket: {} | Priority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        query, market, query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl ETFDataTool {
    fn build_etf_url(&self, query: &str, market: &str) -> String {
        let timestamp = Utc::now().timestamp();
        match market {
            "us" => format!("https://finance.yahoo.com/quote/{}?t={}", query, timestamp),
            "indian" => format!("https://finance.yahoo.com/quote/{}.NS?t={}", query, timestamp),
            _ => format!("https://finance.yahoo.com/quote/{}?t={}", query, timestamp),
        }
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
        "Get bond market data including yields, government bonds, corporate bonds, and bond market trends with intelligent filtering"
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

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, "bond", "validation", json!({}), response_type));
            }

            // Budget check for bond queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "bond", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("bond_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        let url = format!("https://www.investing.com/rates-bonds/?t={}", Utc::now().timestamp());
        
        match scrape_via_brightdata_with_priority(&url, query, "bond", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "bond", query, "bond", market, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "🏛️ **Bond Data for: {}**\n\nMarket: {} | Priority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        query, market, query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
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
        "Get mutual fund data including NAV, performance, portfolio composition, and fund comparisons with intelligent filtering"
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

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, "mutual_fund", "validation", json!({}), response_type));
            }

            // Budget check for mutual fund queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "mutual_fund", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("mf_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        let url = format!("https://www.valueresearchonline.com/funds/?t={}", Utc::now().timestamp());
        
        match scrape_via_brightdata_with_priority(&url, query, "mutual_fund", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "mutual_fund", query, "mutual_fund", "ValueResearch", content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "💼 **Mutual Fund Data for: {}**\n\nPriority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        query, query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
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
        "Get commodity prices and market data including gold, silver, oil, agricultural commodities with intelligent filtering"
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

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, "commodity", "validation", json!({}), response_type));
            }

            // Budget check for commodity queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "commodity", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("commodity_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        let url = format!("https://www.investing.com/commodities/?t={}", Utc::now().timestamp());
        
        match scrape_via_brightdata_with_priority(&url, query, "commodity", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "commodity", query, "commodity", "Investing.com", content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "🥇 **Commodity Data for: {}**\n\nPriority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        query, query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
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
        "Get comprehensive market overview including major indices, sector performance, market sentiment, and overall market trends with intelligent filtering"
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

        let query = format!("{} market overview", market_type);

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(&query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(&query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", &query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", &query, "market_overview", "validation", json!({}), response_type));
            }

            // Budget check for market overview queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 150 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", &query, "market_overview", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("market_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        let url = format!("https://www.moneycontrol.com/?t={}", Utc::now().timestamp());
        
        match scrape_via_brightdata_with_priority(&url, &query, "market_overview", query_priority, recommended_tokens, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "market_overview", &query, "market", region, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "🌍 **Market Overview: {} Market ({})**\n\nPriority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        market_type.to_uppercase(), region.to_uppercase(), query_priority, recommended_tokens, execution_id, content
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };

                // Apply size limits only if filtering enabled
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(&query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}