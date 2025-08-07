// src/tools/stock.rs - ENHANCED VERSION WITH DEDUCT_DATA SUPPORT ONLY
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use crate::extras::logger::JSON_LOGGER;
use crate::metrics::brightdata_logger::BRIGHTDATA_METRICS;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use log::{info, warn, error};

pub struct StockDataTool;

#[async_trait]
impl Tool for StockDataTool {
    fn name(&self) -> &str {
        "get_stock_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive stock data including prices, performance, market cap, volumes with intelligent filtering and priority-based processing. Supports both direct BrightData API and proxy fallback."
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

        let query_priority = ResponseStrategy::classify_query_priority(query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(query);

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
                let method_used = result.get("method_used").and_then(|m| m.as_str()).unwrap_or("Unknown");
                
                // Create formatted response based on DEDUCT_DATA setting
                let formatted_response = self.create_formatted_stock_response(
                    query, market, content, source_used, method_used, 
                    data_type, timeframe, include_ratios, include_volume, &execution_id
                );
                
                let tool_result = ToolResult::success_with_raw(
                    vec![McpContent::text(formatted_response)], 
                    result
                );
                
                // Apply filtering only if DEDUCT_DATA=true
                if self.is_data_reduction_enabled() {
                    Ok(ResponseStrategy::apply_size_limits(tool_result))
                } else {
                    Ok(tool_result)
                }
            }
            Err(_e) => {
                // Return empty data for BrightData errors - Anthropic will retry
                warn!("BrightData error for query '{}', returning empty data for retry", query);
                let empty_response = json!({
                    "query": query,
                    "market": market,
                    "status": "no_data",
                    "reason": "brightdata_error",
                    "execution_id": execution_id
                });
                
                Ok(ToolResult::success_with_raw(
                    vec![McpContent::text("📈 **No Data Available**\n\nPlease try again with a more specific stock symbol.".to_string())],
                    empty_response
                ))
            }
        }
    }
}

