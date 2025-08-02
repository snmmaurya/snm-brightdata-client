// src/tools/crypto.rs - Enhanced with BrightData SERP API parameters and filters
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

pub struct CryptoDataTool;

#[async_trait]
impl Tool for CryptoDataTool {
    fn name(&self) -> &str {
        "get_crypto_data"
    }

    fn description(&self) -> &str {
        "Get cryptocurrency data with enhanced search parameters including prices, market cap, trading volumes with intelligent filtering"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Crypto symbol (BTC, ETH, ADA), crypto name (Bitcoin, Ethereum), comparison query (BTC vs ETH), or market overview (crypto market today, top cryptocurrencies)"
                },
                "page": {
                    "type": "integer",
                    "description": "Page number for pagination (1-based)",
                    "minimum": 1,
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
                    "description": "Time-based filter for crypto data",
                    "default": "day"
                },
                "crypto_type": {
                    "type": "string",
                    "enum": ["any", "bitcoin", "altcoins", "defi", "nft", "stablecoins"],
                    "description": "Type of cryptocurrency to focus on",
                    "default": "any"
                },
                "data_points": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["price", "market_cap", "volume", "change", "supply", "dominance", "fear_greed"]
                    },
                    "description": "Specific data points to focus on",
                    "default": ["price", "market_cap", "volume"]
                },
                "safe_search": {
                    "type": "string",
                    "enum": ["off", "moderate", "strict"],
                    "description": "Safe search filter level",
                    "default": "moderate"
                },
                "use_serp_api": {
                    "type": "boolean",
                    "description": "Use enhanced SERP API with advanced parameters",
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

        let crypto_type = parameters
            .get("crypto_type")
            .and_then(|v| v.as_str())
            .unwrap_or("any");

        let data_points = parameters
            .get("data_points")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["price", "market_cap", "volume"]);

        let safe_search = parameters
            .get("safe_search")
            .and_then(|v| v.as_str())
            .unwrap_or("moderate");

        let use_serp_api = parameters
            .get("use_serp_api")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Early validation
        let response_type = ResponseStrategy::determine_response_type("", query);
        if matches!(response_type, ResponseType::Empty) {
            return Ok(ResponseStrategy::create_response("", query, "crypto", "validation", json!({}), response_type));
        }

        let execution_id = format!("crypto_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        let result = if use_serp_api {
            self.fetch_crypto_data_enhanced(
                query, page, num_results, time_filter, crypto_type, &data_points, safe_search, &execution_id
            ).await?
        } else {
            self.fetch_crypto_data_legacy(query, &execution_id).await?
        };

        let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
        
        let content_text = if use_serp_api {
            result.get("formatted_content").and_then(|c| c.as_str()).unwrap_or(content)
        } else {
            content
        };
        
        let mcp_content = if use_serp_api {
            vec![McpContent::text(format!(
                "💰 **Enhanced Crypto Data for {}**\n\nCrypto Type: {} | Data Points: {:?}\nPage: {} | Results: {} | Time Filter: {} | Safe Search: {}\nExecution ID: {}\n\n{}",
                query,
                crypto_type,
                data_points,
                page,
                num_results,
                time_filter,
                safe_search,
                execution_id,
                content_text
            ))]
        } else {
            vec![McpContent::text(format!(
                "💰 **Crypto Data for {}**\n\nExecution ID: {}\n\n{}",
                query,
                execution_id,
                content_text
            ))]
        };

        // Use filters to create appropriate response
        let tool_result = ToolResult::success_with_raw(mcp_content, result);
        Ok(ResponseStrategy::apply_size_limits(tool_result))
    }
}

