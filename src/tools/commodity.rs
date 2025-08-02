// src/tools/commodity.rs - Enhanced with market-specific sources and real-time data
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

pub struct CommodityDataTool;

#[async_trait]
impl Tool for CommodityDataTool {
    fn name(&self) -> &str {
        "get_commodity_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive commodity prices and market data with real-time updates and market-specific sources"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Commodity name (gold, silver, crude oil), commodity symbol (GC, SI, CL), or commodity market overview"
                },
                "commodity_type": {
                    "type": "string",
                    "enum": ["precious_metals", "energy", "agricultural", "industrial_metals", "livestock", "all"],
                    "default": "all",
                    "description": "Category of commodity for targeted data sources"
                },
                "market_region": {
                    "type": "string",
                    "enum": ["global", "us", "asia", "europe", "india"],
                    "default": "global",
                    "description": "Regional market focus"
                },
                "data_source": {
                    "type": "string",
                    "enum": ["search", "direct", "auto"],
                    "default": "auto",
                    "description": "Data source strategy - search (SERP), direct (commodity exchanges), auto (smart selection)"
                },
                "time_range": {
                    "type": "string",
                    "enum": ["realtime", "day", "week", "month", "year"],
                    "default": "realtime",
                    "description": "Time range for price data"
                },
                "include_futures": {
                    "type": "boolean",
                    "default": true,
                    "description": "Include futures contract prices"
                },
                "include_analysis": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include market analysis and trends"
                },
                "currency": {
                    "type": "string",
                    "enum": ["USD", "EUR", "INR", "CNY", "JPY"],
                    "default": "USD",
                    "description": "Currency for price display"
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

        let commodity_type = parameters
            .get("commodity_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let market_region = parameters
            .get("market_region")
            .and_then(|v| v.as_str())
            .unwrap_or("global");

        let data_source = parameters
            .get("data_source")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let time_range = parameters
            .get("time_range")
            .and_then(|v| v.as_str())
            .unwrap_or("realtime");

        let include_futures = parameters
            .get("include_futures")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_analysis = parameters
            .get("include_analysis")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let currency = parameters
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("USD");

        // Early validation using strategy
        let response_type = ResponseStrategy::determine_response_type("", query);
        if matches!(response_type, ResponseType::Empty) {
            return Ok(ResponseStrategy::create_response("", query, market_region, "validation", json!({}), response_type));
        }

        let execution_id = format!("commodity_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        match self.fetch_commodity_data_with_fallbacks(
            query, commodity_type, market_region, data_source, time_range,
            include_futures, include_analysis, currency, &execution_id
        ).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let source_used = result.get("source_used").and_then(|s| s.as_str()).unwrap_or("Unknown");
                
                // Create appropriate response
                let tool_result = ResponseStrategy::create_financial_response(
                    "commodity", query, market_region, source_used, content, result.clone()
                );
                
                Ok(ResponseStrategy::apply_size_limits(tool_result))
            }
            Err(e) => {
                Ok(ResponseStrategy::create_error_response(query, &e.to_string()))
            }
        }
    }
}