impl StockDataTool {
    /// ENHANCED: Check if data reduction is enabled via DEDUCT_DATA environment variable only
    fn is_data_reduction_enabled(&self) -> bool {
        std::env::var("DEDUCT_DATA")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// ENHANCED: Create formatted response with DEDUCT_DATA control
    fn create_formatted_stock_response(
        &self,
        query: &str,
        market: &str, 
        content: &str,
        source: &str,
        method: &str,
        data_type: &str,
        timeframe: &str,
        include_ratios: bool,
        include_volume: bool,
        execution_id: &str
    ) -> String {
        // If DEDUCT_DATA=false, return full content with basic formatting
        if !self.is_data_reduction_enabled() {
            return format!(
                "📈 **{}** | {} Market\n\n## Full Content\n{}\n\n*Source: {} via {} • Type: {} • Period: {}*",
                query.to_uppercase(), 
                market.to_uppercase(), 
                content,
                source, 
                method, 
                data_type, 
                timeframe
            );
        }

        // TODO: Add filtered data extraction logic when DEDUCT_DATA=true
        // For now, return full content formatted
        format!(
            "📈 **{}** | {} Market\n\n## Content (TODO: Add Filtering)\n{}\n\n*Source: {} via {} • Type: {} • Period: {}*",
            query.to_uppercase(), 
            market.to_uppercase(), 
            content,
            source, 
            method, 
            data_type, 
            timeframe
        )
    }
    
    /// TODO: Extract essential stock data using existing filter methods
    fn extract_essential_stock_data(&self, content: &str, query: &str) -> String {
        // TODO: Add essential stock data extraction logic
        // For now, return original content
        content.to_string()
    }
    
    /// TODO: Extract financial lines when filtering is disabled
    fn extract_financial_lines(&self, content: &str) -> String {
        // TODO: Add financial lines extraction logic
        // For now, return original content
        content.to_string()
    }
    
    /// TODO: Format financial metrics into clean markdown
    fn format_financial_metrics(&self, data: &str) -> String {
        // TODO: Add financial metrics formatting logic
        // For now, return data as-is
        data.to_string()
    }

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
        let mut attempts = Vec::new();

        for (sequence, (url, source_name)) in urls_to_try.iter().enumerate() {
            // Try proxy method only for now
            let methods_to_try = vec![("proxy", "Proxy Fallback")];

            for (method_sequence, (method_type, method_name)) in methods_to_try.iter().enumerate() {
                let attempt_result = match *method_type {
                    "direct" => {
                        info!("🌐 Trying Direct BrightData API for {} (sequence: {}, attempt: {})", 
                              source_name, sequence, method_sequence + 1);
                        self.try_fetch_url_direct_api(
                            url, query, market, source_name, query_priority, token_budget, 
                            execution_id, sequence as u64, method_sequence as u64
                        ).await
                    }
                    "proxy" => {
                        info!("🔄 Trying Proxy method for {} (sequence: {}, attempt: {})", 
                              source_name, sequence, method_sequence + 1);
                        self.try_fetch_url_via_proxy(
                            url, query, market, source_name, query_priority, token_budget, 
                            execution_id, sequence as u64, method_sequence as u64
                        ).await
                    }
                    _ => continue,
                };

                match attempt_result {
                    Ok(mut result) => {
                        let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        
                        attempts.push(json!({
                            "source": source_name,
                            "url": url,
                            "method": method_name,
                            "status": "success",
                            "content_length": content.len(),
                            "sequence": sequence + 1,
                            "method_sequence": method_sequence + 1
                        }));
                        
                        // TODO: Add content quality check when DEDUCT_DATA=true
                        let should_try_next = if self.is_data_reduction_enabled() {
                            // TODO: Add quality-based next source logic
                            false
                        } else {
                            false
                        };
                        
                        if should_try_next && (method_sequence == 0 || sequence < urls_to_try.len() - 1) {
                            if method_sequence == 0 {
                                warn!("Content insufficient from {} via {}, trying proxy method", source_name, method_name);
                                continue; // Try proxy method for same URL
                            } else {
                                warn!("Content insufficient from {} via {}, trying next source", source_name, method_name);
                                break; // Try next URL
                            }
                        }
                        
                        // SUCCESS - but validate data quality first only if DEDUCT_DATA=true
                        if self.is_data_reduction_enabled() {
                            // TODO: Add data quality validation when DEDUCT_DATA=true
                            // For now, accept all content
                        }
                        
                        // SUCCESS
                        result["source_used"] = json!(source_name);
                        result["url_used"] = json!(url);
                        result["method_used"] = json!(method_name);
                        result["execution_id"] = json!(execution_id);
                        result["priority"] = json!(format!("{:?}", query_priority));
                        result["token_budget"] = json!(token_budget);
                        result["attempts"] = json!(attempts);
                        result["successful_attempt"] = json!(sequence + 1);
                        result["successful_method"] = json!(method_sequence + 1);
                        
                        info!("✅ Successfully fetched stock data from {} via {} (sequence: {}, method: {})", 
                              source_name, method_name, sequence + 1, method_sequence + 1);
                        
                        return Ok(result);
                    }
                    Err(e) => {
                        attempts.push(json!({
                            "source": source_name,
                            "url": url,
                            "method": method_name,
                            "status": "failed",
                            "error": e.to_string(),
                            "sequence": sequence + 1,
                            "method_sequence": method_sequence + 1
                        }));
                        
                        last_error = Some(e);
                        warn!("❌ Failed to fetch from {} via {} (sequence: {}, method: {}): {:?}", 
                              source_name, method_name, sequence + 1, method_sequence + 1, last_error);
                    }
                }
            }
        }

        // All methods and sources failed - return empty data instead of error
        warn!("❌ All sources and methods failed for query '{}'. Returning empty data for Anthropic retry", query);
        
        let empty_result = json!({
            "query": query,
            "market": market,
            "status": "no_data_found",
            "attempts": attempts,
            "execution_id": execution_id,
            "total_attempts": urls_to_try.len() * 2, // Direct + Proxy for each URL
            "reason": "all_sources_failed"
        });
        
        Ok(empty_result)
    }

