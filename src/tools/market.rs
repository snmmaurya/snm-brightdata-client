// src/tools/market.rs - Enhanced with BrightData SERP API parameters and optional filtering - COMPLETE VERSION
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

pub struct MarketOverviewTool;

#[async_trait]
impl Tool for MarketOverviewTool {
    fn name(&self) -> &str {
        "get_market_overview"
    }

    fn description(&self) -> &str {
        "Get comprehensive market overview with enhanced search parameters including pagination and localization"
    }

    fn input_schema(&self) -> Value {
        json!({
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
                    "default": "indian",
                    "description": "Market region"
                },
                "page": {
                    "type": "integer",
                    "description": "Page number for pagination (1-based)",
                    "minimum": 1,
                    "maximum": 1,
                    "default": 1
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results per page (10-100)",
                    "minimum": 1,
                    "maximum": 1,
                    "default": 1
                },
                "time_filter": {
                    "type": "string",
                    "enum": ["any", "hour", "day", "week", "month", "year"],
                    "description": "Time-based filter for results",
                    "default": "day"
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

        let page = parameters
            .get("page")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as u32;

        let num_results = parameters
            .get("num_results")
            .and_then(|v| v.as_i64())
            .unwrap_or(20) as u32;

        let time_filter = parameters
            .get("time_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("day");

        let use_serp_api = parameters
            .get("use_serp_api")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", &format!("{} market overview", market_type));
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", &format!("{} market overview", market_type), region, "validation", json!({}), response_type));
            }
        }

        let execution_id = format!("market_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        let result = if use_serp_api {
            self.fetch_market_overview_enhanced(
                market_type, region, page, num_results, time_filter, &execution_id
            ).await?
        } else {
            self.fetch_market_overview_legacy(market_type, region, &execution_id).await?
        };

        let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let source_used = if use_serp_api { "Enhanced SERP" } else { "Legacy" };

        // Create appropriate response based on whether filtering is enabled
        let tool_result = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            ResponseStrategy::create_financial_response(
                "market", &format!("{} market", market_type), region, source_used, content, result.clone()
            )
        } else {
            // No filtering - create standard response
            let content_text = if use_serp_api {
                result.get("formatted_content").and_then(|c| c.as_str()).unwrap_or(content)
            } else {
                content
            };

            let mcp_content = if use_serp_api {
                vec![McpContent::text(format!(
                    "📊 **Enhanced Market Overview - {} Market ({})**\n\nPage: {} | Results: {} | Time Filter: {}\nExecution ID: {}\n\n{}",
                    market_type.to_uppercase(),
                    region.to_uppercase(),
                    page,
                    num_results,
                    time_filter,
                    execution_id,
                    content_text
                ))]
            } else {
                vec![McpContent::text(format!(
                    "📊 **Market Overview - {} Market ({})**\n\nExecution ID: {}\n\n{}",
                    market_type.to_uppercase(),
                    region.to_uppercase(),
                    execution_id,
                    content_text
                ))]
            };
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
}

impl MarketOverviewTool {
    async fn fetch_market_overview_enhanced(
        &self, 
        market_type: &str, 
        region: &str, 
        page: u32,
        num_results: u32,
        time_filter: &str,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        // Build search query based on parameters
        let query = self.build_market_query(market_type, region);
        
        // Build enhanced query parameters
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), query.clone());
        
        // Pagination
        if page > 1 {
            let start = (page - 1) * num_results;
            query_params.insert("start".to_string(), start.to_string());
        }
        query_params.insert("num".to_string(), num_results.to_string());
        
        // Localization based on region
        let (country, language) = self.get_region_settings(region);
        if !country.is_empty() {
            query_params.insert("gl".to_string(), country.to_string());
        }
        query_params.insert("hl".to_string(), language.to_string());
        
        // Time-based filtering
        if time_filter != "any" {
            let tbs_value = match time_filter {
                "hour" => "qdr:h",
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

        // News search for market data
        query_params.insert("tbm".to_string(), "nws".to_string());

        log::info!("🔍 Enhanced Market API search: {} (region: {}, page: {}, results: {}) using zone: {} (execution: {})", 
                   query, region, page, num_results, zone, execution_id);

        // Build URL with query parameters
        let mut search_url = "https://www.google.com/search".to_string();
        let query_string = query_params.iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        
        if !query_string.is_empty() {
            search_url = format!("{}?{}", search_url, query_string);
        }

        // Use BrightData SERP API payload with URL containing parameters
        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
            "render": true,
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
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced market request failed: {}", e)))?;

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
            &format!("Enhanced Market: {} ({})", query, region),
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

        // Apply filters conditionally based on environment variable
        let filtered_content = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError("Enhanced market search returned error page".into()));
            } else if ResponseStrategy::should_try_next_source(&raw_content) {
                return Err(BrightDataError::ToolError("Content quality too low".into()));
            } else {
                ResponseFilter::filter_financial_content(&raw_content)
            }
        } else {
            raw_content.clone()
        };