impl CommodityDataTool {
    async fn fetch_commodity_data_with_fallbacks(
        &self,
        query: &str,
        commodity_type: &str,
        market_region: &str,
        data_source: &str,
        time_range: &str,
        include_futures: bool,
        include_analysis: bool,
        currency: &str,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let sources_to_try = self.build_prioritized_sources(query, commodity_type, market_region, data_source);
        let mut last_error = None;

        for (sequence, (source_type, url_or_query, source_name)) in sources_to_try.iter().enumerate() {
            match source_type.as_str() {
                "direct" => {
                    match self.fetch_direct_commodity_data(
                        url_or_query, query, commodity_type, market_region, 
                        source_name, execution_id, sequence as u64
                    ).await {
                        Ok(mut result) => {
                            let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            
                            if !ResponseStrategy::should_try_next_source(content) {
                                result["source_used"] = json!(source_name);
                                result["data_source_type"] = json!("direct");
                                return Ok(result);
                            }
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                "search" => {
                    match self.fetch_search_commodity_data(
                        url_or_query, commodity_type, market_region, time_range,
                        include_futures, include_analysis, currency, source_name, 
                        execution_id, sequence as u64
                    ).await {
                        Ok(mut result) => {
                            let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            
                            if !ResponseStrategy::should_try_next_source(content) {
                                result["source_used"] = json!(source_name);
                                result["data_source_type"] = json!("search");
                                return Ok(result);
                            }
                        }
                        Err(e) => last_error = Some(e),
                    }
                }
                _ => continue,
            }
        }

        Err(last_error.unwrap_or_else(|| BrightDataError::ToolError("All commodity data sources failed".into())))
    }

    fn build_prioritized_sources(&self, query: &str, commodity_type: &str, market_region: &str, data_source: &str) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();
        let query_lower = query.to_lowercase();

        match data_source {
            "direct" => {
                sources.extend(self.get_direct_sources(commodity_type, market_region));
            }
            "search" => {
                sources.extend(self.get_search_sources(query, commodity_type, market_region));
            }
            "auto" | _ => {
                // Smart selection based on query content
                if query_lower.contains("price") || query_lower.contains("futures") || query_lower.contains("contract") {
                    // For price-specific queries, prioritize direct sources
                    sources.extend(self.get_direct_sources(commodity_type, market_region));
                    sources.extend(self.get_search_sources(query, commodity_type, market_region));
                } else {
                    // For general commodity queries, prioritize search for broader context
                    sources.extend(self.get_search_sources(query, commodity_type, market_region));
                    sources.extend(self.get_direct_sources(commodity_type, market_region));
                }
            }
        }

        sources
    }

    fn get_direct_sources(&self, commodity_type: &str, market_region: &str) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();

        match market_region {
            "india" => {
                sources.push(("direct".to_string(), "https://www.mcxindia.com/market-data/live-rates".to_string(), "MCX India".to_string()));
                sources.push(("direct".to_string(), "https://www.ncdex.com/market/live-rates".to_string(), "NCDEX".to_string()));
                if commodity_type == "precious_metals" || commodity_type == "all" {
                    sources.push(("direct".to_string(), "https://www.goldpriceindia.com/".to_string(), "Gold Price India".to_string()));
                }
            }
            "us" => {
                sources.push(("direct".to_string(), "https://www.cmegroup.com/markets.html".to_string(), "CME Group".to_string()));
                sources.push(("direct".to_string(), "https://www.theice.com/market-data".to_string(), "ICE Markets".to_string()));
                if commodity_type == "energy" || commodity_type == "all" {
                    sources.push(("direct".to_string(), "https://www.eia.gov/petroleum/".to_string(), "EIA Energy".to_string()));
                }
            }
            "europe" => {
                sources.push(("direct".to_string(), "https://www.theice.com/market-data/dashboard".to_string(), "ICE Europe".to_string()));
                sources.push(("direct".to_string(), "https://www.lme.com/Metals".to_string(), "London Metal Exchange".to_string()));
            }
            "asia" => {
                sources.push(("direct".to_string(), "https://www.tocom.or.jp/market/".to_string(), "TOCOM".to_string()));
                sources.push(("direct".to_string(), "https://www.shfe.com.cn/en/".to_string(), "Shanghai Futures".to_string()));
            }
            "global" | _ => {
                // Global commodity sources
                sources.push(("direct".to_string(), "https://www.investing.com/commodities/".to_string(), "Investing.com Commodities".to_string()));
                sources.push(("direct".to_string(), "https://www.bloomberg.com/markets/commodities".to_string(), "Bloomberg Commodities".to_string()));
                sources.push(("direct".to_string(), "https://www.marketwatch.com/investing/commodities".to_string(), "MarketWatch Commodities".to_string()));
                
                // Commodity-specific sources
                match commodity_type {
                    "precious_metals" => {
                        sources.push(("direct".to_string(), "https://www.kitco.com/market/".to_string(), "Kitco Metals".to_string()));
                        sources.push(("direct".to_string(), "https://www.lbma.org.uk/prices-and-data".to_string(), "LBMA".to_string()));
                    }
                    "energy" => {
                        sources.push(("direct".to_string(), "https://oilprice.com/".to_string(), "Oil Price".to_string()));
                        sources.push(("direct".to_string(), "https://www.eia.gov/petroleum/".to_string(), "EIA".to_string()));
                    }
                    "agricultural" => {
                        sources.push(("direct".to_string(), "https://www.cbot.com/".to_string(), "CBOT".to_string()));
                        sources.push(("direct".to_string(), "https://www.usda.gov/topics/data".to_string(), "USDA".to_string()));
                    }
                    "industrial_metals" => {
                        sources.push(("direct".to_string(), "https://www.lme.com/Metals".to_string(), "LME".to_string()));
                    }
                    _ => {}
                }
            }
        }

        sources
    }

    fn get_search_sources(&self, query: &str, commodity_type: &str, market_region: &str) -> Vec<(String, String, String)> {
        let mut sources = Vec::new();
        
        let region_terms = match market_region {
            "india" => "india MCX NCDEX commodity exchange rupee INR",
            "us" => "united states CME CBOT futures contract dollar USD",
            "europe" => "europe ICE LME futures contract euro EUR",
            "asia" => "asia TOCOM shanghai futures contract",
            "global" => "global international commodity futures trading",
            _ => "commodity futures trading market price"
        };

        let commodity_terms = match commodity_type {
            "precious_metals" => "gold silver platinum palladium precious metals spot price",
            "energy" => "crude oil natural gas gasoline heating oil energy futures",
            "agricultural" => "wheat corn soybeans rice agricultural commodity farming",
            "industrial_metals" => "copper aluminum zinc nickel industrial metals LME",
            "livestock" => "cattle hogs pork livestock futures meat",
            _ => "commodity futures spot price market trading"
        };

        // Enhanced search queries
        sources.push(("search".to_string(), 
            format!("{} {} {} current price futures today", query, region_terms, commodity_terms),
            "Enhanced Commodity Search".to_string()));

        sources.push(("search".to_string(), 
            format!("{} {} latest trading data market analysis", query, commodity_terms),
            "Commodity Market Analysis".to_string()));

        sources.push(("search".to_string(), 
            format!("{} {} price chart trends technical analysis", query, region_terms),
            "Commodity Trends Search".to_string()));

        sources
    }

    async fn fetch_direct_commodity_data(
        &self,
        url: &str,
        query: &str,
        commodity_type: &str,
        market_region: &str,
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

        info!("🥇 Direct commodity data fetch from {} using zone: {} (execution: {})", source_name, zone, execution_id);

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
            .map_err(|e| BrightDataError::ToolError(format!("Direct commodity data request failed: {}", e)))?;

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
                "BrightData direct commodity data error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply filters
        let filtered_content = if ResponseFilter::is_error_page(&raw_content) {
            return Err(BrightDataError::ToolError(format!("{} returned error page", source_name)));
        } else {
            ResponseFilter::filter_financial_content(&raw_content)
        };

        Ok(json!({
            "content": filtered_content,
            "query": query,
            "commodity_type": commodity_type,
            "market_region": market_region,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }

    async fn fetch_search_commodity_data(
        &self,
        search_query: &str,
        commodity_type: &str,
        market_region: &str,
        time_range: &str,
        include_futures: bool,
        include_analysis: bool,
        currency: &str,
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
        
        if include_futures {
            enhanced_query.push_str(" futures contract trading");
        }
        
        if include_analysis {
            enhanced_query.push_str(" market analysis trends forecast");
        }

        if currency != "USD" {
            enhanced_query.push_str(&format!(" price {}", currency));
        }

        match time_range {
            "realtime" => enhanced_query.push_str(" live current real time"),
            "day" => enhanced_query.push_str(" today daily"),
            "week" => enhanced_query.push_str(" this week weekly"),
            "month" => enhanced_query.push_str(" this month monthly"),
            "year" => enhanced_query.push_str(" this year annual"),
            _ => {}
        }

        // Build SERP API query parameters
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), enhanced_query.clone());
        query_params.insert("num".to_string(), "20".to_string()); // More results for better data
        
        // Set geographic location based on market region
        let country_code = match market_region {
            "india" => "in",
            "us" => "us",
            "europe" => "de", // Default to Germany for Europe
            "asia" => "jp", // Default to Japan for Asia
            _ => "us" // Default to US for global
        };
        query_params.insert("gl".to_string(), country_code.to_string());
        query_params.insert("hl".to_string(), "en".to_string());
        
        // Time-based filtering for recent data
        if time_range != "year" {
            let tbs_value = match time_range {
                "realtime" | "day" => "qdr:d",
                "week" => "qdr:w",
                "month" => "qdr:m",
                _ => ""
            };
            if !tbs_value.is_empty() {
                query_params.insert("tbs".to_string(), tbs_value.to_string());
            }
        }

        info!("🔍 Enhanced commodity search: {} using zone: {} (execution: {})", enhanced_query.clone(), zone.clone(), execution_id.clone());

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
            .map_err(|e| BrightDataError::ToolError(format!("Commodity search request failed: {}", e)))?;

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
            &format!("Commodity Search: {}", enhanced_query),
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
                "BrightData commodity search error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Apply filters
        let filtered_content = if ResponseFilter::is_error_page(&raw_content) {
            return Err(BrightDataError::ToolError(format!("{} search returned error page", source_name)));
        } else {
            ResponseFilter::filter_financial_content(&raw_content)
        };

        Ok(json!({
            "content": filtered_content,
            "search_query": enhanced_query,
            "commodity_type": commodity_type,
            "market_region": market_region,
            "time_range": time_range,
            "currency": currency,
            "execution_id": execution_id,
            "sequence": sequence,
            "success": true
        }))
    }
}