// src/tools/stock.rs - Updated with optional filtering via TRUNCATE_FILTER env var
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use crate::metrics::EnhancedLogger;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::{Duration, Instant};
use std::collections::HashMap;

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
                    "description": "Stock symbol (e.g. TATAMOTORS, TCS, AAPL), company name, or market overview request"
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

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;
        
        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, market, "validation", json!({}), response_type));
            }
        }

        let execution_id = format!("stock_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        match self.fetch_stock_data_with_fallbacks(query, market, &execution_id).await {
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
                        "📈 **Stock Data for: {}**\n\nMarket: {}\nSource: {}\n\n{}",
                        query, market, source_used, content
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
    async fn fetch_stock_data_with_fallbacks(&self, query: &str, market: &str, execution_id: &str) -> Result<Value, BrightDataError> {
        let urls_to_try = self.build_prioritized_urls(query, market);
        let mut last_error = None;

        for (sequence, (url, source_name)) in urls_to_try.iter().enumerate() {
            match self.try_fetch_url_with_enhanced_logging(url, query, market, source_name, execution_id, sequence as u64).await {
                Ok(mut result) => {
                    let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    
                    // Use strategy to determine if we should try next source only if filtering enabled
                    if std::env::var("TRUNCATE_FILTER")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false) {
                        
                        if ResponseStrategy::should_try_next_source(content) {
                            log::warn!("Content quality too low from {}, trying next source", source_name);
                            last_error = Some(BrightDataError::ToolError(format!(
                                "{} returned low-quality content", source_name
                            )));
                            continue;
                        }
                    }
                    
                    result["source_used"] = json!(source_name);
                    result["url_used"] = json!(url);
                    result["execution_id"] = json!(execution_id);
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    log::warn!("Failed to fetch from {}: {:?}", source_name, last_error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("All stock data sources failed".into())))
    }

    async fn try_fetch_url_with_enhanced_logging(
        &self, 
        url: &str, 
        query: &str, 
        market: &str, 
        source_name: &str, 
        execution_id: &str,
        sequence: u64
    ) -> Result<Value, BrightDataError> {
        let start_time = Instant::now();
        
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
            
            // Log failed call with enhanced logger
            if let Err(e) = EnhancedLogger::log_brightdata_request_enhanced(
                &format!("{}_{}", execution_id, sequence),
                &zone,
                url,
                payload,
                status,
                response_headers,
                "markdown",
                &error_text,
                None,
                duration,
                Some(execution_id),
            ).await {
                log::warn!("Failed to log enhanced metrics: {}", e);
            }
            
            return Err(BrightDataError::ToolError(format!(
                "{} returned {}: {}",
                source_name, status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply filters conditionally based on environment variable
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
                ResponseFilter::filter_financial_content(&raw_content)
            }
        } else {
            // No filtering if disabled
            raw_content.clone()
        };

        // Log successful call with enhanced logger
        if let Err(e) = EnhancedLogger::log_brightdata_request_enhanced(
            &format!("{}_{}", execution_id, sequence),
            &zone,
            url,
            payload,
            status,
            response_headers,
            "markdown",
            &raw_content,
            Some(&filtered_content),
            duration,
            Some(execution_id),
        ).await {
            log::warn!("Failed to log enhanced metrics: {}", e);
        }

        Ok(json!({
            "content": filtered_content,
            "query": query,
            "market": market,
            "success": true
        }))
    }

    fn build_prioritized_urls(&self, query: &str, market: &str) -> Vec<(String, String)> {
        let mut urls = Vec::new();
        let clean_query = query.trim().to_uppercase();

        if self.is_likely_stock_symbol(&clean_query) {
            match market {
                "indian" => {
                    let symbols_to_try = vec![
                        format!("{}.NS", clean_query),
                        format!("{}.BO", clean_query),
                        clean_query.clone(),
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

        // Add search fallbacks
        urls.push((
            format!("https://finance.yahoo.com/lookup?s={}", urlencoding::encode(query)),
            "Yahoo Finance Search".to_string()
        ));

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
}