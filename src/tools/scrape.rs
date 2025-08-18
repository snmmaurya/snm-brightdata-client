// src/tools/scrape.rs - ENHANCED VERSION WITH REDIS CACHE SUPPORT
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::extras::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy};
use crate::services::cache::scrape_cache::get_scrape_cache;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::time::Duration;
use std::collections::HashMap;
use log::{info, warn, error};

pub struct Scraper;

#[async_trait]
impl Tool for Scraper {
    fn name(&self) -> &str {
        "scrape_website"
    }

    fn description(&self) -> &str {
        "Scrape a webpage using BrightData with intelligent caching and priority-based processing. Supports Web Unlocker with Redis cache for improved performance."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to scrape"
                },
                "session_id": {
                    "type": "string",
                    "description": "Session ID for caching and conversation context tracking"
                },
                "data_type": {
                    "type": "string",
                    "enum": ["auto", "article", "product", "news", "contact", "general"],
                    "default": "auto",
                    "description": "Type of content to focus on during extraction"
                },
                "extraction_format": {
                    "type": "string",
                    "enum": ["structured", "markdown", "text", "json"],
                    "default": "structured",
                    "description": "Format for extracted content"
                },
                "clean_content": {
                    "type": "boolean",
                    "default": true,
                    "description": "Remove noise and focus on main content"
                },
                "schema": {
                    "type": "object",
                    "description": "Optional extraction schema for structured data"
                },
                "force_refresh": {
                    "type": "boolean",
                    "default": false,
                    "description": "Force fresh scraping, bypassing cache"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        self.execute_internal(parameters).await
    }

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let url = parameters
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'url' parameter".into()))?;

        let session_id = parameters
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'user_id' parameter".into()))?;

        let data_type = parameters
            .get("data_type")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let extraction_format = parameters
            .get("extraction_format")
            .and_then(|v| v.as_str())
            .unwrap_or("structured");

        let clean_content = parameters
            .get("clean_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let force_refresh = parameters
            .get("force_refresh")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let schema = parameters.get("schema").cloned();

        let execution_id = self.generate_execution_id();
        
        info!("🌐 Scraping request: '{}' (session: {}, type: {}, format: {})", 
              url, session_id, data_type, extraction_format);
        
        // 🎯 CACHE CHECK - Check Redis cache first (unless force_refresh=true)
        if !force_refresh {
            match self.check_cache_first(url, session_id).await {
                Ok(Some(cached_result)) => {
                    info!("🚀 Cache HIT: Returning cached data for {} in session {}", url, session_id);
                    
                    // Create tool result from cached data
                    let content = cached_result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    let source_used = "Cache";
                    let method_used = "Redis Cache";
                    
                    let formatted_response = self.create_formatted_scrape_response(
                        url, data_type, extraction_format, content, &execution_id
                    );
                    
                    let tool_result = ToolResult::success_with_raw(
                        vec![McpContent::text(formatted_response)], 
                        cached_result
                    );
                    
                    // Apply filtering only if DEDUCT_DATA=true
                    if self.is_data_reduction_enabled() {
                        return Ok(ResponseStrategy::apply_size_limits(tool_result));
                    } else {
                        return Ok(tool_result);
                    }
                }
                Ok(None) => {
                    info!("💾 Cache MISS: Fetching fresh data for {} in session {}", url, session_id);
                }
                Err(e) => {
                    warn!("🚨 Cache error (continuing with fresh fetch): {}", e);
                }
            }
        } else {
            info!("🔄 Force refresh requested, bypassing cache for {}", url);
        }

        // 🌐 FRESH FETCH - Cache miss or force refresh, fetch from BrightData
        match self.scrape_with_brightdata(url, data_type, extraction_format, clean_content, schema, &execution_id).await {
            Ok(result) => {
                // 🗄️ CACHE STORE - Store successful result in cache
                if let Err(e) = self.store_in_cache(url, session_id, &result).await {
                    warn!("Failed to store result in cache: {}", e);
                }
                
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create formatted response based on DEDUCT_DATA setting
                let formatted_response = self.create_formatted_scrape_response(
                    url, data_type, extraction_format, content, &execution_id
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
                warn!("BrightData error for URL '{}', returning empty data for retry", url);
                let empty_response = json!({
                    "url": url,
                    "data_type": data_type,
                    "status": "no_data",
                    "reason": "brightdata_error",
                    "execution_id": execution_id,
                    "session_id": session_id
                });
                
                Ok(ToolResult::success_with_raw(
                    vec![McpContent::text("📊 **No Data Available**\n\nPlease try again with a different URL or check if the website is accessible.".to_string())],
                    empty_response
                ))
            }
        }
    }
}

impl Scraper {
    /// ENHANCED: Check if data reduction is enabled via DEDUCT_DATA environment variable only
    fn is_data_reduction_enabled(&self) -> bool {
        std::env::var("DEDUCT_DATA")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// ENHANCED: Create formatted response with DEDUCT_DATA control
    fn create_formatted_scrape_response(
        &self,
        url: &str,
        data_type: &str,
        extraction_format: &str,
        content: &str,
        execution_id: &str
    ) -> String {
        // If DEDUCT_DATA=false, return full content with basic formatting
        if !self.is_data_reduction_enabled() {
            return format!(
                "📊 **Data Extraction from: {}**\n\n## Full Content\n{}\n\n*Data Type: {} | Format: {} • Execution: {}*",
                url, 
                content,
                data_type, 
                extraction_format,
                execution_id
            );
        }

        // TODO: Add filtered data extraction logic when DEDUCT_DATA=true
        // For now, return full content formatted
        format!(
            "📊 **Data Extraction from: {}**\n\n## Content (TODO: Add Filtering)\n{}\n\n*Data Type: {} | Format: {} • Execution: {}*",
            url, 
            content,
            data_type, 
            extraction_format,
            execution_id
        )
    }

    fn generate_execution_id(&self) -> String {
        format!("scrape_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"))
    }

