// src/tools/scrape.rs - PATCHED: Enhanced with priority-aware filtering and token budget management
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::extras::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use async_trait::async_trait;
use serde_json::{Value, json};
use reqwest::Client;
use std::time::Duration;
use std::collections::HashMap;
use log::info;

pub struct ScrapeMarkdown;

#[async_trait]
impl Tool for ScrapeMarkdown {
    fn name(&self) -> &str {
        "scrape_website"
    }

    fn description(&self) -> &str {
        "Scrape a webpage using BrightData - supports API, Web Unlocker, and Residential Proxy"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to scrape"
                },
                "method": {
                    "type": "string",
                    "enum": ["api", "web_unlocker_proxy", "residential_proxy", "auto"],
                    "description": "Method: 'api' for REST API, 'web_unlocker_proxy' for Web Unlocker proxy, 'residential_proxy' for standard proxy, 'auto' to detect best available",
                    "default": "auto"
                },
                "format": {
                    "type": "string",
                    "enum": ["raw", "markdown", "screenshot"],
                    "description": "Output format - raw (HTML), markdown, or screenshot (Web Unlocker only)",
                    "default": "markdown"
                },
                "country": {
                    "type": "string",
                    "description": "Country code for geo-targeting (e.g., 'us', 'in', 'uk')",
                    "default": ""
                },
                "city": {
                    "type": "string",
                    "description": "City for geo-targeting (Web Unlocker only)",
                    "default": ""
                },
                "zipcode": {
                    "type": "string",
                    "description": "Zipcode for precise geo-targeting (Web Unlocker only)",
                    "default": ""
                },
                "mobile": {
                    "type": "boolean",
                    "description": "Use mobile user agent",
                    "default": false
                },
                "wait_for": {
                    "type": "string",
                    "description": "CSS selector or text to wait for (Web Unlocker only)",
                    "default": ""
                },
                "custom_headers": {
                    "type": "object",
                    "description": "Custom headers to send",
                    "additionalProperties": true,
                    "default": {}
                },
                "disable_captcha_solving": {
                    "type": "boolean",
                    "description": "Disable automatic CAPTCHA solving (Web Unlocker only)",
                    "default": false
                }
            },
            "required": ["url"]
        })
    }

    // FIXED: Remove the execute method override to use the default one with metrics logging
    // async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
    //     self.execute_internal(parameters).await
    // }

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let url = parameters
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'url' parameter".into()))?;
        
        // Validate URL is not empty and is properly formatted
        if url.trim().is_empty() {
            return Err(BrightDataError::ToolError("URL cannot be empty".into()));
        }
        
        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(BrightDataError::ToolError("URL must start with http:// or https://".into()));
        }

        let method = parameters
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let format = parameters
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("markdown");

        let country = parameters
            .get("country")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let city = parameters
            .get("city")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let zipcode = parameters
            .get("zipcode")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mobile = parameters
            .get("mobile")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let wait_for = parameters
            .get("wait_for")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let custom_headers = parameters
            .get("custom_headers")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let disable_captcha_solving = parameters
            .get("disable_captcha_solving")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(url);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(url);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            // Budget check for scrape queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 150 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", url, "scrape", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = self.generate_execution_id();
        
        // Auto-detect best available method with priority awareness
        let selected_method = if method == "auto" {
            self.detect_best_method_with_priority(query_priority)
        } else {
            method.to_string()
        };

        let result = match selected_method.as_str() {
            "api" => {
                self.scrape_with_api_interface_priority(
                    url, format, country, mobile, wait_for, &custom_headers,
                    disable_captcha_solving, query_priority, recommended_tokens, &execution_id
                ).await?
            }
            "web_unlocker_proxy" => {
                self.scrape_with_web_unlocker_proxy_priority(
                    url, format, country, city, zipcode, mobile, wait_for, &custom_headers,
                    disable_captcha_solving, query_priority, recommended_tokens, &execution_id
                ).await?
            }
            "residential_proxy" => {
                self.scrape_with_residential_proxy_priority(
                    url, country, mobile, &custom_headers, query_priority, recommended_tokens, &execution_id
                ).await?
            }
            _ => {
                return Err(BrightDataError::ToolError("Invalid method selected".into()));
            }
        };
        
        let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let service_used = result.get("service").and_then(|s| s.as_str()).unwrap_or("Unknown");

        // Create appropriate response based on whether filtering is enabled
        let tool_result = if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            ResponseStrategy::create_financial_response(
                "scrape", url, "web", service_used, content, result.clone()
            )
        } else {
            // No filtering - create standard response
            let content_text = if content.len() > 2000 { 
                format!("{}... (truncated - {} total chars)", &content[..2000], content.len())
            } else { 
                content.to_string() 
            };

            let mcp_content = vec![McpContent::text(format!(
                "🌐 **BrightData Scrape from {}**\n\nMethod: {} | Format: {} | Country: {} | Mobile: {} | Priority: {:?} | Tokens: {}\nZone/Service: {} | Execution ID: {}\n\n{}",
                url,
                selected_method.to_uppercase(),
                format,
                if country.is_empty() { "Auto" } else { country },
                mobile,
                query_priority,
                recommended_tokens,
                service_used,
                execution_id,
                content_text
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
}

impl ScrapeMarkdown {
    fn generate_execution_id(&self) -> String {
        format!("scrape_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"))
    }

    // ENHANCED: Priority-aware method detection
    fn detect_best_method_with_priority(&self, priority: crate::filters::strategy::QueryPriority) -> String {
        // For critical queries, prefer best available methods
        match priority {
            crate::filters::strategy::QueryPriority::Critical => {
                // Priority: Web Unlocker API > Web Unlocker Proxy > Residential Proxy
                if std::env::var("BRIGHTDATA_API_TOKEN").is_ok() || std::env::var("API_TOKEN").is_ok() {
                    return "api".to_string();
                }
            }
            _ => {
                // For lower priority, use simpler methods first
            }
        }
        
        if std::env::var("BRIGHTDATA_API_TOKEN").is_ok() || std::env::var("API_TOKEN").is_ok() {
            return "api".to_string();
        }
        
        if std::env::var("BRIGHTDATA_CUSTOMER_ID").is_ok() && 
           std::env::var("BRIGHTDATA_ZONE_PASSWORD").is_ok() {
            return "web_unlocker_proxy".to_string();
        }
        
        if std::env::var("BRIGHTDATA_PROXY_USERNAME").is_ok() && 
           std::env::var("BRIGHTDATA_PROXY_PASSWORD").is_ok() {
            return "residential_proxy".to_string();
        }
        
        // Fallback to API if no credentials detected
        "api".to_string()
    }

    // ENHANCED: Priority-aware Web Unlocker API Interface
    async fn scrape_with_api_interface_priority(
        &self, 
        url: &str, 
        format: &str,
        country: &str,
        mobile: bool,
        wait_for: &str,
        custom_headers: &serde_json::Map<String, Value>,
        disable_captcha_solving: bool,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN for API method".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        info!("🌐 Priority {} API Interface: Web Unlocker scraping {} using zone: {} (execution: {})", 
              format!("{:?}", priority), url, zone, execution_id);

        let mut payload = json!({
            "zone": zone,
            "url": url,
            "format": "raw"
        });

        // Data format
        match format {
            "markdown" => { payload["data_format"] = json!("markdown"); }
            "screenshot" => { payload["data_format"] = json!("screenshot"); }
            _ => {}
        }

        if !country.is_empty() { payload["country"] = json!(country); }
        if mobile { payload["mobile"] = json!(true); }
        if disable_captcha_solving { payload["disable_captcha_solving"] = json!(true); }

        if !wait_for.is_empty() {
            if wait_for.starts_with('.') || wait_for.starts_with('#') {
                payload["expect"] = json!({"element": wait_for});
            } else {
                payload["expect"] = json!({"text": wait_for});
            }
        }

        if !custom_headers.is_empty() {
            payload["headers"] = json!(custom_headers);
        }

        // Add priority processing hints
        // (No undocumented fields should be added to the payload)
        // if std::env::var("TRUNCATE_FILTER")
        //     .map(|v| v.to_lowercase() == "true")
        //     .unwrap_or(false) {
        //     // Only add token_budget if it's a documented/accepted field
        //     payload["token_budget"] = json!(token_budget);
        // }

        let client = Client::builder().timeout(Duration::from_secs(120)).build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("API request failed: {}", e)))?;

        self.handle_response_with_priority(response, execution_id, &zone, url, format, payload, "Web Unlocker API", priority, token_budget).await
    }

    // ENHANCED: Priority-aware Web Unlocker Proxy Interface
    async fn scrape_with_web_unlocker_proxy_priority(
        &self, 
        url: &str, 
        format: &str,
        country: &str,
        city: &str,
        zipcode: &str,
        mobile: bool,
        wait_for: &str,
        custom_headers: &serde_json::Map<String, Value>,
        disable_captcha_solving: bool,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let customer_id = std::env::var("BRIGHTDATA_CUSTOMER_ID")
            .or_else(|_| std::env::var("CUSTOMER_ID"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_CUSTOMER_ID for Web Unlocker proxy".into()))?;

        let zone = std::env::var("WEB_UNLOCKER_ZONE")
            .unwrap_or_else(|_| "default".to_string());

        let password = std::env::var("BRIGHTDATA_ZONE_PASSWORD")
            .or_else(|_| std::env::var("ZONE_PASSWORD"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_ZONE_PASSWORD for Web Unlocker proxy".into()))?;

        info!("🌐 Priority {} Web Unlocker Proxy: scraping {} using zone: {} (execution: {})", 
              format!("{:?}", priority), url, zone, execution_id);

        let proxy_username = format!("brd-customer-{}-zone-{}", customer_id, zone);
        let proxy_url = format!("http://{}:{}@brd.superproxy.io:33335", proxy_username, password);
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|e| BrightDataError::ToolError(format!("Invalid Web Unlocker proxy URL: {}", e)))?;

        let mut headers = reqwest::header::HeaderMap::new();

        match format {
            "markdown" => { headers.insert("x-unblock-data-format", "markdown".parse().unwrap()); }
            "screenshot" => { headers.insert("x-unblock-data-format", "screenshot".parse().unwrap()); }
            _ => {}
        }

        if !country.is_empty() { headers.insert("x-unblock-country", country.parse().unwrap()); }
        if !city.is_empty() { headers.insert("x-unblock-city", city.parse().unwrap()); }
        if !zipcode.is_empty() { headers.insert("x-unblock-zipcode", zipcode.parse().unwrap()); }
        if mobile { headers.insert("x-unblock-mobile", "true".parse().unwrap()); }
        if disable_captcha_solving { headers.insert("x-unblock-disable-captcha", "true".parse().unwrap()); }

        // Add priority header
        headers.insert("x-unblock-priority", format!("{:?}", priority).parse().unwrap());

        if !wait_for.is_empty() {
            let expect_value = if wait_for.starts_with('.') || wait_for.starts_with('#') {
                format!(r#"{{"element":"{}"}}"#, wait_for)
            } else {
                format!(r#"{{"text":"{}"}}"#, wait_for)
            };
            headers.insert("x-unblock-expect", expect_value.parse().unwrap());
        }

        for (key, value) in custom_headers.iter() {
            if let Some(value_str) = value.as_str() {
                let header_name = format!("x-unblock-header-{}", key.to_lowercase());
                if let (Ok(header_key), Ok(header_value)) = (
                    reqwest::header::HeaderName::from_bytes(header_name.as_bytes()),
                    value_str.parse::<reqwest::header::HeaderValue>()
                ) {
                    headers.insert(header_key, header_value);
                }
            }
        }

        let client = Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(120))
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client.get(url).headers(headers).send().await
            .map_err(|e| BrightDataError::ToolError(format!("Web Unlocker proxy request failed: {}", e)))?;

        let payload = json!({
            "method": "web_unlocker_proxy",
            "url": url,
            "zone": zone,
            "format": format,
            "country": country,
            "city": city,
            "zipcode": zipcode,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget
        });

        self.handle_response_with_priority(response, execution_id, &zone, url, format, payload, "Web Unlocker Proxy", priority, token_budget).await
    }

    // ENHANCED: Priority-aware Residential/Datacenter Proxy Interface
    async fn scrape_with_residential_proxy_priority(
        &self, 
        url: &str, 
        country: &str,
        mobile: bool,
        custom_headers: &serde_json::Map<String, Value>,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let proxy_username = std::env::var("BRIGHTDATA_PROXY_USERNAME")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_PROXY_USERNAME for residential proxy".into()))?;

        let proxy_password = std::env::var("BRIGHTDATA_PROXY_PASSWORD")
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_PROXY_PASSWORD for residential proxy".into()))?;

        let proxy_host = std::env::var("BRIGHTDATA_PROXY_HOST")
            .unwrap_or_else(|_| "zproxy.lum-superproxy.io".to_string());

        let proxy_port = std::env::var("BRIGHTDATA_PROXY_PORT")
            .unwrap_or_else(|_| "22225".to_string());

        info!("🌐 Priority {} Residential Proxy: scraping {} via {}:{} (execution: {})", 
              format!("{:?}", priority), url, proxy_host, proxy_port, execution_id);

        // Build enhanced username with targeting and priority
        let mut enhanced_username = proxy_username.clone();
        if !country.is_empty() {
            enhanced_username = format!("{}-country-{}", enhanced_username, country);
        }
        if mobile {
            enhanced_username = format!("{}-session-mobile", enhanced_username);
        }
        // Add priority session info for critical queries
        if matches!(priority, crate::filters::strategy::QueryPriority::Critical) {
            enhanced_username = format!("{}-session-critical", enhanced_username);
        }

        let proxy_url = format!("http://{}:{}@{}:{}", enhanced_username, proxy_password, proxy_host, proxy_port);
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|e| BrightDataError::ToolError(format!("Invalid residential proxy URL: {}", e)))?;

        let mut headers = reqwest::header::HeaderMap::new();
        
        // Add custom headers
        for (key, value) in custom_headers.iter() {
            if let Some(value_str) = value.as_str() {
                if let (Ok(header_key), Ok(header_value)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    value_str.parse::<reqwest::header::HeaderValue>()
                ) {
                    headers.insert(header_key, header_value);
                }
            }
        }

        // Set appropriate user agent based on priority
        if mobile {
            headers.insert("user-agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 14_7_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.2 Mobile/15E148 Safari/604.1".parse().unwrap());
        } else if matches!(priority, crate::filters::strategy::QueryPriority::Critical) {
            headers.insert("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".parse().unwrap());
        }

        let client = Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(120))
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client.get(url).headers(headers).send().await
            .map_err(|e| BrightDataError::ToolError(format!("Residential proxy request failed: {}", e)))?;

        let payload = json!({
            "method": "residential_proxy",
            "url": url,
            "proxy_host": proxy_host,
            "proxy_port": proxy_port,
            "country": country,
            "mobile": mobile,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget
        });

        self.handle_response_with_priority(response, execution_id, "residential_proxy", url, "raw", payload, "Residential Proxy", priority, token_budget).await
    }

    // ENHANCED: Priority-aware response handling with token management
    async fn handle_response_with_priority(
        &self,
        response: reqwest::Response,
        execution_id: &str,
        service_name: &str,
        url: &str,
        format: &str,
        payload: Value,
        service_type: &str,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
    ) -> Result<Value, BrightDataError> {
        let status = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Log BrightData request
        if let Err(e) = JSON_LOGGER.log_brightdata_request(
            execution_id,
            service_name,
            url,
            payload.clone(),
            status,
            response_headers,
            format
        ).await {
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData {} error {}: {}",
                service_type, status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Check if response is empty or contains error
        if raw_content.trim().is_empty() {
            return Err(BrightDataError::ToolError(format!(
                "{} returned empty response", service_type
            )));
        }

        // Parse the response to check for BrightData API errors
        let content_to_process = if let Ok(response_json) = serde_json::from_str::<Value>(&raw_content) {
            if let Some(error) = response_json.get("error") {
                return Err(BrightDataError::ToolError(format!(
                    "{} returned API error: {}", service_type, error
                )));
            }
            
            // Check if response contains the expected data
            if let Some(data) = response_json.get("data") {
                if let Some(content) = data.as_str() {
                    content.to_string()
                } else {
                    raw_content
                }
            } else {
                raw_content
            }
        } else {
            raw_content
        };

        // Apply filters conditionally based on environment variable with priority awareness
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&content_to_process) {
                return Err(BrightDataError::ToolError(format!("{} returned error page", service_type)));
            } else if ResponseStrategy::should_try_next_source(&content_to_process) {
                return Err(BrightDataError::ToolError("Content quality too low".into()));
            }
        }

        let processed_content = self.post_process_content_with_priority(&content_to_process, format, priority, token_budget);

        Ok(json!({
            "content": processed_content,
            "raw_content": content_to_process,
            "service": service_type,
            "service_name": service_name,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget,
            "execution_id": execution_id,
            "success": true
        }))
    }

    // ENHANCED: Priority-aware post-processing
    fn post_process_content_with_priority(&self, content: &str, format: &str, priority: crate::filters::strategy::QueryPriority, token_budget: usize) -> String {
        let mut processed = match format {
            "screenshot" => {
                format!("Screenshot captured successfully. Base64 data length: {} characters", content.len())
            }
            "markdown" => content.to_string(),
            _ => content.to_string()
        };

        // Apply filters conditionally with priority awareness
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) && format != "screenshot" {
            
            // Use token budget aware filtering
            let max_tokens = token_budget / 2; // Reserve tokens for formatting
            processed = ResponseFilter::extract_high_value_financial_data(&processed, max_tokens);
            
            // Apply priority-specific truncation
            let max_length = match priority {
                crate::filters::strategy::QueryPriority::Critical => 20000,
                crate::filters::strategy::QueryPriority::High => 15000,
                crate::filters::strategy::QueryPriority::Medium => 10000,
                crate::filters::strategy::QueryPriority::Low => 5000,
            };
            
            if processed.len() > max_length {
                processed = ResponseFilter::smart_truncate_preserving_financial_data(&processed, max_length);
            }
        }

        processed
    }
}