    // Direct BrightData API method (existing implementation)
    async fn try_fetch_url_direct_api(
        &self, 
        url: &str, 
        query: &str, 
        market: &str, 
        source_name: &str, 
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
        sequence: u64,
        method_sequence: u64
    ) -> Result<Value, BrightDataError> {
        let max_retries = env::var("MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        
        let mut last_error = None;
        
        for retry_attempt in 0..max_retries {
            let start_time = Instant::now();
            let attempt_id = format!("{}_direct_s{}_m{}_r{}", execution_id, sequence, method_sequence, retry_attempt);
            
            info!("🌐 Direct API: Fetching from {} (execution: {}, retry: {}/{})", 
                  source_name, attempt_id, retry_attempt + 1, max_retries);
            
            let api_token = env::var("BRIGHTDATA_API_TOKEN")
                .or_else(|_| env::var("API_TOKEN"))
                .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN environment variable".into()))?;

            let base_url = env::var("BRIGHTDATA_BASE_URL")
                .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

            let zone = env::var("WEB_UNLOCKER_ZONE")
                .unwrap_or_else(|_| "mcp_unlocker".to_string());

            let payload = json!({
                "url": url,
                "zone": zone,
                "format": "raw",
                "data_format": "markdown"
            });

            if retry_attempt == 0 {
                info!("📤 Direct API Request:");
                info!("   Endpoint: {}/request", base_url);
                info!("   Zone: {}", zone);
                info!("   Target: {}", url);
            }

            let client = Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .map_err(|e| BrightDataError::ToolError(format!("Failed to create HTTP client: {}", e)))?;

            let response = client
                .post(&format!("{}/request", base_url))
                .header("Authorization", format!("Bearer {}", api_token))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|e| BrightDataError::ToolError(format!("Direct API request failed to {}: {}", source_name, e)))?;

            let duration = start_time.elapsed();
            let status = response.status().as_u16();
            let response_headers: HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            info!("📥 Direct API Response (retry {}):", retry_attempt + 1);
            info!("   Status: {}", status);
            info!("   Duration: {}ms", duration.as_millis());

            let response_text = response.text().await
                .map_err(|e| BrightDataError::ToolError(format!("Failed to read response body from {}: {}", source_name, e)))?;

            // Handle server errors with retry
            if matches!(status, 502 | 503 | 504) && retry_attempt < max_retries - 1 {
                let wait_time = Duration::from_millis(1000 + (retry_attempt as u64 * 1000));
                warn!("⏳ Direct API: Server error {}, waiting {}ms before retry...", status, wait_time.as_millis());
                tokio::time::sleep(wait_time).await;
                last_error = Some(BrightDataError::ToolError(format!("Direct API server error: {}", status)));
                continue;
            }
            
            if !(200..300).contains(&status) {
                let error_msg = format!("Direct API: {} returned HTTP {}: {}", source_name, status, 
                                      &response_text[..response_text.len().min(500)]);
                last_error = Some(BrightDataError::ToolError(error_msg));
                if retry_attempt == max_retries - 1 {
                    return Err(last_error.unwrap());
                }
                continue;
            }

            // SUCCESS - Process response (apply filtering only if DEDUCT_DATA=true)
            let raw_content = response_text;
            let filtered_content = if self.is_data_reduction_enabled() {
                // TODO: Add content filtering logic when DEDUCT_DATA=true
                raw_content.clone()
            } else {
                raw_content.clone()
            };

            info!("📊 Direct API: Content processed: {} bytes -> {} bytes", 
                  raw_content.len(), filtered_content.len());

            // Log metrics
            if let Err(e) = BRIGHTDATA_METRICS.log_call(
                &attempt_id,
                url,
                &zone,
                "raw",
                None,
                payload.clone(),
                status,
                response_headers.clone(),
                &raw_content,
                Some(&filtered_content),
                duration.as_millis() as u64,
                None,
                None,
            ).await {
                warn!("Failed to log direct API metrics: {}", e);
            }

            return Ok(json!({
                "content": filtered_content,
                "raw_content": raw_content,
                "query": query,
                "market": market,
                "source": source_name,
                "method": "Direct BrightData API",
                "priority": format!("{:?}", priority),
                "token_budget": token_budget,
                "execution_id": execution_id,
                "sequence": sequence,
                "method_sequence": method_sequence,
                "success": true,
                "url": url,
                "zone": zone,
                "format": "raw",
                "status_code": status,
                "response_size_bytes": raw_content.len(),
                "filtered_size_bytes": filtered_content.len(),
                "duration_ms": duration.as_millis(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "retry_attempts": retry_attempt + 1,
                "max_retries": max_retries,
                "payload_used": payload
            }));
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("Direct API: All retry attempts failed".into())))
    }

