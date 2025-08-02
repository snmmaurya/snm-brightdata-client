// src/tools/search.rs - Enhanced with BrightData SERP API parameters - FIXED VERSION
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;
use std::collections::HashMap;
use log::info;

pub struct SearchEngine;

#[async_trait]
impl Tool for SearchEngine {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search the web using various search engines via BrightData SERP API with pagination and advanced parameters"
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
            .unwrap_or(10) as u32;

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

        let execution_id = self.generate_execution_id();
        
        let result = if use_serp_api {
            self.search_with_brightdata_serp_api(
                query, engine, page, num_results, country, language,
                safe_search, time_filter, search_type, &execution_id
            ).await?
        } else {
            // Fallback to your original method
            self.search_with_brightdata(query, engine, &execution_id).await?
        };

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No results");
        
        let mcp_content = if use_serp_api {
            vec![McpContent::text(format!(
                "🔍 **Enhanced Search Results for '{}'**\n\nEngine: {} | Page: {} | Results: {} | Country: {} | Language: {}\nSearch Type: {} | Safe Search: {} | Time Filter: {}\nExecution ID: {}\n\n{}",
                query, engine, page, num_results, country, language, search_type, safe_search, time_filter, execution_id, content_text
            ))]
        } else {
            vec![McpContent::text(format!(
                "🔍 **Search Results for '{}'**\n\nEngine: {}\nExecution ID: {}\n\n{}",
                query, engine, execution_id, content_text
            ))]
        };

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl SearchEngine {
    fn generate_execution_id(&self) -> String {
        format!("search_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"))
    }

    // Enhanced method using BrightData SERP API
    async fn search_with_brightdata_serp_api(
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
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        // Build query parameters based on BrightData SERP API documentation
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), query.to_string());
        
        // Pagination
        if page > 1 {
            let start = (page - 1) * num_results;
            query_params.insert("start".to_string(), start.to_string());
        }
        query_params.insert("num".to_string(), num_results.to_string());
        
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
        
        // Search type (tbm parameter)
        if search_type != "web" {
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

        info!("🔍 Enhanced SERP API search: {} (engine: {}, page: {}, results: {}, country: {}) using zone: {} (execution: {})", 
              query, engine, page, num_results, country, zone, execution_id);

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
        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
            "render": true, // Enable JavaScript rendering for dynamic content
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
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData SERP API error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to read SERP response: {}", e)))?;

        // Format the results for better readability
        let formatted_content = self.format_search_results(&content, query, page, num_results, search_type);

        Ok(json!({
            "content": formatted_content,
            "query": query,
            "engine": engine,
            "page": page,
            "num_results": num_results,
            "country": country,
            "language": language,
            "search_type": search_type,
            "safe_search": safe_search,
            "time_filter": time_filter,
            "zone": zone,
            "execution_id": execution_id,
            "raw_response": content,
            "success": true,
            "api_type": "serp_api"
        }))
    }

    // Your original method (preserved for backward compatibility)
    async fn search_with_brightdata(&self, query: &str, engine: &str, execution_id: &str) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let search_url = self.build_search_url(engine, query);
        let zone = std::env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        info!("🔍 Search URL: {} using zone: {} (execution: {})", search_url, zone, execution_id);

        let payload = json!({
            "url": search_url,
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
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData API error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to read response: {}", e)))?;

        Ok(json!({
            "content": content,
            "query": query,
            "engine": engine,
            "search_url": search_url,
            "zone": zone,
            "execution_id": execution_id,
            "success": true,
            "api_type": "legacy"
        }))
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

    fn format_search_results(&self, content: &str, query: &str, page: u32, num_results: u32, search_type: &str) -> String {
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