impl CryptoDataTool {
    async fn fetch_crypto_data_enhanced(
        &self, 
        query: &str, 
        page: u32,
        num_results: u32,
        time_filter: &str,
        crypto_type: &str,
        data_points: &[&str],
        safe_search: &str,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        // Build enhanced search query
        let search_query = self.build_crypto_query(query, crypto_type, data_points);
        
        // Build enhanced query parameters
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), search_query.clone());
        
        // Pagination
        if page > 1 {
            let start = (page - 1) * num_results;
            query_params.insert("start".to_string(), start.to_string());
        }
        query_params.insert("num".to_string(), num_results.to_string());
        
        // Global settings for crypto (not region-specific)
        query_params.insert("gl".to_string(), "us".to_string());
        query_params.insert("hl".to_string(), "en".to_string());
        
        // Safe search
        let safe_value = match safe_search {
            "off" => "off",
            "strict" => "strict",
            _ => "moderate"
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

        // Build URL with query parameters
        let mut search_url = "https://www.google.com/search".to_string();
        let query_string = query_params.iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        
        if !query_string.is_empty() {
            search_url = format!("{}?{}", search_url, query_string);
        }

        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw",
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
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced crypto request failed: {}", e)))?;

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
            &format!("Enhanced Crypto: {} ({})", search_query, crypto_type),
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
                "BrightData enhanced crypto error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Use filters to validate content before processing
        if ResponseStrategy::should_try_next_source(&content) {
            return Err(BrightDataError::ToolError("Content quality too low".into()));
        }

        // Format the results
        let formatted_content = self.format_crypto_results(&content, query, crypto_type, data_points, page, num_results, time_filter);

        Ok(json!({
            "content": content,
            "formatted_content": formatted_content,
            "query": query,
            "search_query": search_query,
            "crypto_type": crypto_type,
            "data_points": data_points,
            "page": page,
            "num_results": num_results,
            "time_filter": time_filter,
            "safe_search": safe_search,
            "zone": zone,
            "execution_id": execution_id,
            "raw_response": content,
            "success": true,
            "api_type": "enhanced_serp"
        }))
    }

    async fn fetch_crypto_data_legacy(&self, query: &str, execution_id: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        let search_url = format!(
            "https://www.google.com/search?q={} cryptocurrency price market cap coinmarketcap",
            urlencoding::encode(query)
        );

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
            .map_err(|e| BrightDataError::ToolError(format!("Crypto data request failed: {}", e)))?;

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
                "BrightData crypto data error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Use filters to validate content before returning
        if ResponseStrategy::should_try_next_source(&content) {
            return Err(BrightDataError::ToolError("Content quality too low".into()));
        }

        Ok(json!({
            "content": content,
            "query": query,
            "execution_id": execution_id,
            "success": true,
            "api_type": "legacy"
        }))
    }

    fn build_crypto_query(&self, query: &str, crypto_type: &str, data_points: &[&str]) -> String {
        let mut search_terms = vec![query.to_string()];
        
        // Add cryptocurrency identifier
        search_terms.push("cryptocurrency".to_string());
        
        // Add crypto type if specified
        if crypto_type != "any" {
            match crypto_type {
                "bitcoin" => search_terms.push("bitcoin BTC".to_string()),
                "altcoins" => search_terms.push("altcoins ethereum".to_string()),
                "defi" => search_terms.push("DeFi decentralized finance".to_string()),
                "nft" => search_terms.push("NFT non-fungible token".to_string()),
                "stablecoins" => search_terms.push("stablecoin USDT USDC".to_string()),
                _ => {}
            }
        }
        
        // Add data points as search terms
        for data_point in data_points {
            match *data_point {
                "price" => search_terms.push("price".to_string()),
                "market_cap" => search_terms.push("market cap".to_string()),
                "volume" => search_terms.push("volume".to_string()),
                "change" => search_terms.push("change percentage".to_string()),
                "supply" => search_terms.push("supply circulating".to_string()),
                "dominance" => search_terms.push("dominance".to_string()),
                "fear_greed" => search_terms.push("fear greed index".to_string()),
                _ => {}
            }
        }
        
        // Add popular crypto data sources
        search_terms.extend_from_slice(&[
            "coinmarketcap".to_string(),
            "coingecko".to_string()
        ]);
        
        search_terms.join(" ")
    }

    fn format_crypto_results(&self, content: &str, query: &str, crypto_type: &str, data_points: &[&str], page: u32, num_results: u32, time_filter: &str) -> String {
        let mut formatted = String::new();
        
        // Add header with search parameters
        formatted.push_str(&format!("# Cryptocurrency Data: {}\n\n", query));
        formatted.push_str(&format!("**Crypto Type**: {} | **Data Points**: {:?} | **Time Filter**: {}\n", 
            crypto_type, data_points, time_filter));
        formatted.push_str(&format!("**Page**: {} | **Results**: {}\n\n", page, num_results));
        
        // Try to parse JSON response if available
        if let Ok(json_data) = serde_json::from_str::<Value>(content) {
            // If we get structured JSON, format it nicely
            if let Some(results) = json_data.get("organic_results").and_then(|r| r.as_array()) {
                formatted.push_str("## Cryptocurrency Information\n\n");
                for (i, result) in results.iter().take(num_results as usize).enumerate() {
                    let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("No title");
                    let link = result.get("link").and_then(|l| l.as_str()).unwrap_or("");
                    let snippet = result.get("snippet").and_then(|s| s.as_str()).unwrap_or("");
                    
                    formatted.push_str(&format!("### {}. {}\n", i + 1, title));
                    if !link.is_empty() {
                        formatted.push_str(&format!("**Source**: {}\n", link));
                    }
                    if !snippet.is_empty() {
                        formatted.push_str(&format!("**Details**: {}\n", snippet));
                    }
                    
                    // Highlight specific data points if found in snippet
                    self.highlight_crypto_data_points(&mut formatted, snippet, data_points);
                    formatted.push_str("\n");
                }
            } else {
                // JSON but no organic_results, return formatted JSON
                formatted.push_str("## Crypto Data\n\n");
                formatted.push_str("```json\n");
                formatted.push_str(&serde_json::to_string_pretty(&json_data).unwrap_or_else(|_| content.to_string()));
                formatted.push_str("\n```\n");
            }
        } else {
            // Plain text/markdown response
            formatted.push_str("## Cryptocurrency Information\n\n");
            formatted.push_str(content);
        }
        
        // Add pagination info
        if page > 1 || num_results < 100 {
            formatted.push_str(&format!("\n---\n*Page {} of cryptocurrency results*\n", page));
            if page > 1 {
                formatted.push_str("💡 *To get more results, use page parameter*\n");
            }
        }
        
        formatted
    }

    fn highlight_crypto_data_points(&self, formatted: &mut String, snippet: &str, data_points: &[&str]) {
        let snippet_lower = snippet.to_lowercase();
        let mut found_points = Vec::new();
        
        for data_point in data_points {
            match *data_point {
                "price" if snippet_lower.contains("price") || snippet_lower.contains("$") || snippet_lower.contains("usd") => {
                    found_points.push("💰 Price data detected");
                }
                "market_cap" if snippet_lower.contains("market cap") || snippet_lower.contains("mcap") => {
                    found_points.push("📊 Market cap information");
                }
                "volume" if snippet_lower.contains("volume") || snippet_lower.contains("traded") => {
                    found_points.push("📈 Volume data found");
                }
                "change" if snippet_lower.contains("%") || snippet_lower.contains("change") || snippet_lower.contains("gain") => {
                    found_points.push("📊 Price change information");
                }
                "supply" if snippet_lower.contains("supply") || snippet_lower.contains("circulation") => {
                    found_points.push("🪙 Supply data available");
                }
                "dominance" if snippet_lower.contains("dominance") => {
                    found_points.push("👑 Market dominance data");
                }
                "fear_greed" if snippet_lower.contains("fear") || snippet_lower.contains("greed") => {
                    found_points.push("😰 Fear & Greed index");
                }
                _ => {}
            }
        }
        
        if !found_points.is_empty() {
            formatted.push_str("**Key Data Points**: ");
            formatted.push_str(&found_points.join(" | "));
            formatted.push_str("\n");
        }
    }
}