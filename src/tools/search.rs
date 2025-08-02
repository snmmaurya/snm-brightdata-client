// src/tools/search.rs - COMPLETE PATCHED VERSION with enhanced token budget management
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;
use std::collections::HashMap;
use log::{info, warn};

pub struct SearchEngine;

#[async_trait]
impl Tool for SearchEngine {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search the web using various search engines via BrightData SERP API with pagination, advanced parameters, and intelligent filtering"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "engine": {
                    "type": "string",
                    "enum": ["google", "bing", "yandex", "duckduckgo"],
                    "description": "Search engine to use",
                    "default": "google"
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
                    "description": "Number of results per page (5-50)",
                    "minimum": 5,
                    "maximum": 50,
                    "default": 20
                },
                "country": {
                    "type": "string",
                    "description": "Country code for localized results (e.g., 'us', 'in', 'uk', 'ca')",
                    "default": "us"
                },
                "language": {
                    "type": "string", 
                    "description": "Language code for results (e.g., 'en', 'hi', 'es', 'fr')",
                    "default": "en"
                },
                "safe_search": {
                    "type": "string",
                    "enum": ["off", "moderate", "strict"],
                    "description": "Safe search filter level",
                    "default": "moderate"
                },
                "time_filter": {
                    "type": "string",
                    "enum": ["any", "hour", "day", "week", "month", "year"],
                    "description": "Time-based filter for results",
                    "default": "any"
                },
                "search_type": {
                    "type": "string",
                    "enum": ["web", "images", "videos", "news", "shopping"],
                    "description": "Type of search results",
                    "default": "web"
                },
                "use_serp_api": {
                    "type": "boolean",
                    "description": "Use BrightData SERP API for structured results (recommended)",
                    "default": true
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

        let engine = parameters
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("google");

        let page = parameters
            .get("page")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as u32;

        let num_results = parameters
            .get("num_results")
            .and_then(|v| v.as_i64())
            .unwrap_or(20) as u32;

        let country = parameters
            .get("country")
            .and_then(|v| v.as_str())
            .unwrap_or("us");

        let language = parameters
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("en");

        let safe_search = parameters
            .get("safe_search")
            .and_then(|v| v.as_str())
            .unwrap_or("moderate");

        let time_filter = parameters
            .get("time_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("any");

        let search_type = parameters
            .get("search_type")
            .and_then(|v| v.as_str())
            .unwrap_or("web");

        let use_serp_api = parameters
            .get("use_serp_api")
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
                return Ok(ResponseStrategy::create_response("", query, "search", "validation", json!({}), response_type));
            }

            // Budget check for search queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 100 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", query, "search", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = self.generate_execution_id();
        
        info!("🔍 Search query: '{}' (engine: {}, priority: {:?}, tokens: {})", 
              query, engine, query_priority, recommended_tokens);
        
        let result = if use_serp_api {
            self.search_with_brightdata_serp_api_with_priority(
                query, engine, page, num_results, country, language,
                safe_search, time_filter, search_type, query_priority, 
                recommended_tokens, &execution_id
            ).await?
        } else {
            // Fallback to original method with priority
            self.search_with_brightdata_with_priority(query, engine, query_priority, recommended_tokens, &execution_id).await?
        };

        let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let source_used = if use_serp_api { "Enhanced SERP" } else { "Legacy" };

        // Create appropriate response based on whether filtering is enabled
        let tool_result = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            ResponseStrategy::create_financial_response(
                "search", query, "web", source_used, content, result.clone()
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
                    "🔍 **Enhanced Search Results for '{}'**\n\nEngine: {} | Page: {} | Results: {} | Country: {} | Language: {} | Priority: {:?} | Tokens: {}\nSearch Type: {} | Safe Search: {} | Time Filter: {}\nExecution ID: {}\n\n{}",
                    query, engine, page, num_results, country, language, query_priority, recommended_tokens, search_type, safe_search, time_filter, execution_id, content_text
                ))]
            } else {
                vec![McpContent::text(format!(
                    "🔍 **Search Results for '{}'**\n\nEngine: {} | Priority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                    query, engine, query_priority, recommended_tokens, execution_id, content_text
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

impl SearchEngine {
    fn generate_execution_id(&self) -> String {
        format!("search_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"))
    }

    // ENHANCED: Token-aware response handling with priority management
    async fn handle_brightdata_response_with_priority(
        &self,
        raw_content: String,
        query: &str,
        engine: &str,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        
        // Step 1: Check if filtering is enabled
        if !std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            // No filtering - return as-is
            return Ok(json!({
                "content": raw_content,
                "formatted_content": self.format_search_results(&raw_content, query, 1, 20, "web"),
                "query": query,
                "engine": engine,
                "priority": format!("{:?}", priority),
                "token_budget": token_budget,
                "execution_id": execution_id,
                "success": true,
                "api_type": "no_filter"
            }));
        }

        // Step 2: Determine response type based on content quality and priority
        let response_type = ResponseStrategy::determine_response_type(&raw_content, query);
        
        // Step 3: Apply priority-aware filtering based on response type
        match response_type {
            ResponseType::Skip => {
                // Return minimal response, don't even process content
                return Err(BrightDataError::ToolError("Skipping low quality search source".into()));
            }
            
            ResponseType::Emergency => {
                // Extract only the most essential data (10-15 tokens max)
                let max_tokens = std::cmp::min(token_budget / 4, 15);
                let emergency_content = ResponseFilter::extract_high_value_financial_data(
                    &raw_content, 
                    max_tokens
                );
                
                return Ok(json!({
                    "content": emergency_content,
                    "formatted_content": emergency_content,
                    "response_type": "emergency",
                    "query": query,
                    "engine": engine,
                    "priority": format!("{:?}", priority),
                    "token_budget": token_budget,
                    "execution_id": execution_id,
                    "success": true,
                    "api_type": "emergency_serp"
                }));
            }
            
            ResponseType::KeyMetrics => {
                // Extract only key metrics (20-40 tokens max)
                let max_tokens = std::cmp::min(token_budget / 3, 40);
                let metrics_content = ResponseFilter::extract_high_value_financial_data(
                    &raw_content, 
                    max_tokens
                );
                
                return Ok(json!({
                    "content": metrics_content,
                    "formatted_content": metrics_content,
                    "response_type": "key_metrics", 
                    "query": query,
                    "engine": engine,
                    "priority": format!("{:?}", priority),
                    "token_budget": token_budget,
                    "execution_id": execution_id,
                    "success": true,
                    "api_type": "metrics_serp"
                }));
            }
            
            ResponseType::Summary => {
                // Create ultra-compact summary (40-60 tokens max)
                let max_chars = std::cmp::min(token_budget * 4 / 2, 200); // Reserve half tokens for formatting
                let summary_content = ResponseFilter::smart_truncate_preserving_financial_data(
                    &raw_content,
                    max_chars
                );
                
                let formatted_content = self.format_search_results_with_priority(&summary_content, query, 1, 1, "web", priority);
                
                return Ok(json!({
                    "content": summary_content,
                    "formatted_content": formatted_content,
                    "response_type": "summary",
                    "query": query,
                    "engine": engine,
                    "priority": format!("{:?}", priority),
                    "token_budget": token_budget,
                    "execution_id": execution_id,
                    "success": true,
                    "api_type": "summary_serp"
                }));
            }
            
            ResponseType::Filtered => {
                // Apply aggressive filtering (60-100 tokens max)
                let filtered_content = ResponseFilter::filter_financial_content(&raw_content);
                let max_chars = std::cmp::min(token_budget * 4 / 2, 400);
                let truncated_content = ResponseFilter::truncate_content(&filtered_content, max_chars);
                
                let formatted_content = self.format_search_results_with_priority(&truncated_content, query, 1, 10, "web", priority);
                
                return Ok(json!({
                    "content": truncated_content,
                    "formatted_content": formatted_content,
                    "response_type": "filtered",
                    "query": query,
                    "engine": engine,
                    "priority": format!("{:?}", priority),
                    "token_budget": token_budget,
                    "execution_id": execution_id,
                    "success": true,
                    "api_type": "filtered_serp"
                }));
            }
            
            _ => {
                // Fallback - should not happen
                let max_tokens = std::cmp::min(token_budget / 4, 20);
                let minimal_content = ResponseFilter::extract_high_value_financial_data(&raw_content, max_tokens);
                return Ok(json!({
                    "content": minimal_content,
                    "formatted_content": minimal_content,
                    "response_type": "fallback",
                    "query": query,
                    "engine": engine,
                    "priority": format!("{:?}", priority),
                    "token_budget": token_budget,
                    "execution_id": execution_id,
                    "success": true,
                    "api_type": "fallback_serp"
                }));
            }
        }
    }

    // ENHANCED: Priority-aware SERP API search with comprehensive token management
    async fn search_with_brightdata_serp_api_with_priority(
        &self,
        query: &str,
        engine: &str,
        page: u32,
        num_results: u32,
        country: &str,
        language: &str,
        safe_search: &str,
        time_filter: &str,
        search_type: &str,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        // ENHANCED: Adjust search parameters based on priority and token budget
        let effective_num_results = match priority {
            crate::filters::strategy::QueryPriority::Critical => num_results,
            crate::filters::strategy::QueryPriority::High => std::cmp::min(num_results, 30),
            crate::filters::strategy::QueryPriority::Medium => std::cmp::min(num_results, 20),
            crate::filters::strategy::QueryPriority::Low => std::cmp::min(num_results, 10),
        };

        // Build query parameters based on BrightData SERP API documentation
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), query.to_string());
        
        // Pagination
        if page > 1 {
            let start = (page - 1) * effective_num_results;
            query_params.insert("start".to_string(), start.to_string());
        }
        query_params.insert("num".to_string(), effective_num_results.to_string());
        
        // Localization
        query_params.insert("gl".to_string(), country.to_string()); // Geographic location
        query_params.insert("hl".to_string(), language.to_string()); // Host language
        
        // Safe search mapping
        let safe_value = match safe_search {
            "off" => "off",
            "strict" => "strict", 
            _ => "moderate" // Default
        };
        query_params.insert("safe".to_string(), safe_value.to_string());
        
        // Time-based filtering (skip for low priority to save tokens)
        if time_filter != "any" && !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
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
        
        // Search type (tbm parameter) - only for higher priority queries
        if search_type != "web" && !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            let tbm_value = match search_type {
                "images" => "isch",
                "videos" => "vid",
                "news" => "nws", 
                "shopping" => "shop",
                _ => ""
            };
            if !tbm_value.is_empty() {
                query_params.insert("tbm".to_string(), tbm_value.to_string());
            }
        }

        info!("🔍 Priority {} enhanced SERP API search: {} (engine: {}, page: {}, results: {}, country: {}) using zone: {} (execution: {})", 
              format!("{:?}", priority), query, engine, page, effective_num_results, country, zone, execution_id);

        // Build URL with query parameters for BrightData SERP API
        let mut search_url = self.get_base_search_url(engine);
        let query_string = query_params.iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        
        if !query_string.is_empty() {
            search_url = format!("{}?{}", search_url, query_string);
        }

        // Use BrightData SERP API payload with URL containing parameters
        let mut payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
            "render": true, // Enable JavaScript rendering for dynamic content
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
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced search request failed: {}", e)))?;

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
            &format!("Enhanced SERP: {} ({})", query, engine),
            payload.clone(),
            status,
            response_headers,
            "markdown"
        ).await {
            warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData SERP API error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to read SERP response: {}", e)))?;

        // ENHANCED: Use the new priority-aware filtering handler
        self.handle_brightdata_response_with_priority(raw_content, query, engine, priority, token_budget, execution_id).await
    }

    // ENHANCED: Priority-aware legacy search method
    async fn search_with_brightdata_with_priority(
        &self, 
        query: &str, 
        engine: &str, 
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let search_url = self.build_search_url(engine, query);
        let zone = std::env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        info!("🔍 Priority {} search URL: {} using zone: {} (execution: {})", 
              format!("{:?}", priority), search_url, zone, execution_id);

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
            .map_err(|e| BrightDataError::ToolError(format!("Search request failed: {}", e)))?;

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
            warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData API error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to read response: {}", e)))?;

        // ENHANCED: Use the new priority-aware filtering handler for legacy method too
        self.handle_brightdata_response_with_priority(raw_content, query, engine, priority, token_budget, execution_id).await
    }

    fn get_base_search_url(&self, engine: &str) -> String {
        match engine {
            "bing" => "https://www.bing.com/search".to_string(),
            "yandex" => "https://yandex.com/search/".to_string(),
            "duckduckgo" => "https://duckduckgo.com/".to_string(),
            _ => "https://www.google.com/search".to_string(),
        }
    }

    fn build_search_url(&self, engine: &str, query: &str) -> String {
        let encoded_query = urlencoding::encode(query);
        match engine {
            "bing" => format!("https://www.bing.com/search?q={}", encoded_query),
            "yandex" => format!("https://yandex.com/search/?text={}", encoded_query),
            "duckduckgo" => format!("https://duckduckgo.com/?q={}", encoded_query),
            _ => format!("https://www.google.com/search?q={}", encoded_query),
        }
    }

    // ENHANCED: Priority-aware result formatting
    fn format_search_results_with_priority(
        &self, 
        content: &str, 
        query: &str, 
        page: u32, 
        num_results: u32, 
        search_type: &str, 
        priority: crate::filters::strategy::QueryPriority
    ) -> String {
        // Check if we need compact formatting
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            // Ultra-compact formatting for filtered mode
            return format!("🔍 {}: {}", 
                ResponseStrategy::ultra_abbreviate_query(query), 
                content
            );
        }

        // Regular formatting for non-filtered mode
        self.format_search_results(content, query, page, num_results, search_type)
    }

    fn format_search_results(&self, content: &str, query: &str, page: u32, num_results: u32, search_type: &str) -> String {
        // Check for compact formatting first
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            // Ultra-compact formatting for filtered mode
            return format!("🔍 {}: {}", 
                ResponseStrategy::ultra_abbreviate_query(query), 
                content
            );
        }

        // Original formatting for non-filtered mode
        let mut formatted = String::new();
        
        // Add header with search parameters
        formatted.push_str(&format!("# Search Results for: {}\n\n", query));
        formatted.push_str(&format!("**Page**: {} | **Results per page**: {} | **Type**: {}\n\n", page, num_results, search_type));
        
        // Try to parse JSON response if available
        if let Ok(json_data) = serde_json::from_str::<Value>(content) {
            // If we get structured JSON, format it nicely
            if let Some(results) = json_data.get("organic_results").and_then(|r| r.as_array()) {
                formatted.push_str("## Organic Results\n\n");
                for (i, result) in results.iter().take(num_results as usize).enumerate() {
                    let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("No title");
                    let link = result.get("link").and_then(|l| l.as_str()).unwrap_or("");
                    let snippet = result.get("snippet").and_then(|s| s.as_str()).unwrap_or("");
                    
                    formatted.push_str(&format!("### {}. {}\n", i + 1, title));
                    if !link.is_empty() {
                        formatted.push_str(&format!("**URL**: {}\n", link));
                    }
                    if !snippet.is_empty() {
                        formatted.push_str(&format!("**Snippet**: {}\n", snippet));
                    }
                    formatted.push_str("\n");
                }
            } else {
                // JSON but no organic_results, return formatted JSON
                formatted.push_str("## Structured Results\n\n");
                formatted.push_str("```json\n");
                formatted.push_str(&serde_json::to_string_pretty(&json_data).unwrap_or_else(|_| content.to_string()));
                formatted.push_str("\n```\n");
            }
        } else {
            // Plain text/markdown response
            formatted.push_str("## Search Results\n\n");
            formatted.push_str(content);
        }
        
        // Add pagination info
        if page > 1 || num_results < 100 {
            formatted.push_str(&format!("\n---\n*Page {} of search results*\n", page));
            if page > 1 {
                formatted.push_str("💡 *To get more results, use page parameter*\n");
            }
        }
        
        formatted
    }
}