// src/tools/stock.rs - COMPLETE PATCHED VERSION with enhanced token budget management
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use crate::logger::JSON_LOGGER;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use log::{info, warn};

pub struct StockDataTool;

#[async_trait]
impl Tool for StockDataTool {
    fn name(&self) -> &str {
        "get_stock_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive stock data including prices, performance, market cap, volumes with intelligent filtering and priority-based processing"
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
                },
                "data_type": {
                    "type": "string",
                    "enum": ["price", "fundamentals", "technical", "news", "all"],
                    "default": "all",
                    "description": "Type of stock data to focus on"
                },
                "timeframe": {
                    "type": "string",
                    "enum": ["realtime", "day", "week", "month", "quarter", "year"],
                    "default": "realtime",
                    "description": "Time period for stock data analysis"
                },
                "include_ratios": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include financial ratios like P/E, P/B, ROE"
                },
                "include_volume": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include trading volume and liquidity data"
                }
            },
            "required": ["query"]
        })
    }

    // FIXED: Add the missing execute method that delegates to execute_internal
    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        self.execute_internal(parameters).await
    }

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;
        
        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let data_type = parameters
            .get("data_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let timeframe = parameters
            .get("timeframe")
            .and_then(|v| v.as_str())
            .unwrap_or("realtime");

        let include_ratios = parameters
            .get("include_ratios")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_volume = parameters
            .get("include_volume")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, market, "validation", json!({}), response_type));
            }

            // Budget check for stock queries  
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, market, "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("stock_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        info!("📈 Stock query: '{}' (market: {}, priority: {:?}, tokens: {})", 
              query, market, query_priority, recommended_tokens);
        
        match self.fetch_stock_data_with_fallbacks_and_priority(
            query, market, data_type, timeframe, include_ratios, include_volume,
            query_priority, recommended_tokens, &execution_id
        ).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let source_used = result.get("source_used").and_then(|s| s.as_str()).unwrap_or("Unknown");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    // Use filtering strategy
                    ResponseStrategy::create_financial_response(
                        "stock", query, market, source_used, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "📈 **Stock Data for: {}**\n\nMarket: {} | Data Type: {} | Timeframe: {} | Priority: {:?} | Tokens: {}\nSource: {} | Ratios: {} | Volume: {}\nExecution ID: {}\n\n{}",
                        query, market, data_type, timeframe, query_priority, recommended_tokens, 
                        source_used, include_ratios, include_volume, execution_id, content
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
                    Err(e) // Return original error if filtering disabled
                }
            }
        }
    }
}