    // Proxy-based method
    async fn try_fetch_url_via_proxy(
        &self, 
        url: &str, 
        query: &str, 
        market: &str, 
        source_name: &str, 
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
        sequence: u64,
        method_sequence: u64
    ) -> Result<Value, BrightDataError> {
        let max_retries = env::var("MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        
        let mut last_error = None;
        
        // Get proxy configuration from environment
        let proxy_host = env::var("BRIGHTDATA_PROXY_HOST")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_PROXY_HOST environment variable".into()))?;
        let proxy_port = env::var("BRIGHTDATA_PROXY_PORT")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_PROXY_PORT environment variable".into()))?;
        let proxy_username = env::var("BRIGHTDATA_PROXY_USERNAME")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_PROXY_USERNAME environment variable".into()))?;
        let proxy_password = env::var("BRIGHTDATA_PROXY_PASSWORD")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_PROXY_PASSWORD environment variable".into()))?;

        let proxy_url = format!("http://{}:{}@{}:{}", proxy_username, proxy_password, proxy_host, proxy_port);
        
        for retry_attempt in 0..max_retries {
            let start_time = Instant::now();
            let attempt_id = format!("{}_proxy_s{}_m{}_r{}", execution_id, sequence, method_sequence, retry_attempt);
            
            info!("🔄 Proxy: Fetching from {} via proxy (execution: {}, retry: {}/{})", 
                  source_name, attempt_id, retry_attempt + 1, max_retries);
            
            if retry_attempt == 0 {
                info!("📤 Proxy Request:");
                info!("   Proxy: {}:{}@{}:{}", proxy_username, "***", proxy_host, proxy_port);
                info!("   Target: {}", url);
            }

            // Create client with proxy configuration
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| BrightDataError::ToolError(format!("Failed to create proxy: {}", e)))?;

            let client = Client::builder()
                .proxy(proxy)
                .timeout(Duration::from_secs(90))
                .danger_accept_invalid_certs(true) // Often needed for proxy connections
                .build()
                .map_err(|e| BrightDataError::ToolError(format!("Failed to create proxy client: {}", e)))?;

            let response = client
                .get(url)
                .header("x-unblock-data-format", "markdown")
                .send()
                .await
                .map_err(|e| BrightDataError::ToolError(format!("Proxy request failed to {}: {}", source_name, e)))?;

            let duration = start_time.elapsed();
            let status = response.status().as_u16();
            let response_headers: HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            info!("📥 Proxy Response (retry {}):", retry_attempt + 1);
            info!("   Status: {}", status);
            info!("   Duration: {}ms", duration.as_millis());

            let response_text = response.text().await
                .map_err(|e| BrightDataError::ToolError(format!("Failed to read proxy response body from {}: {}", source_name, e)))?;

            // Handle server errors with retry
            if matches!(status, 502 | 503 | 504) && retry_attempt < max_retries - 1 {
                let wait_time = Duration::from_millis(1000 + (retry_attempt as u64 * 1000));
                warn!("⏳ Proxy: Server error {}, waiting {}ms before retry...", status, wait_time.as_millis());
                tokio::time::sleep(wait_time).await;
                last_error = Some(BrightDataError::ToolError(format!("Proxy server error: {}", status)));
                continue;
            }
            
            if !(200..300).contains(&status) {
                println!("-----------------------------------------------------------------");
                println!("MARKDOWN SUCCESS: {:?}", status.clone());
                println!("-----------------------------------------------------------------");
                let error_msg = format!("Proxy: {} returned HTTP {}: {}", source_name, status, 
                                      &response_text[..response_text.len().min(200)]);
                
                warn!("Proxy HTTP error: {}", error_msg);
                last_error = Some(BrightDataError::ToolError(error_msg));
                
                // Log error metrics for proxy
                let proxy_payload = json!({
                    "url": url,
                    "method": "proxy",
                    "proxy_host": proxy_host,
                    "proxy_port": proxy_port,
                    "error": format!("HTTP {}", status)
                });

                if let Err(e) = BRIGHTDATA_METRICS.log_call(
                    &attempt_id,
                    url,
                    "proxy",
                    "raw",
                    None,
                    proxy_payload,
                    status,
                    response_headers.clone(),
                    &response_text,
                    Some(&format!("Proxy HTTP {} Error", status)),
                    duration.as_millis() as u64,
                    None,
                    None,
                ).await {
                    warn!("Failed to log proxy error metrics: {}", e);
                }
                
                if retry_attempt == max_retries - 1 {
                    return Err(last_error.unwrap());
                }
                continue;
            }

            // SUCCESS - Process response (apply filtering only if DEDUCT_DATA=true)
            let raw_content = response_text;
            let filtered_content = if self.is_data_reduction_enabled() {
                // TODO: Add content filtering logic when DEDUCT_DATA=true
                raw_content.clone()
            } else {
                raw_content.clone()
            };

            info!("📊 Proxy: Content processed: {} bytes -> {} bytes", 
                  raw_content.len(), filtered_content.len());

            // Log metrics (using a simplified payload for proxy requests)
            let proxy_payload = json!({
                "url": url,
                "method": "proxy",
                "proxy_host": proxy_host,
                "proxy_port": proxy_port
            });

            if let Err(e) = BRIGHTDATA_METRICS.log_call(
                &attempt_id,
                url,
                "proxy",
                "raw",
                None,
                proxy_payload.clone(),
                status,
                response_headers.clone(),
                &raw_content,
                Some(&filtered_content),
                duration.as_millis() as u64,
                None,
                None,
            ).await {
                warn!("Failed to log proxy metrics: {}", e);
            }

            return Ok(json!({
                "content": filtered_content,
                "raw_content": raw_content,
                "query": query,
                "market": market,
                "source": source_name,
                "method": "BrightData Proxy",
                "priority": format!("{:?}", priority),
                "token_budget": token_budget,
                "execution_id": execution_id,
                "sequence": sequence,
                "method_sequence": method_sequence,
                "success": true,
                "url": url,
                "proxy_host": proxy_host,
                "proxy_port": proxy_port,
                "status_code": status,
                "response_size_bytes": raw_content.len(),
                "filtered_size_bytes": filtered_content.len(),
                "duration_ms": duration.as_millis(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "retry_attempts": retry_attempt + 1,
                "max_retries": max_retries,
                "payload_used": proxy_payload
            }));
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("Proxy: All retry attempts failed".into())))
    }

