// src/tools/mutual_fund.rs - Enhanced with BrightData SERP API parameters
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use std::collections::HashMap;

pub struct MutualFundDataTool;

#[async_trait]
impl Tool for MutualFundDataTool {
    fn name(&self) -> &str {
        "get_mutual_fund_data"
    }

    fn description(&self) -> &str {
        "Get mutual fund data with enhanced search parameters including NAV, performance, portfolio composition, and fund comparisons"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Fund name, fund symbol, fund category (equity funds, debt funds), or fund comparison query"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian",
                    "description": "Market region"
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
                    "minimum": 10,
                    "maximum": 100,
                    "default": 20
                },
                "time_filter": {
                    "type": "string",
                    "enum": ["any", "day", "week", "month", "year"],
                    "description": "Time-based filter for fund performance data",
                    "default": "any"
                },
                "fund_type": {
                    "type": "string",
                    "enum": ["any", "equity", "debt", "hybrid", "index", "etf"],
                    "description": "Type of mutual fund to search for",
                    "default": "any"
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
        
        let market = parameters
            .get("market")
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
            .unwrap_or("any");

        let fund_type = parameters
            .get("fund_type")
            .and_then(|v| v.as_str())
            .unwrap_or("any");

        let use_serp_api = parameters
            .get("use_serp_api")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let execution_id = format!("mutual_fund_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        let result = if use_serp_api {
            self.fetch_mutual_fund_data_enhanced(
                query, market, page, num_results, time_filter, fund_type, &execution_id
            ).await?
        } else {
            self.fetch_mutual_fund_data_legacy(query, market, &execution_id).await?
        };

        let content_text = result.get("content").and_then(|c| c.as_str()).unwrap_or("No mutual fund data found");
        
        let mcp_content = if use_serp_api {
            vec![McpContent::text(format!(
                "🏦 **Enhanced Mutual Fund Data for {}**\n\nMarket: {} | Fund Type: {} | Page: {} | Results: {} | Time Filter: {}\nExecution ID: {}\n\n{}",
                query,
                market.to_uppercase(),
                fund_type,
                page,
                num_results,
                time_filter,
                execution_id,
                content_text
            ))]
        } else {
            vec![McpContent::text(format!(
                "🏦 **Mutual Fund Data for {}**\n\nMarket: {}\nExecution ID: {}\n\n{}",
                query,
                market.to_uppercase(),
                execution_id,
                content_text
            ))]
        };

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl MutualFundDataTool {
    async fn fetch_mutual_fund_data_enhanced(
        &self, 
        query: &str, 
        market: &str, 
        page: u32,
        num_results: u32,
        time_filter: &str,
        fund_type: &str,
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
        let search_query = self.build_fund_query(query, market, fund_type);
        
        // Build enhanced query parameters
        let mut query_params = HashMap::new();
        query_params.insert("q".to_string(), search_query.clone());
        
        // Pagination
        if page > 1 {
            let start = (page - 1) * num_results;
            query_params.insert("start".to_string(), start.to_string());
        }
        query_params.insert("num".to_string(), num_results.to_string());
        
        // Localization based on market
        let (country, language) = self.get_market_settings(market);
        query_params.insert("gl".to_string(), country.to_string());
        query_params.insert("hl".to_string(), language.to_string());
        
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
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced mutual fund request failed: {}", e)))?;

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
            &format!("Enhanced Mutual Fund: {} ({})", search_query, market),
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
                "BrightData enhanced mutual fund error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Format the results
        let formatted_content = self.format_fund_results(&content, query, market, fund_type, page, num_results);

        Ok(json!({
            "content": formatted_content,
            "query": query,
            "search_query": search_query,
            "market": market,
            "fund_type": fund_type,
            "page": page,
            "num_results": num_results,
            "time_filter": time_filter,
            "zone": zone,
            "execution_id": execution_id,
            "raw_response": content,
            "success": true,
            "api_type": "enhanced_serp"
        }))
    }

    async fn fetch_mutual_fund_data_legacy(&self, query: &str, market: &str, execution_id: &str) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        let search_url = match market {
            "indian" => format!("https://www.google.com/search?q={} mutual fund NAV performance AMFI india SIP", urlencoding::encode(query)),
            "us" => format!("https://www.google.com/search?q={} mutual fund performance expense ratio morningstar", urlencoding::encode(query)),
            "global" => format!("https://www.google.com/search?q={} mutual fund global performance rating", urlencoding::encode(query)),
            _ => format!("https://www.google.com/search?q={} mutual fund NAV performance", urlencoding::encode(query))
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
            .map_err(|e| BrightDataError::ToolError(format!("Mutual fund data request failed: {}", e)))?;

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
                "BrightData mutual fund data error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        Ok(json!({
            "content": content,
            "query": query,
            "market": market,
            "execution_id": execution_id,
            "success": true,
            "api_type": "legacy"
        }))
    }

    fn build_fund_query(&self, query: &str, market: &str, fund_type: &str) -> String {
        let mut search_terms = vec![query.to_string()];
        
        // Add fund type if specified
        if fund_type != "any" {
            search_terms.push(format!("{} fund", fund_type));
        } else {
            search_terms.push("mutual fund".to_string());
        }
        
        // Add market-specific terms
        match market {
            "indian" => {
                search_terms.extend_from_slice(&[
                    "NAV".to_string(),
                    "performance".to_string(),
                    "AMFI".to_string(),
                    "india".to_string(),
                    "SIP".to_string()
                ]);
            }
            "us" => {
                search_terms.extend_from_slice(&[
                    "performance".to_string(),
                    "expense ratio".to_string(),
                    "morningstar".to_string()
                ]);
            }
            "global" => {
                search_terms.extend_from_slice(&[
                    "global".to_string(),
                    "performance".to_string(),
                    "rating".to_string()
                ]);
            }
            _ => {
                search_terms.extend_from_slice(&[
                    "NAV".to_string(),
                    "performance".to_string()
                ]);
            }
        }
        
        search_terms.join(" ")
    }

    fn get_market_settings(&self, market: &str) -> (&str, &str) {
        match market {
            "indian" => ("in", "en"),
            "us" => ("us", "en"),
            "global" => ("", "en"),
            _ => ("us", "en")
        }
    }

    fn format_fund_results(&self, content: &str, query: &str, market: &str, fund_type: &str, page: u32, num_results: u32) -> String {
        let mut formatted = String::new();
        
        // Add header with search parameters
        formatted.push_str(&format!("# Mutual Fund Data: {}\n\n", query));
        formatted.push_str(&format!("**Market**: {} | **Fund Type**: {} | **Page**: {} | **Results**: {}\n\n", 
            market.to_uppercase(), fund_type, page, num_results));
        
        // Try to parse JSON response if available
        if let Ok(json_data) = serde_json::from_str::<Value>(content) {
            // If we get structured JSON, format it nicely
            if let Some(results) = json_data.get("organic_results").and_then(|r| r.as_array()) {
                formatted.push_str("## Fund Information\n\n");
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
                    formatted.push_str("\n");
                }
            } else {
                // JSON but no organic_results, return formatted JSON
                formatted.push_str("## Fund Data\n\n");
                formatted.push_str("```json\n");
                formatted.push_str(&serde_json::to_string_pretty(&json_data).unwrap_or_else(|_| content.to_string()));
                formatted.push_str("\n```\n");
            }
        } else {
            // Plain text/markdown response
            formatted.push_str("## Fund Information\n\n");
            formatted.push_str(content);
        }
        
        // Add pagination info
        if page > 1 || num_results < 100 {
            formatted.push_str(&format!("\n---\n*Page {} of mutual fund results*\n", page));
            if page > 1 {
                formatted.push_str("💡 *To get more results, use page parameter*\n");
            }
        }
        
        formatted
    }
}