impl StockDataTool {
    // ENHANCED: Priority-aware stock data fetching with token management
    async fn fetch_stock_data_with_fallbacks_and_priority(
        &self, 
        query: &str, 
        market: &str, 
        data_type: &str,
        timeframe: &str,
        include_ratios: bool,
        include_volume: bool,
        query_priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let urls_to_try = self.build_prioritized_urls_with_priority(query, market, data_type, query_priority);
        let mut last_error = None;

        for (sequence, (url, source_name)) in urls_to_try.iter().enumerate() {
            match self.try_fetch_url_with_priority_logging(
                url, query, market, source_name, query_priority, token_budget, execution_id, sequence as u64
            ).await {
                Ok(mut result) => {
                    let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    
                    // Use strategy to determine if we should try next source only if filtering enabled
                    if std::env::var("TRUNCATE_FILTER")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false) {
                        
                        if ResponseStrategy::should_try_next_source(content) {
                            warn!("Content quality too low from {}, trying next source", source_name);
                            last_error = Some(BrightDataError::ToolError(format!(
                                "{} returned low-quality content", source_name
                            )));
                            continue;
                        }
                    }
                    
                    result["source_used"] = json!(source_name);
                    result["url_used"] = json!(url);
                    result["execution_id"] = json!(execution_id);
                    result["priority"] = json!(format!("{:?}", query_priority));
                    result["token_budget"] = json!(token_budget);
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    warn!("Failed to fetch from {}: {:?}", source_name, last_error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("All stock data sources failed".into())))
    }

    // ENHANCED: Priority-aware URL fetching with comprehensive token management
    async fn try_fetch_url_with_priority_logging(
        &self, 
        url: &str, 
        query: &str, 
        market: &str, 
        source_name: &str, 
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
        sequence: u64
    ) -> Result<Value, BrightDataError> {
        let start_time = Instant::now();
        
        info!("📈 Priority {} stock data fetch from {} (execution: {})", 
              format!("{:?}", priority), source_name, execution_id);
        
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

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
            .map_err(|e| BrightDataError::ToolError(format!("Request failed for {}: {}", source_name, e)))?;

        let duration = start_time.elapsed();
        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            
            // Log failed call
            if let Err(e) = JSON_LOGGER.log_brightdata_request(
                &format!("{}_{}", execution_id, sequence),
                &zone,
                url,
                payload,
                status,
                response_headers,
                "markdown",
            ).await {
                warn!("Failed to log BrightData request: {}", e);
            }
            
            return Err(BrightDataError::ToolError(format!(
                "{} returned {}: {}",
                source_name, status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply priority-aware filters conditionally based on environment variable
        let filtered_content = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            // Apply filtering only if enabled
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError(format!(
                    "{} returned error page", source_name
                )));
            } else if ResponseFilter::is_mostly_navigation(&raw_content) {
                return Err(BrightDataError::ToolError(format!(
                    "{} returned mostly navigation content", source_name
                )));
            } else {
                // Use enhanced extraction with token budget awareness
                let max_tokens = token_budget / 2; // Reserve tokens for formatting
                ResponseFilter::extract_high_value_financial_data(&raw_content, max_tokens)
            }
        } else {
            // No filtering if disabled
            raw_content.clone()
        };

        // Log successful call
        if let Err(e) = JSON_LOGGER.log_brightdata_request(
            &format!("{}_{}", execution_id, sequence),
            &zone,
            url,
            payload,
            status,
            response_headers,
            "markdown",
        ).await {
            warn!("Failed to log BrightData request: {}", e);
        }

        Ok(json!({
            "content": filtered_content,
            "raw_content": raw_content,
            "query": query,
            "market": market,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }

    // ENHANCED: Priority-aware URL building with token considerations
    fn build_prioritized_urls_with_priority(
        &self, 
        query: &str, 
        market: &str, 
        data_type: &str,
        priority: crate::filters::strategy::QueryPriority
    ) -> Vec<(String, String)> {
        let mut urls = Vec::new();
        let clean_query = query.trim().to_uppercase();

        // Limit sources based on priority to save tokens
        let max_sources = match priority {
            crate::filters::strategy::QueryPriority::Critical => 5, // No limit for critical
            crate::filters::strategy::QueryPriority::High => 4,
            crate::filters::strategy::QueryPriority::Medium => 3,
            crate::filters::strategy::QueryPriority::Low => 2,
        };

        if self.is_likely_stock_symbol(&clean_query) {
            match market {
                "indian" => {
                    // Priority-based source selection for Indian stocks
                    match priority {
                        crate::filters::strategy::QueryPriority::Critical => {
                            // Use Yahoo Finance with multiple symbol variations for critical queries
                            let symbols_to_try = vec![
                                format!("{}.NS", clean_query),
                                format!("{}.BO", clean_query),
                                clean_query.clone(),
                            ];
                            
                            for (i, symbol) in symbols_to_try.iter().enumerate() {
                                if i >= max_sources { break; }
                                urls.push((
                                    format!("https://finance.yahoo.com/quote/{}/", symbol),
                                    format!("Yahoo Finance ({})", symbol)
                                ));
                            }
                        }
                        crate::filters::strategy::QueryPriority::High => {
                            // Use primary NSE symbol and one fallback
                            urls.push((
                                format!("https://finance.yahoo.com/quote/{}.NS/", clean_query),
                                format!("Yahoo Finance ({}.NS)", clean_query)
                            ));
                            urls.push((
                                format!("https://www.moneycontrol.com/india/stockpricequote/{}", urlencoding::encode(query)),
                                "MoneyControl".to_string()
                            ));
                        }
                        _ => {
                            // Basic NSE symbol only for lower priorities
                            urls.push((
                                format!("https://finance.yahoo.com/quote/{}.NS/", clean_query),
                                format!("Yahoo Finance ({}.NS)", clean_query)
                            ));
                        }
                    }
                }
                "us" => {
                    // Priority-based source selection for US stocks
                    match priority {
                        crate::filters::strategy::QueryPriority::Critical => {
                            urls.push((
                                format!("https://finance.yahoo.com/quote/{}/", clean_query),
                                format!("Yahoo Finance ({})", clean_query)
                            ));
                            urls.push((
                                format!("https://www.bloomberg.com/quote/{}", clean_query),
                                format!("Bloomberg ({})", clean_query)
                            ));
                        }
                        _ => {
                            urls.push((
                                format!("https://finance.yahoo.com/quote/{}/", clean_query),
                                format!("Yahoo Finance ({})", clean_query)
                            ));
                        }
                    }
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

        // Add search fallbacks only for higher priority queries to save tokens
        if !matches!(priority, crate::filters::strategy::QueryPriority::Low) && urls.len() < max_sources {
            urls.push((
                format!("https://finance.yahoo.com/lookup?s={}", urlencoding::encode(query)),
                "Yahoo Finance Search".to_string()
            ));
        }

        // Add market-specific search for critical queries
        if matches!(priority, crate::filters::strategy::QueryPriority::Critical) && urls.len() < max_sources {
            match market {
                "indian" => {
                    urls.push((
                        format!("https://www.screener.in/search/?q={}", urlencoding::encode(query)),
                        "Screener.in Search".to_string()
                    ));
                }
                "us" => {
                    urls.push((
                        format!("https://www.marketwatch.com/investing/stock/{}?mod=search_symbol", clean_query),
                        format!("MarketWatch ({})", clean_query)
                    ));
                }
                _ => {}
            }
        }

        // Truncate to max sources for the priority level
        urls.truncate(max_sources);
        urls
    }

    fn is_likely_stock_symbol(&self, query: &str) -> bool {
        let clean = query.trim();
        
        if clean.len() < 1 || clean.len() > 15 {
            return false;
        }

        let valid_chars = clean.chars().all(|c| c.is_alphanumeric() || c == '.');
        let has_letters = clean.chars().any(|c| c.is_alphabetic());
        
        valid_chars && has_letters
    }

    // ENHANCED: Smart stock symbol mapping for Indian market
    fn get_stock_symbol_variants(&self, query: &str) -> Vec<String> {
        let query_upper = query.to_uppercase();
        let mut variants = Vec::new();
        
        // Common stock symbol mappings for Indian market
        match query_upper.as_str() {
            "TATA MOTORS" | "TATAMOTORS" => {
                variants.extend(vec!["TATAMOTORS.NS".to_string(), "TATAMOTORS.BO".to_string()]);
            }
            "TCS" | "TATA CONSULTANCY" => {
                variants.extend(vec!["TCS.NS".to_string(), "TCS.BO".to_string()]);
            }
            "RELIANCE" | "RIL" => {
                variants.extend(vec!["RELIANCE.NS".to_string(), "RELIANCE.BO".to_string()]);
            }
            "INFOSYS" | "INFY" => {
                variants.extend(vec!["INFY.NS".to_string(), "INFY.BO".to_string()]);
            }
            "HDFC BANK" | "HDFCBANK" => {
                variants.extend(vec!["HDFCBANK.NS".to_string(), "HDFCBANK.BO".to_string()]);
            }
            _ => {
                // Default patterns
                if !query_upper.contains('.') {
                    variants.push(format!("{}.NS", query_upper));
                    variants.push(format!("{}.BO", query_upper));
                }
                variants.push(query_upper);
            }
        }
        
        variants
    }
}