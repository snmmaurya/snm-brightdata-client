// src/tools/bond.rs - Enhanced with optional filtering via TRUNCATE_FILTER env var
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

pub struct BondDataTool;

#[async_trait]
impl Tool for BondDataTool {
    fn name(&self) -> &str {
        "get_bond_data"
    }

    fn description(&self) -> &str {
        "Get bond market data including yields, government bonds, corporate bonds, and bond market trends with smart data source selection"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Bond type (government bonds, corporate bonds), yield query (10-year yield), or bond market analysis"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian",
                    "description": "Market region"
                },
                "bond_type": {
                    "type": "string",
                    "enum": ["government", "corporate", "municipal", "treasury", "sovereign", "all"],
                    "default": "all",
                    "description": "Specific type of bonds to focus on"
                },
                "maturity": {
                    "type": "string",
                    "enum": ["short", "medium", "long", "all"],
                    "default": "all",
                    "description": "Bond maturity period - short (1-3y), medium (3-10y), long (10y+)"
                },
                "data_source": {
                    "type": "string",
                    "enum": ["search", "direct", "auto"],
                    "default": "auto",
                    "description": "Data source strategy - search (SERP), direct (financial sites), auto (smart selection)"
                },
                "time_filter": {
                    "type": "string",
                    "enum": ["any", "day", "week", "month", "year"],
                    "default": "month",
                    "description": "How recent the bond data should be"
                },
                "include_rates": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include current interest rates and yield curves"
                },
                "include_analysis": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include market analysis and trends"
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

        let bond_type = parameters
            .get("bond_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let maturity = parameters
            .get("maturity")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let data_source = parameters
            .get("data_source")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let time_filter = parameters
            .get("time_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("month");

        let include_rates = parameters
            .get("include_rates")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_analysis = parameters
            .get("include_analysis")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", query, market, "validation", json!({}), response_type));
            }
        }

        let execution_id = format!("bond_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        match self.fetch_bond_data_with_fallbacks(
            query, market, bond_type, maturity, data_source, time_filter,
            include_rates, include_analysis, &execution_id
        ).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let source_used = result.get("source_used").and_then(|s| s.as_str()).unwrap_or("Unknown");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "bond", query, market, source_used, content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "🏛️ **Bond Data for: {}**\n\nMarket: {} | Bond Type: {} | Maturity: {}\nSource: {} | Time Filter: {}\n\n{}",
                        query, market, bond_type, maturity, source_used, time_filter, content
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
                    Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl BondDataTool {
    async fn fetch_bond_data_with_fallbacks(
        &self,
        query: &str,
        market: &str,
        bond_type: &str,
        maturity: &str,
        data_source: &str,
        time_filter: &str,
        include_rates: bool,
        include_analysis: bool,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let sources_to_try = self.build_prioritized_sources(query, market, bond_type, data_source);
        let mut last_error = None;

        for (sequence, (source_type, url_or_query, source_name)) in sources_to_try.iter().enumerate() {
            match source_type.as_str() {
                "direct" => {
                    match self.fetch_direct_bond_data(url_or_query, query, market, source_name, execution_id, sequence as u64).await {
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
                            return Ok(result);
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                "search" => {
                    match self.fetch_search_bond_data(
                        url_or_query, market, bond_type, maturity, time_filter,
                        include_rates, include_analysis, source_name, execution_id, sequence as u64
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
                            result["data_source_type"] = json!("search");
                            return Ok(result);
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                _ => continue,
            }
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("All bond data sources failed".into())))
    }

    fn build_prioritized_sources(&self, query: &str, market: &str, bond_type: &str, data_source: &str) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();
        let query_lower = query.to_lowercase();

        match data_source {
            "direct" => {
                sources.extend(self.get_direct_sources(market, bond_type));
            }
            "search" => {
                sources.extend(self.get_search_sources(query, market, bond_type));
            }
            "auto" | _ => {
                // Smart selection based on query content
                if query_lower.contains("yield") || query_lower.contains("rate") || query_lower.contains("10 year") {
                    // For yield-specific queries, prioritize direct sources
                    sources.extend(self.get_direct_sources(market, bond_type));
                    sources.extend(self.get_search_sources(query, market, bond_type));
                } else {
                    // For general bond queries, prioritize search for broader context
                    sources.extend(self.get_search_sources(query, market, bond_type));
                    sources.extend(self.get_direct_sources(market, bond_type));
                }
            }
        }

        sources
    }

    fn get_direct_sources(&self, market: &str, bond_type: &str) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();

        match market {
            "indian" => {
                sources.push(("direct".to_string(), "https://www.rbi.org.in/scripts/BS_PressReleaseDisplay.aspx".to_string(), "RBI Press Releases".to_string()));
                sources.push(("direct".to_string(), "https://www.nseindia.com/market-data/bonds-debentures".to_string(), "NSE Bonds".to_string()));
                sources.push(("direct".to_string(), "https://www.bseindia.com/markets/debt/debt_main.aspx".to_string(), "BSE Debt Market".to_string()));
                if bond_type == "government" || bond_type == "all" {
                    sources.push(("direct".to_string(), "https://dbie.rbi.org.in/DBIE/dbie.rbi?site=home".to_string(), "RBI Database".to_string()));
                }
            }
            "us" => {
                sources.push(("direct".to_string(), "https://www.treasury.gov/resource-center/data-chart-center/interest-rates/".to_string(), "US Treasury".to_string()));
                sources.push(("direct".to_string(), "https://www.federalreserve.gov/releases/h15/".to_string(), "Federal Reserve H.15".to_string()));
                sources.push(("direct".to_string(), "https://fred.stlouisfed.org/categories/22".to_string(), "FRED Economic Data".to_string()));
                if bond_type == "corporate" || bond_type == "all" {
                    sources.push(("direct".to_string(), "https://www.finra.org/investors/market-data".to_string(), "FINRA Market Data".to_string()));
                }
            }
            "global" | _ => {
                sources.push(("direct".to_string(), "https://www.investing.com/rates-bonds/".to_string(), "Investing.com Bonds".to_string()));
                sources.push(("direct".to_string(), "https://www.bloomberg.com/markets/rates-bonds".to_string(), "Bloomberg Bonds".to_string()));
                sources.push(("direct".to_string(), "https://www.marketwatch.com/investing/bonds".to_string(), "MarketWatch Bonds".to_string()));
            }
        }

        sources
    }

    fn get_search_sources(&self, query: &str, market: &str, bond_type: &str) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();
        let market_terms = match market {
            "indian" => "india RBI government bond yield NSE BSE debt market",
            "us" => "united states treasury federal reserve bond yield corporate",
            "global" => "global international sovereign bond yield market",
            _ => "bond yield market analysis"
        };

        let bond_type_terms = match bond_type {
            "government" => "government treasury sovereign bond yield",
            "corporate" => "corporate bond credit rating yield spread",
            "municipal" => "municipal bond tax free yield",
            "treasury" => "treasury bond government yield curve",
            "sovereign" => "sovereign bond international yield",
            _ => "bond yield interest rate market"
        };

        // Enhanced search queries
        sources.push(("search".to_string(), 
            format!("{} {} {} current rate yield today", query, market_terms, bond_type_terms),
            "Enhanced Bond Search".to_string()));

        sources.push(("search".to_string(), 
            format!("{} {} latest data yield curve analysis", query, market_terms),
            "Bond Market Analysis".to_string()));

        sources.push(("search".to_string(), 
            format!("{} {} performance trends rating", query, bond_type_terms),
            "Bond Performance Search".to_string()));

        sources
    }

    async fn fetch_direct_bond_data(
        &self,
        url: &str,
        query: &str,
        market: &str,
        source_name: &str,
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

        info!("🏛️ Direct bond data fetch from {} using zone: {} (execution: {})", source_name, zone, execution_id);

        let payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw",
            "data_format": "markdown",
            "render": true // Enable JavaScript rendering for dynamic content
        });

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
            .map_err(|e| BrightDataError::ToolError(format!("Direct bond data request failed: {}", e)))?;

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
                "BrightData direct bond data error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply filters conditionally based on environment variable
        let filtered_content = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError(format!("{} returned error page", source_name)));
            } else {
                ResponseFilter::filter_financial_content(&raw_content)
            }
        } else {
            raw_content.clone()
        };

        Ok(json!({
            "content": filtered_content,
            "query": query,
            "market": market,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }

    async fn fetch_search_bond_data(
        &self,
        search_query: &str,
        market: &str,
        bond_type: &str,
        maturity: &str,
        time_filter: &str,
        include_rates: bool,
        include_analysis: bool,
        source_name: &str,
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

        // Build enhanced search query
        let mut enhanced_query = search_query.to_string();
        
        if include_rates {
            enhanced_query.push_str(" current yield rate");
        }
        
        if include_analysis {
            enhanced_query.push_str(" market analysis trends forecast");
        }

        match maturity {
            "short" => enhanced_query.push_str(" short term 1-3 year"),
            "medium" => enhanced_query.push_str(" medium term 3-10 year"),
            "long" => enhanced_query.push_str(" long term 10+ year"),
            _ => {}
        }

        // Build SERP API query parameters
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), enhanced_query.clone());
        query_params.insert("num".to_string(), "20".to_string()); // More results for better data
        
        // Set geographic location based on market
        let country_code = match market {
            "indian" => "in",
            "us" => "us",
            _ => "us" // Default to US for global
        };
        query_params.insert("gl".to_string(), country_code.to_string());
        query_params.insert("hl".to_string(), "en".to_string());
        
        // Time-based filtering
        if time_filter != "any" {
            let tbs_value = match time_filter {
                "day" => "qdr:d",
                "week" => "qdr:w",
                "month" => "qdr:m",
                "year" => "qdr:y",
                _ => ""
            };
            if !tbs_value.is_empty() {
                query_params.insert("tbs".to_string(), tbs_value.to_string());
            }
        }

        info!("🔍 Enhanced bond search: {} using zone: {} (execution: {})", enhanced_query.clone(), zone.clone(), execution_id.clone());

        let payload = json!({
            "zone": zone,
            "url": "https://www.google.com/search",
            "format": "json",
            "query_params": query_params,
            "render": true,
            "data_format": "markdown"
        });

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
            .map_err(|e| BrightDataError::ToolError(format!("Bond search request failed: {}", e)))?;

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
            &format!("Bond Search: {}", enhanced_query),
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
                "BrightData bond search error {}: {}",
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
                return Err(BrightDataError::ToolError(format!("{} search returned error page", source_name)));
            } else {
                ResponseFilter::filter_financial_content(&raw_content)
            }
        } else {
            raw_content.clone()
        };

        Ok(json!({
            "content": filtered_content,
            "search_query": enhanced_query,
            "market": market,
            "bond_type": bond_type,
            "maturity": maturity,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }
}