        // Format the results
        let formatted_content = self.format_market_results(&filtered_content, market_type, region, page, num_results);

        Ok(json!({
            "content": filtered_content,
            "formatted_content": formatted_content,
            "query": query,
            "market_type": market_type,
            "region": region,
            "page": page,
            "num_results": num_results,
            "time_filter": time_filter,
            "zone": zone,
            "execution_id": execution_id,
            "raw_response": raw_content,
            "success": true,
            "api_type": "enhanced_serp"
        }))
    }

    async fn fetch_market_overview_legacy(&self, market_type: &str, region: &str, execution_id: &str) -> Result<Value, BrightDataError> {
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

        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
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
            .map_err(|e| BrightDataError::ToolError(format!("Market overview request failed: {}", e)))?;

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
                ResponseFilter::filter_financial_content(&raw_content)
            }
        } else {
            raw_content.clone()
        };

        Ok(json!({
            "content": filtered_content,
            "market_type": market_type,
            "region": region,
            "execution_id": execution_id,
            "success": true,
            "api_type": "legacy"
        }))
    }

    fn build_market_query(&self, market_type: &str, region: &str) -> String {
        match (region, market_type) {
            ("indian", "stocks") => "indian stock market today nifty sensex BSE NSE performance".to_string(),
            ("indian", "crypto") => "cryptocurrency market india bitcoin ethereum price today".to_string(),
            ("indian", "bonds") => "indian bond market government bonds yield RBI today".to_string(),
            ("indian", "commodities") => "commodity market india gold silver oil prices today".to_string(),
            ("indian", "overall") => "indian financial market overview today nifty sensex rupee".to_string(),
            
            ("us", "stocks") => "US stock market today dow jones s&p nasdaq performance".to_string(),
            ("us", "crypto") => "cryptocurrency market USA bitcoin ethereum price today".to_string(),
            ("us", "bonds") => "US bond market treasury yield federal reserve today".to_string(),
            ("us", "commodities") => "commodity market USA gold oil prices futures today".to_string(),
            ("us", "overall") => "US financial market overview today wall street performance".to_string(),
            
            ("global", "stocks") => "global stock market overview world indices performance today".to_string(),
            ("global", "crypto") => "global cryptocurrency market bitcoin ethereum worldwide today".to_string(),
            ("global", "bonds") => "global bond market sovereign yields worldwide today".to_string(),
            ("global", "commodities") => "global commodity market gold oil silver prices worldwide".to_string(),
            ("global", "overall") => "global financial market overview world economy today".to_string(),
            
            _ => format!("{} {} market overview today", region, market_type)
        }
    }

    fn get_region_settings(&self, region: &str) -> (&str, &str) {
        match region {
            "indian" => ("in", "en"),
            "us" => ("us", "en"),
            "global" => ("", "en"), // Empty country for global
            _ => ("us", "en")
        }
    }

    fn format_market_results(&self, content: &str, market_type: &str, region: &str, page: u32, num_results: u32) -> String {
        let mut formatted = String::new();
        
        // Add header with search parameters
        formatted.push_str(&format!("# Market Overview: {} Market ({})\n\n", 
            market_type.to_uppercase(), region.to_uppercase()));
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