    fn build_prioritized_urls_with_priority(
        &self, 
        query: &str, 
        market: &str, 
        data_type: &str,
        priority: crate::filters::strategy::QueryPriority
    ) -> Vec<(String, String)> {
        let mut urls = Vec::new();
        let clean_query = query.trim().to_uppercase();

        // TODO: Add priority-based URL limiting when DEDUCT_DATA=true
        let max_sources = if self.is_data_reduction_enabled() {
            // TODO: Add priority-based source limiting logic
            5
        } else {
            5 // No limit when DEDUCT_DATA=false
        };

        if self.is_likely_stock_symbol(&clean_query) {
            match market {
                "indian" => {
                    // TODO: Add priority-based URL selection logic
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

        // Add search fallbacks (no restrictions when DEDUCT_DATA=false)
        if urls.len() < max_sources {
            urls.push((
                format!("https://finance.yahoo.com/quote/{}", urlencoding::encode(query)),
                "Yahoo Finance Search".to_string()
            ));
        }

        info!("🎯 Generated {} URLs for query '{}' (priority: {:?})", urls.len(), query, priority);
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

    /// Test both direct API and proxy connectivity
    pub async fn test_connectivity(&self) -> Result<String, BrightDataError> {
        let test_url = "https://finance.yahoo.com/quote/AAPL/";
        let mut results = Vec::new();
        
        // Test Direct API
        info!("🧪 Testing Direct BrightData API...");
        match self.try_fetch_url_direct_api(
            test_url, "AAPL", "us", "Yahoo Finance Test", 
            crate::filters::strategy::QueryPriority::High, 1000, 
            "connectivity_test", 0, 0
        ).await {
            Ok(_) => {
                results.push("✅ Direct API: SUCCESS".to_string());
            }
            Err(e) => {
                results.push(format!("❌ Direct API: FAILED - {}", e));
            }
        }
        
        // Test Proxy
        info!("🧪 Testing Proxy method...");
        match self.try_fetch_url_via_proxy(
            test_url, "AAPL", "us", "Yahoo Finance Test", 
            crate::filters::strategy::QueryPriority::High, 1000, 
            "connectivity_test", 0, 1
        ).await {
            Ok(_) => {
                results.push("✅ Proxy: SUCCESS".to_string());
            }
            Err(e) => {
                results.push(format!("❌ Proxy: FAILED - {}", e));
            }
        }
        
        Ok(format!("🔍 Connectivity Test Results:\n{}", results.join("\n")))
    }
}