    // 🎯 ADDED: Check Redis cache first
    async fn check_cache_first(
        &self,
        url: &str,
        session_id: &str,
    ) -> Result<Option<Value>, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.get_cached_scrape_data(session_id, url).await
    }

    // 🗄️ ADDED: Store successful result in Redis cache
    async fn store_in_cache(
        &self,
        url: &str,
        session_id: &str,
        data: &Value,
    ) -> Result<(), BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.cache_scrape_data(session_id, url, data.clone()).await
    }

    // 🔍 ADDED: Get all cached URLs for session (useful for finding related content)
    pub async fn get_session_cached_urls(&self, session_id: &str) -> Result<Vec<String>, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.get_session_scrape_urls(session_id).await
    }

    // 🔍 ADDED: Get cached URLs by domain
    pub async fn get_cached_urls_by_domain(
        &self,
        session_id: &str,
        domain: &str,
    ) -> Result<Vec<String>, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.get_cached_urls_by_domain(session_id, domain).await
    }

    // 🗑️ ADDED: Clear cache for specific URL
    pub async fn clear_url_cache(
        &self,
        url: &str,
        session_id: &str,
    ) -> Result<(), BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.clear_scrape_url_cache(session_id, url).await
    }

    // 🗑️ ADDED: Clear entire session cache
    pub async fn clear_session_cache(&self, session_id: &str) -> Result<u32, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.clear_session_scrape_cache(session_id).await
    }

    // 📊 ADDED: Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<Value, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.get_scrape_cache_stats().await
    }

    // 📊 ADDED: Get cache summary for session
    pub async fn get_cache_summary(&self, session_id: &str) -> Result<Value, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.get_cache_summary(session_id).await
    }

    // 🏥 ADDED: Enhanced connectivity test including cache
    pub async fn test_connectivity_with_cache(&self) -> Result<String, BrightDataError> {
        let mut results = Vec::new();
        
        // Test cache connectivity
        info!("🧪 Testing Redis Cache...");
        match get_scrape_cache().await {
            Ok(cache_service) => {
                match cache_service.health_check().await {
                    Ok(_) => results.push("✅ Redis Cache: SUCCESS".to_string()),
                    Err(e) => results.push(format!("❌ Redis Cache: FAILED - {}", e)),
                }
            }
            Err(e) => results.push(format!("❌ Redis Cache: FAILED - {}", e)),
        }
        
        // Test existing connectivity
        let api_test = self.test_connectivity().await?;
        results.push(api_test);
        
        Ok(format!("🔍 Enhanced Connectivity Test Results:\n{}", results.join("\n")))
    }

    /// ENHANCED: Extract data with BrightData using only WEB_UNLOCKER_ZONE
    async fn scrape_with_brightdata(
        &self,
        url: &str,
        data_type: &str,
        extraction_format: &str,
        clean_content: bool,
        schema: Option<Value>,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        // Always use WEB_UNLOCKER_ZONE
        let zone = env::var("WEB_UNLOCKER_ZONE").unwrap_or_else(|_| "web_unlocker".to_string());

        info!("📊 Extracting from {} using WEB_UNLOCKER_ZONE: {} (execution: {})", 
              url, zone, execution_id);

        // Build payload with mandatory markdown format
        let mut payload = json!({
            "url": url,
            "zone": zone,
            "format": "json",
            "data_format": "markdown"  // MANDATORY: Always use markdown format
        });

        // Add optional schema if provided
        if let Some(schema_obj) = schema {
            payload["extraction_schema"] = schema_obj;
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
            .map_err(|e| BrightDataError::ToolError(format!("BrightData extraction request failed: {}", e)))?;

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
            url,
            payload.clone(),
            status,
            response_headers,
            extraction_format
        ).await {
            log::warn!("Failed to log BrightData extraction request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData extraction error {}: {}",
                status, error_text
            )));
        }

        let raw_content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        // Print what came from BrightData
        println!("################################################################################################################");
        println!("BRIGHTDATA RAW RESPONSE FROM: {}", url);
        println!("ZONE: {}", zone);
        println!("EXECUTION: {}", execution_id);
        println!("DATA TYPE: {}", data_type);
        println!("EXTRACTION FORMAT: {}", extraction_format);
        println!("CONTENT LENGTH: {} bytes", raw_content.len());
        println!("################################################################################################################");
        println!("{}", raw_content);
        println!("################################################################################################################");
        println!("END OF BRIGHTDATA RESPONSE");
        println!("################################################################################################################");

        // Apply filters only if DEDUCT_DATA=true
        if self.is_data_reduction_enabled() {
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError("Extraction returned error page".into()));
            } else if ResponseStrategy::should_try_next_source(&raw_content) {
                return Err(BrightDataError::ToolError("Content quality too low".into()));
            }
        }

        // Print what will be sent to Anthropic
        println!("--------------------------------------------------------------------------");
        println!("SENDING TO ANTHROPIC FROM EXTRACT TOOL:");
        println!("URL: {}", url);
        println!("DATA TYPE: {}", data_type);
        println!("EXTRACTION FORMAT: {}", extraction_format);
        println!("DATA REDUCTION ENABLED: {}", self.is_data_reduction_enabled());
        println!("CONTENT LENGTH: {} bytes", raw_content.len());
        println!("--------------------------------------------------------------------------");
        println!("{}", raw_content);
        println!("--------------------------------------------------------------------------");
        println!("END OF CONTENT SENT TO ANTHROPIC");
        println!("--------------------------------------------------------------------------");

        // Return enhanced result with additional metadata
        Ok(json!({
            "content": raw_content,
            "metadata": {
                "url": url,
                "zone": zone,
                "execution_id": execution_id,
                "data_type": data_type,
                "extraction_format": extraction_format,
                "clean_content": clean_content,
                "data_format": "markdown",
                "data_reduction_enabled": self.is_data_reduction_enabled(),
                "status_code": status,
                "content_size_bytes": raw_content.len(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "payload_used": payload
            },
            "success": true
        }))
    }

    /// Test BrightData connectivity
    pub async fn test_connectivity(&self) -> Result<String, BrightDataError> {
        let test_url = "https://httpbin.org/json";
        let mut results = Vec::new();
        
        // Test BrightData API
        info!("🧪 Testing BrightData Web Unlocker...");
        match self.scrape_with_brightdata(
            test_url, "auto", "structured", true, None, "connectivity_test"
        ).await {
            Ok(_) => {
                results.push("✅ BrightData Web Unlocker: SUCCESS".to_string());
            }
            Err(e) => {
                results.push(format!("❌ BrightData Web Unlocker: FAILED - {}", e));
            }
        }
        
        Ok(format!("🔍 Connectivity Test Results:\n{}", results.join("\n")))
    }

    /// Check if URL is cached
    pub async fn is_url_cached(&self, session_id: &str, url: &str) -> Result<bool, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.is_url_cached(session_id, url).await
    }

    /// Batch cache multiple URLs (useful for bulk operations)
    pub async fn batch_cache_urls(
        &self,
        session_id: &str,
        url_data: Vec<(String, Value)>, // Vec<(url, data)>
    ) -> Result<Vec<String>, BrightDataError> {
        let cache_service = get_scrape_cache().await?;
        cache_service.batch_cache_scrape_data(session_id, url_data).await
    }
}