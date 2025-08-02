// src/tools/market.rs - PATCHED: Enhanced with priority-aware filtering and token budget management
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use std::collections::HashMap;
use log::info;

pub struct MarketOverviewTool;

#[async_trait]
impl Tool for MarketOverviewTool {
    fn name(&self) -> &str {
        "get_market_overview"
    }

    fn description(&self) -> &str {
        "Get comprehensive market overview with enhanced search parameters including pagination, localization, and intelligent filtering"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "market_type": {
                    "type": "string",
                    "enum": ["stocks", "crypto", "bonds", "commodities", "forex", "overall"],
                    "default": "overall",
                    "description": "Type of market overview - overall for general market, or specific asset class"
                },
                "region": {
                    "type": "string",
                    "enum": ["indian", "us", "global", "asia", "europe"],
                    "default": "indian",
                    "description": "Market region for localized data"
                },
                "timeframe": {
                    "type": "string",
                    "enum": ["live", "day", "week", "month", "quarter", "year"],
                    "default": "day",
                    "description": "Time period for market overview analysis"
                },
                "include_indices": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include major market indices (Nifty, Sensex, S&P500, etc.)"
                },
                "include_movers": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include top gainers and losers"
                },
                "include_news": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include latest market news and events"
                },
                "data_source": {
                    "type": "string",
                    "enum": ["search", "direct", "auto"],
                    "default": "auto",
                    "description": "Data source strategy - search (SERP), direct (financial sites), auto (smart selection)"
                },
                "page": {
                    "type": "integer",
                    "description": "Page number for pagination (1-based)",
                    "minimum": 1,
                    "maximum": 5,
                    "default": 1
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results per page",
                    "minimum": 5,
                    "maximum": 50,
                    "default": 20
                },
                "use_serp_api": {
                    "type": "boolean",
                    "description": "Use enhanced SERP API with advanced parameters",
                    "default": true
                }
            },
            "required": []
        })
    }

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let market_type = parameters
            .get("market_type")
            .and_then(|v| v.as_str())
            .unwrap_or("overall");
        
        let region = parameters
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let timeframe = parameters
            .get("timeframe")
            .and_then(|v| v.as_str())
            .unwrap_or("day");

        let include_indices = parameters
            .get("include_indices")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_movers = parameters
            .get("include_movers")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_news = parameters
            .get("include_news")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let data_source = parameters
            .get("data_source")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let page = parameters
            .get("page")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as u32;

        let num_results = parameters
            .get("num_results")
            .and_then(|v| v.as_i64())
            .unwrap_or(20) as u32;

        let use_serp_api = parameters
            .get("use_serp_api")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // ENHANCED: Priority classification and token allocation
        let query_string = format!("{} {} market overview", region, market_type);
        let query_priority = ResponseStrategy::classify_query_priority(&query_string);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(&query_string);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", &query_string);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", &query_string, region, "validation", json!({}), response_type));
            }

            // Budget check for market overview queries  
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", &query_string, region, "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("market_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        match self.fetch_market_overview_with_fallbacks_and_priority(
            market_type, region, timeframe, include_indices, include_movers, 
            include_news, data_source, page, num_results, use_serp_api, 
            query_priority, recommended_tokens, &execution_id
        ).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let source_used = result.get("source_used").and_then(|s| s.as_str()).unwrap_or("Unknown");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "market", &query_string, region, source_used, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let content_text = if use_serp_api {
                        result.get("formatted_content").and_then(|c| c.as_str()).unwrap_or(content)
                    } else {
                        content
                    };

                    let mcp_content = vec![McpContent::text(format!(
                        "📊 **Market Overview - {} Market ({})**\n\nTimeframe: {} | Priority: {:?} | Tokens: {}\nSource: {} | Indices: {} | Movers: {} | News: {}\nExecution ID: {}\n\n{}",
                        market_type.to_uppercase(), region.to_uppercase(), timeframe, query_priority, 
                        recommended_tokens, source_used, include_indices, include_movers, include_news, execution_id, content_text
                    ))];
                    ToolResult::success_with_raw(mcp_content, result)
                };
                
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
                    Ok(ResponseStrategy::create_error_response(&query_string, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl MarketOverviewTool {
    // ENHANCED: Priority-aware market data fetching with token management
    async fn fetch_market_overview_with_fallbacks_and_priority(
        &self,
        market_type: &str,
        region: &str,
        timeframe: &str,
        include_indices: bool,
        include_movers: bool,
        include_news: bool,
        data_source: &str,
        page: u32,
        num_results: u32,
        use_serp_api: bool,
        query_priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let sources_to_try = self.build_prioritized_sources_with_priority(
            market_type, region, data_source, query_priority
        );
        let mut last_error = None;

        for (sequence, (source_type, url_or_query, source_name)) in sources_to_try.iter().enumerate() {
            match source_type.as_str() {
                "direct" => {
                    match self.fetch_direct_market_data_with_priority(
                        url_or_query, market_type, region, source_name, query_priority, 
                        token_budget, execution_id, sequence as u64
                    ).await {
                        Ok(mut result) => {
                            let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            
                            // Use strategy to determine if we should try next source only if filtering enabled
                            if std::env::var("TRUNCATE_FILTER")
                                .map(|v| v.to_lowercase() == "true")
                                .unwrap_or(false) {
                                
                                if ResponseStrategy::should_try_next_source(content) {
                                    last_error = Some(BrightDataError::ToolError(format!(
                                        "{} returned low-quality content", source_name
                                    )));
                                    continue;
                                }
                            }
                            
                            result["source_used"] = json!(source_name);
                            result["data_source_type"] = json!("direct");
                            result["priority"] = json!(format!("{:?}", query_priority));
                            return Ok(result);
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                "search" => {
                    if use_serp_api {
                        match self.fetch_market_overview_enhanced_with_priority(
                            url_or_query, region, timeframe, include_indices, include_movers,
                            include_news, page, num_results, source_name, query_priority, 
                            token_budget, execution_id, sequence as u64
                        ).await {
                            Ok(mut result) => {
                                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                
                                if std::env::var("TRUNCATE_FILTER")
                                    .map(|v| v.to_lowercase() == "true")
                                    .unwrap_or(false) {
                                    
                                    if ResponseStrategy::should_try_next_source(content) {
                                        last_error = Some(BrightDataError::ToolError(format!(
                                            "{} returned low-quality content", source_name
                                        )));
                                        continue;
                                    }
                                }
                                
                                result["source_used"] = json!(source_name);
                                result["data_source_type"] = json!("enhanced_search");
                                result["priority"] = json!(format!("{:?}", query_priority));
                                return Ok(result);
                            }
                            Err(e) => last_error = Some(e),
                        }
                    } else {
                        match self.fetch_market_overview_legacy_with_priority(
                            market_type, region, query_priority, token_budget, execution_id, sequence as u64
                        ).await {
                            Ok(mut result) => {
                                result["source_used"] = json!("Legacy Search");
                                result["data_source_type"] = json!("legacy_search");
                                result["priority"] = json!(format!("{:?}", query_priority));
                                return Ok(result);
                            }
                            Err(e) => last_error = Some(e),
                        }
                    }
                }
                _ => continue,
            }
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("All market overview sources failed".into())))
    }

    // ENHANCED: Priority-aware source building
    fn build_prioritized_sources_with_priority(
        &self, 
        market_type: &str, 
        region: &str, 
        data_source: &str, 
        priority: crate::filters::strategy::QueryPriority
    ) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();

        match data_source {
            "direct" => {
                sources.extend(self.get_direct_sources_with_priority(market_type, region, priority));
            }
            "search" => {
                sources.extend(self.get_search_sources_with_priority(market_type, region, priority));
            }
            "auto" | _ => {
                // Priority-aware smart selection
                match priority {
                    crate::filters::strategy::QueryPriority::Critical => {
                        // For critical queries, prioritize direct sources for speed
                        sources.extend(self.get_direct_sources_with_priority(market_type, region, priority));
                        sources.extend(self.get_search_sources_with_priority(market_type, region, priority));
                    }
                    _ => {
                        // For most market overviews, search provides better comprehensive data
                        sources.extend(self.get_search_sources_with_priority(market_type, region, priority));
                        sources.extend(self.get_direct_sources_with_priority(market_type, region, priority));
                    }
                }
            }
        }

        // Limit sources based on priority to save tokens
        let max_sources = match priority {
            crate::filters::strategy::QueryPriority::Critical => sources.len(), // No limit for critical
            crate::filters::strategy::QueryPriority::High => std::cmp::min(sources.len(), 3),
            crate::filters::strategy::QueryPriority::Medium => std::cmp::min(sources.len(), 2),
            crate::filters::strategy::QueryPriority::Low => std::cmp::min(sources.len(), 1),
        };

        sources.truncate(max_sources);
        sources
    }

    fn get_direct_sources_with_priority(
        &self, 
        market_type: &str, 
        region: &str, 
        priority: crate::filters::strategy::QueryPriority
    ) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();

        match (region, market_type) {
            ("indian", "stocks") => {
                sources.push(("direct".to_string(), "https://www.nseindia.com/".to_string(), "NSE India".to_string()));
                if matches!(priority, crate::filters::strategy::QueryPriority::Critical | crate::filters::strategy::QueryPriority::High) {
                    sources.push(("direct".to_string(), "https://www.bseindia.com/".to_string(), "BSE India".to_string()));
                    sources.push(("direct".to_string(), "https://www.moneycontrol.com/".to_string(), "MoneyControl".to_string()));
                }
            }
            ("indian", "overall") => {
                sources.push(("direct".to_string(), "https://www.nseindia.com/market-data/live-equity-market".to_string(), "NSE Live Market".to_string()));
                if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
                    sources.push(("direct".to_string(), "https://www.rbi.org.in/".to_string(), "RBI".to_string()));
                    sources.push(("direct".to_string(), "https://economictimes.indiatimes.com/markets".to_string(), "Economic Times Markets".to_string()));
                }
            }
            ("us", "stocks") => {
                sources.push(("direct".to_string(), "https://finance.yahoo.com/".to_string(), "Yahoo Finance".to_string()));
                if matches!(priority, crate::filters::strategy::QueryPriority::Critical | crate::filters::strategy::QueryPriority::High) {
                    sources.push(("direct".to_string(), "https://www.bloomberg.com/markets".to_string(), "Bloomberg Markets".to_string()));
                    sources.push(("direct".to_string(), "https://www.marketwatch.com/".to_string(), "MarketWatch".to_string()));
                }
            }
            ("us", "overall") => {
                sources.push(("direct".to_string(), "https://finance.yahoo.com/".to_string(), "Yahoo Finance".to_string()));
                if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
                    sources.push(("direct".to_string(), "https://www.federalreserve.gov/".to_string(), "Federal Reserve".to_string()));
                    sources.push(("direct".to_string(), "https://www.cnbc.com/markets/".to_string(), "CNBC Markets".to_string()));
                }
            }
            ("global", _) => {
                sources.push(("direct".to_string(), "https://www.investing.com/".to_string(), "Investing.com".to_string()));
                if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
                    sources.push(("direct".to_string(), "https://www.bloomberg.com/markets".to_string(), "Bloomberg Global".to_string()));
                    sources.push(("direct".to_string(), "https://finance.yahoo.com/world-indices/".to_string(), "Yahoo Global".to_string()));
                }
            }
            _ => {
                // Default sources
                sources.push(("direct".to_string(), "https://www.investing.com/".to_string(), "Investing.com".to_string()));
            }
        }

        sources
    }

    fn get_search_sources_with_priority(
        &self, 
        market_type: &str, 
        region: &str, 
        priority: crate::filters::strategy::QueryPriority
    ) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();

        // Priority-based search query construction
        match priority {
            crate::filters::strategy::QueryPriority::Critical => {
                sources.push(("search".to_string(), 
                    self.build_market_query_with_priority(market_type, region, "live current real-time"),
                    "Critical Market Search".to_string()));
            }
            crate::filters::strategy::QueryPriority::High => {
                sources.push(("search".to_string(), 
                    self.build_market_query_with_priority(market_type, region, "today latest performance"),
                    "High Priority Market Search".to_string()));
                sources.push(("search".to_string(), 
                    self.build_market_query_with_priority(market_type, region, "analysis trends overview"),
                    "Market Analysis Search".to_string()));
            }
            crate::filters::strategy::QueryPriority::Medium => {
                sources.push(("search".to_string(), 
                    self.build_market_query_with_priority(market_type, region, "overview performance"),
                    "Market Performance Search".to_string()));
            }
            crate::filters::strategy::QueryPriority::Low => {
                sources.push(("search".to_string(), 
                    self.build_market_query_with_priority(market_type, region, "overview"),
                    "Basic Market Search".to_string()));
            }
        }

        sources
    }

    // ENHANCED: Priority-aware direct market data fetching
    async fn fetch_direct_market_data_with_priority(
        &self,
        url: &str,
        market_type: &str,
        region: &str,
        source_name: &str,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
        sequence: u64,
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        info!("📊 Priority {} direct market data fetch from {} using zone: {} (execution: {})", 
              format!("{:?}", priority), source_name, zone, execution_id);

        let mut payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown",
            "render": true
        });

        // Add priority processing hints
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            payload["processing_priority"] = json!(format!("{:?}", priority));
            payload["token_budget"] = json!(token_budget);
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Direct market data request failed: {}", e)))?;

        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Log BrightData request
        if let Err(e) = JSON_LOGGER.log_brightdata_request(
            &format!("{}_{}", execution_id, sequence),
            &zone,
            url,
            payload.clone(),
            status,
            response_headers,
            "markdown"
        ).await {
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData direct market data error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply priority-aware filters conditionally based on environment variable
        let filtered_content = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError(format!("{} returned error page", source_name)));
            } else {
                // Use enhanced extraction with token budget awareness
                let max_tokens = token_budget / 2; // Reserve tokens for formatting
                ResponseFilter::extract_high_value_financial_data(&raw_content, max_tokens)
            }
        } else {
            raw_content.clone()
        };

        Ok(json!({
            "content": filtered_content,
            "market_type": market_type,
            "region": region,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }

    // ENHANCED: Priority-aware enhanced SERP market data fetching
    async fn fetch_market_overview_enhanced_with_priority(
        &self,
        search_query: &str,
        region: &str,
        timeframe: &str,
        include_indices: bool,
        include_movers: bool,
        include_news: bool,
        page: u32,
        num_results: u32,
        source_name: &str,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
        sequence: u64,
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        // Build enhanced search query with priority awareness
        let mut enhanced_query = search_query.to_string();
        
        // Add terms based on priority
        match priority {
            crate::filters::strategy::QueryPriority::Critical => {
                enhanced_query.push_str(" live current real-time market");
            }
            crate::filters::strategy::QueryPriority::High => {
                if include_indices {
                    enhanced_query.push_str(" indices nifty sensex dow s&p");
                }
                if include_movers {
                    enhanced_query.push_str(" gainers losers top performers");
                }
                if include_news {
                    enhanced_query.push_str(" latest news market updates");
                }
            }
            _ => {
                // Basic terms for lower priority
                enhanced_query.push_str(" overview summary");
            }
        }

        // Add timeframe only for higher priority queries
        if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            match timeframe {
                "live" => enhanced_query.push_str(" live real-time"),
                "day" => enhanced_query.push_str(" today daily"),
                "week" => enhanced_query.push_str(" weekly performance"),
                "month" => enhanced_query.push_str(" monthly trends"),
                "quarter" => enhanced_query.push_str(" quarterly performance"),
                "year" => enhanced_query.push_str(" annual overview"),
                _ => {}
            }
        }

        // Build SERP API query parameters with priority-based limits
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), enhanced_query.clone());
        
        // Adjust results based on priority
        let effective_num_results = match priority {
            crate::filters::strategy::QueryPriority::Critical => num_results,
            crate::filters::strategy::QueryPriority::High => std::cmp::min(num_results, 30),
            crate::filters::strategy::QueryPriority::Medium => std::cmp::min(num_results, 20),
            crate::filters::strategy::QueryPriority::Low => std::cmp::min(num_results, 10),
        };
        
        // Pagination
        if page > 1 {
            let start = (page - 1) * effective_num_results;
            query_params.insert("start".to_string(), start.to_string());
        }
        query_params.insert("num".to_string(), effective_num_results.to_string());
        
        // Localization based on region
        let (country, language) = self.get_region_settings(region);
        if !country.is_empty() {
            query_params.insert("gl".to_string(), country.to_string());
        }
        query_params.insert("hl".to_string(), language.to_string());
        
        // Time-based filtering (skip for low priority to save tokens)
        if timeframe != "live" && !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            let tbs_value = match timeframe {
                "day" => "qdr:d",
                "week" => "qdr:w",
                "month" => "qdr:m",
                "quarter" => "qdr:m3",
                "year" => "qdr:y",
                _ => ""
            };
            if !tbs_value.is_empty() {
                query_params.insert("tbs".to_string(), tbs_value.to_string());
            }
        }

        // Include news search for market data
        if include_news && !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            query_params.insert("tbm".to_string(), "nws".to_string());
        }

        info!("🔍 Priority {} enhanced market search: {} using zone: {} (execution: {})", 
              format!("{:?}", priority), enhanced_query.clone(), zone.clone(), execution_id.clone());

        // Build URL with query parameters
        let mut search_url = "https://www.google.com/search".to_string();
        let query_string = query_params.iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        
        if !query_string.is_empty() {
            search_url = format!("{}?{}", search_url, query_string);
        }

        let mut payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
            "render": true,
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
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced market request failed: {}", e)))?;

        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Log BrightData request
        if let Err(e) = JSON_LOGGER.log_brightdata_request(
            &format!("{}_{}", execution_id, sequence),
            &zone,
            &format!("Enhanced Market: {} ({})", enhanced_query, region),
            payload.clone(),
            status,
            response_headers,
            "markdown"
        ).await {
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData enhanced market overview error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply priority-aware filters conditionally
        let filtered_content = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError(format!("{} search returned error page", source_name)));
            } else {
                let max_tokens = token_budget / 3; // Reserve tokens for formatting
                ResponseFilter::extract_high_value_financial_data(&raw_content, max_tokens)
            }
        } else {
            raw_content.clone()
        };

        // Format the results with priority awareness
        let formatted_content = self.format_market_results_with_priority(
            &filtered_content, region, page, effective_num_results, priority
        );

        Ok(json!({
            "content": filtered_content,
            "formatted_content": formatted_content,
            "search_query": enhanced_query,
            "region": region,
            "timeframe": timeframe,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget,
            "page": page,
            "num_results": effective_num_results,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }

    // ENHANCED: Priority-aware legacy market data fetching
    async fn fetch_market_overview_legacy_with_priority(
        &self, 
        market_type: &str, 
        region: &str, 
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
        sequence: u64
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        let search_url = match (region, market_type) {
            ("indian", "stocks") => "https://www.google.com/search?q=indian stock market today nifty sensex BSE NSE performance".to_string(),
            ("indian", "crypto") => "https://www.google.com/search?q=cryptocurrency market india bitcoin ethereum price today".to_string(),
            ("indian", "bonds") => "https://www.google.com/search?q=indian bond market government bonds yield RBI today".to_string(),
            ("indian", "commodities") => "https://www.google.com/search?q=commodity market india gold silver oil prices today".to_string(),
            ("indian", "overall") => "https://www.google.com/search?q=indian financial market overview today nifty sensex rupee".to_string(),
            
            ("us", "stocks") => "https://www.google.com/search?q=US stock market today dow jones s&p nasdaq performance".to_string(),
            ("us", "crypto") => "https://www.google.com/search?q=cryptocurrency market USA bitcoin ethereum price today".to_string(),
            ("us", "bonds") => "https://www.google.com/search?q=US bond market treasury yield federal reserve today".to_string(),
            ("us", "commodities") => "https://www.google.com/search?q=commodity market USA gold oil prices futures today".to_string(),
            ("us", "overall") => "https://www.google.com/search?q=US financial market overview today wall street performance".to_string(),
            
            ("global", "stocks") => "https://www.google.com/search?q=global stock market overview world indices performance today".to_string(),
            ("global", "crypto") => "https://www.google.com/search?q=global cryptocurrency market bitcoin ethereum worldwide today".to_string(),
            ("global", "bonds") => "https://www.google.com/search?q=global bond market sovereign yields worldwide today".to_string(),
            ("global", "commodities") => "https://www.google.com/search?q=global commodity market gold oil silver prices worldwide".to_string(),
            ("global", "overall") => "https://www.google.com/search?q=global financial market overview world economy today".to_string(),
            
            _ => format!("https://www.google.com/search?q={} {} market overview today", region, market_type)
        };

        let mut payload = json!({
            "url": search_url,
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
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Market overview request failed: {}", e)))?;

        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Log BrightData request
        if let Err(e) = JSON_LOGGER.log_brightdata_request(
            &format!("{}_{}", execution_id, sequence),
            &zone,
            &search_url,
            payload.clone(),
            status,
            response_headers,
            "markdown"
        ).await {
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData market overview error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply filters conditionally
        let filtered_content = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError("Market search returned error page".into()));
            } else {
                let max_tokens = token_budget / 2;
                ResponseFilter::extract_high_value_financial_data(&raw_content, max_tokens)
            }
        } else {
            raw_content.clone()
        };

        Ok(json!({
            "content": filtered_content,
            "market_type": market_type,
            "region": region,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true,
            "api_type": "legacy"
        }))
    }

    fn build_market_query_with_priority(&self, market_type: &str, region: &str, additional_terms: &str) -> String {
        let base_query = match (region, market_type) {
            ("indian", "stocks") => "indian stock market nifty sensex BSE NSE",
            ("indian", "crypto") => "cryptocurrency market india bitcoin ethereum",
            ("indian", "bonds") => "indian bond market government bonds yield RBI",
            ("indian", "commodities") => "commodity market india gold silver oil",
            ("indian", "forex") => "indian forex market rupee exchange rate",
            ("indian", "overall") => "indian financial market nifty sensex rupee",
            
            ("us", "stocks") => "US stock market dow jones s&p nasdaq",
            ("us", "crypto") => "cryptocurrency market USA bitcoin ethereum",
            ("us", "bonds") => "US bond market treasury yield federal reserve",
            ("us", "commodities") => "commodity market USA gold oil futures",
            ("us", "forex") => "US forex market dollar exchange rate",
            ("us", "overall") => "US financial market wall street",
            
            ("global", "stocks") => "global stock market world indices",
            ("global", "crypto") => "global cryptocurrency market bitcoin ethereum worldwide",
            ("global", "bonds") => "global bond market sovereign yields worldwide",
            ("global", "commodities") => "global commodity market gold oil silver worldwide",
            ("global", "forex") => "global forex market currency exchange rates",
            ("global", "overall") => "global financial market world economy",
            
            _ => &format!("{} {} market", region, market_type)
        };

        format!("{} {}", base_query, additional_terms)
    }

    fn get_region_settings(&self, region: &str) -> (&str, &str) {
        match region {
            "indian" => ("in", "en"),
            "us" => ("us", "en"),
            "asia" => ("", "en"), // No specific country for Asia region
            "europe" => ("", "en"), // No specific country for Europe region
            "global" => ("", "en"), // Empty country for global
            _ => ("us", "en")
        }
    }

    // ENHANCED: Priority-aware result formatting
    fn format_market_results_with_priority(
        &self, 
        content: &str, 
        region: &str, 
        page: u32, 
        num_results: u32, 
        priority: crate::filters::strategy::QueryPriority
    ) -> String {
        // Check if we need compact formatting
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            // Ultra-compact formatting for filtered mode
            return format!("📊 {} Market: {}", 
                region.to_uppercase(), 
                content
            );
        }

        // Regular formatting for non-filtered mode
        self.format_market_results(content, region, page, num_results)
    }

    fn format_market_results(&self, content: &str, region: &str, page: u32, num_results: u32) -> String {
        let mut formatted = String::new();
        
        // Add header with search parameters
        formatted.push_str(&format!("# Market Overview: {} Market\n\n", region.to_uppercase()));
        formatted.push_str(&format!("**Page**: {} | **Results per page**: {}\n\n", page, num_results));
        
        // Try to parse JSON response if available
        if let Ok(json_data) = serde_json::from_str::<Value>(content) {
            // If we get structured JSON, format it nicely
            if let Some(results) = json_data.get("organic_results").and_then(|r| r.as_array()) {
                formatted.push_str("## Market News & Analysis\n\n");
                for (i, result) in results.iter().take(num_results as usize).enumerate() {
                    let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("No title");
                    let link = result.get("link").and_then(|l| l.as_str()).unwrap_or("");
                    let snippet = result.get("snippet").and_then(|s| s.as_str()).unwrap_or("");
                    
                    formatted.push_str(&format!("### {}. {}\n", i + 1, title));
                    if !link.is_empty() {
                        formatted.push_str(&format!("**Source**: {}\n", link));
                    }
                    if !snippet.is_empty() {
                        formatted.push_str(&format!("**Summary**: {}\n", snippet));
                    }
                    formatted.push_str("\n");
                }
            } else {
                // JSON but no organic_results, return formatted JSON
                formatted.push_str("## Market Data\n\n");
                formatted.push_str("```json\n");
                formatted.push_str(&serde_json::to_string_pretty(&json_data).unwrap_or_else(|_| content.to_string()));
                formatted.push_str("\n```\n");
            }
        } else {
            // Plain text/markdown response
            formatted.push_str("## Market Information\n\n");
            formatted.push_str(content);
        }
        
        // Add pagination info
        if page > 1 || num_results < 100 {
            formatted.push_str(&format!("\n---\n*Page {} of market overview results*\n", page));
            if page > 1 {
                formatted.push_str("💡 *To get more results, use page parameter*\n");
            }
        }
        
        formatted
    }
}