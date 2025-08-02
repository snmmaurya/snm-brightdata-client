// src/tools/extract.rs - PATCHED: Enhanced with priority-aware filtering and token budget management
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

pub struct Extractor;

#[async_trait]
impl Tool for Extractor {
    fn name(&self) -> &str {
        "extract_data"
    }

    fn description(&self) -> &str {
        "Extract structured data from a webpage using BrightData with smart zone selection and data processing"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to extract data from"
                },
                "data_type": {
                    "type": "string",
                    "enum": ["auto", "financial", "ecommerce", "social", "news", "general"],
                    "default": "auto",
                    "description": "Type of data to extract for optimized processing"
                },
                "extraction_format": {
                    "type": "string",
                    "enum": ["markdown", "json", "structured", "raw"],
                    "default": "structured",
                    "description": "Format for extracted data"
                },
                "schema": {
                    "type": "object",
                    "description": "Optional schema to guide extraction",
                    "additionalProperties": true
                },
                "enable_js": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable JavaScript rendering for dynamic content"
                },
                "extract_images": {
                    "type": "boolean",
                    "default": false,
                    "description": "Extract image URLs and metadata"
                },
                "extract_links": {
                    "type": "boolean",
                    "default": false,
                    "description": "Extract internal and external links"
                },
                "clean_content": {
                    "type": "boolean",
                    "default": true,
                    "description": "Remove navigation, ads, and boilerplate content"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let url = parameters
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'url' parameter".into()))?;

        let data_type = parameters
            .get("data_type")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        let extraction_format = parameters
            .get("extraction_format")
            .and_then(|v| v.as_str())
            .unwrap_or("structured");

        let schema = parameters.get("schema").cloned();

        let enable_js = parameters
            .get("enable_js")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let extract_images = parameters
            .get("extract_images")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let extract_links = parameters
            .get("extract_links")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let clean_content = parameters
            .get("clean_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(url);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(url);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", url);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", url, "extraction", "validation", json!({}), response_type));
            }

            // Budget check for extraction queries
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            if remaining_tokens < 150 && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", url, "extraction", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = self.generate_execution_id();
        
        match self.extract_with_brightdata_enhanced_priority(
            url, data_type, extraction_format, schema, enable_js, 
            extract_images, extract_links, clean_content, query_priority, recommended_tokens, &execution_id
        ).await {
            Ok(result) => {
                let content = result.get("processed_content").and_then(|c| c.as_str())
                    .or_else(|| result.get("content").and_then(|c| c.as_str()))
                    .unwrap_or("");
                
                // Create appropriate response based on whether filtering is enabled
                let tool_result = if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    ResponseStrategy::create_financial_response(
                        "extraction", url, "web", "BrightData", content, result.clone()
                    )
                } else {
                    // No filtering - create standard response
                    let mcp_content = vec![McpContent::text(format!(
                        "📊 **Data Extraction from: {}**\n\nData Type: {} | Format: {} | JS Enabled: {} | Priority: {:?} | Tokens: {}\nExecution ID: {}\n\n{}",
                        url, data_type, extraction_format, enable_js, query_priority, recommended_tokens, execution_id, content
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
            Err(e) => {
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    Ok(ResponseStrategy::create_error_response(url, &e.to_string()))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl Extractor {
    fn generate_execution_id(&self) -> String {
        format!("extract_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"))
    }

    // ENHANCED: Priority-aware extraction with token budget management
    async fn extract_with_brightdata_enhanced_priority(
        &self,
        url: &str,
        data_type: &str,
        extraction_format: &str,
        schema: Option<Value>,
        enable_js: bool,
        extract_images: bool,
        extract_links: bool,
        clean_content: bool,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
        execution_id: &str,
    ) -> Result<Value, BrightDataError> {
        let api_token = env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        // Smart zone selection based on URL and data type with priority awareness
        let zone = self.select_optimal_zone_with_priority(url, data_type, priority);

        info!("📊 Priority {} enhanced extraction from {} using zone: {} (execution: {})", 
              format!("{:?}", priority), url, zone, execution_id);

        // Build enhanced payload with priority awareness
        let mut payload = json!({
            "url": url,
            "zone": zone,
            "format": "json",
            "render": enable_js,
            "data_format": extraction_format
        });

        // Add extraction-specific parameters
        if let Some(schema_obj) = schema {
            payload["extraction_schema"] = schema_obj;
        }

        // Configure content processing with priority awareness
        let mut processing_options = json!({
            "clean_content": clean_content,
            "extract_images": extract_images && !matches!(priority, crate::filters::strategy::QueryPriority::Low), // Skip images for low priority
            "extract_links": extract_links && !matches!(priority, crate::filters::strategy::QueryPriority::Low), // Skip links for low priority
            "data_type": data_type,
            "priority": format!("{:?}", priority),
            "token_budget": token_budget
        });

        // Data type specific optimizations with priority filtering
        match data_type {
            "financial" => {
                processing_options["extract_tables"] = json!(true);
                processing_options["extract_numbers"] = json!(true);
                processing_options["focus_selectors"] = json!([
                    ".price", ".financial-data", ".stock-price", ".market-data",
                    "table[class*='financial']", ".earnings", ".revenue"
                ]);
            }
            "ecommerce" => {
                if matches!(priority, crate::filters::strategy::QueryPriority::Critical | crate::filters::strategy::QueryPriority::High) {
                    processing_options["extract_prices"] = json!(true);
                    processing_options["extract_reviews"] = json!(true);
                }
                processing_options["focus_selectors"] = json!([
                    ".product", ".price", ".review", ".rating", ".description"
                ]);
            }
            "news" => {
                processing_options["extract_article"] = json!(true);
                if matches!(priority, crate::filters::strategy::QueryPriority::Critical | crate::filters::strategy::QueryPriority::High) {
                    processing_options["extract_date"] = json!(true);
                }
                processing_options["focus_selectors"] = json!([
                    "article", ".article-content", ".news-body", "main"
                ]);
            }
            "social" => {
                if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
                    processing_options["extract_posts"] = json!(true);
                    processing_options["extract_metadata"] = json!(true);
                }
            }
            _ => {
                // Auto-detect or general extraction
                processing_options["extract_main_content"] = json!(true);
            }
        }

        payload["processing_options"] = processing_options;

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
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced extraction request failed: {}", e)))?;

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
            log::warn!("Failed to log BrightData request: {}", e);
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

        // Apply filters conditionally based on environment variable with priority awareness
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            if ResponseFilter::is_error_page(&raw_content) {
                return Err(BrightDataError::ToolError("Extraction returned error page".into()));
            } else if ResponseStrategy::should_try_next_source(&raw_content) {
                return Err(BrightDataError::ToolError("Content quality too low".into()));
            }
        }

        // Process and structure the extracted data with priority awareness
        let processed_data = self.process_extracted_data_with_priority(
            &raw_content, url, data_type, extraction_format, clean_content, priority, token_budget
        )?;

        Ok(json!({
            "content": raw_content,
            "processed_content": processed_data.get("processed_content").unwrap_or(&json!("")),
            "extracted_data": processed_data.get("extracted_data").unwrap_or(&json!({})),
            "metadata": {
                "url": url,
                "zone": zone,
                "data_type": data_type,
                "extraction_format": extraction_format,
                "execution_id": execution_id,
                "enable_js": enable_js,
                "extract_images": extract_images,
                "extract_links": extract_links,
                "clean_content": clean_content,
                "priority": format!("{:?}", priority),
                "token_budget": token_budget
            },
            "success": true
        }))
    }

    // ENHANCED: Priority-aware zone selection
    fn select_optimal_zone_with_priority(&self, url: &str, data_type: &str, priority: crate::filters::strategy::QueryPriority) -> String {
        // For critical queries, use best available zones
        if matches!(priority, crate::filters::strategy::QueryPriority::Critical) {
            if url.contains("google.com/search") || url.contains("bing.com/search") || 
               url.contains("yandex.com/search") || url.contains("duckduckgo.com") {
                return env::var("BRIGHTDATA_SERP_ZONE").unwrap_or_else(|_| "serp_api2".to_string());
            }
        }

        // Smart zone selection based on URL patterns and data type
        if url.contains("google.com/search") || url.contains("bing.com/search") || 
           url.contains("yandex.com/search") || url.contains("duckduckgo.com") {
            env::var("BRIGHTDATA_SERP_ZONE").unwrap_or_else(|_| "serp_api2".to_string())
        } else if self.is_social_media_url(url) && !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            env::var("BRIGHTDATA_SOCIAL_ZONE").unwrap_or_else(|_| "social_api".to_string())
        } else if self.is_ecommerce_url(url) && !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            env::var("BRIGHTDATA_ECOMMERCE_ZONE").unwrap_or_else(|_| "ecommerce_api".to_string())
        } else if data_type == "financial" || self.is_financial_url(url) {
            env::var("BRIGHTDATA_FINANCIAL_ZONE").unwrap_or_else(|_| "web_unlocker".to_string())
        } else if self.requires_browser_rendering(url) && matches!(priority, crate::filters::strategy::QueryPriority::Critical | crate::filters::strategy::QueryPriority::High) {
            env::var("BROWSER_ZONE").unwrap_or_else(|_| "browser_zone".to_string())
        } else {
            env::var("WEB_UNLOCKER_ZONE").unwrap_or_else(|_| "default".to_string())
        }
    }

    fn is_social_media_url(&self, url: &str) -> bool {
        let social_domains = [
            "twitter.com", "x.com", "facebook.com", "instagram.com", 
            "linkedin.com", "youtube.com", "tiktok.com", "reddit.com"
        ];
        social_domains.iter().any(|domain| url.contains(domain))
    }

    fn is_ecommerce_url(&self, url: &str) -> bool {
        let ecommerce_domains = [
            "amazon.com", "ebay.com", "alibaba.com", "shopify.com",
            "flipkart.com", "myntra.com", "etsy.com", "walmart.com"
        ];
        ecommerce_domains.iter().any(|domain| url.contains(domain))
    }

    fn is_financial_url(&self, url: &str) -> bool {
        let financial_domains = [
            "bloomberg.com", "reuters.com", "finance.yahoo.com", "marketwatch.com",
            "moneycontrol.com", "investing.com", "cnbc.com", "wsj.com"
        ];
        financial_domains.iter().any(|domain| url.contains(domain))
    }

    fn requires_browser_rendering(&self, url: &str) -> bool {
        let spa_indicators = [
            "/app/", "/dashboard/", "react", "angular", "vue",
            "single-page", "spa"
        ];
        spa_indicators.iter().any(|indicator| url.contains(indicator))
    }

    // ENHANCED: Priority-aware data processing
    fn process_extracted_data_with_priority(
        &self,
        raw_content: &str,
        url: &str,
        data_type: &str,
        extraction_format: &str,
        clean_content: bool,
        priority: crate::filters::strategy::QueryPriority,
        token_budget: usize,
    ) -> Result<Value, BrightDataError> {
        let mut result = json!({});

        // Try to parse JSON response first
        if let Ok(json_data) = serde_json::from_str::<Value>(raw_content) {
            // Structured response from BrightData
            result["extracted_data"] = json_data.clone();
            
            if let Some(content) = json_data.get("content").and_then(|c| c.as_str()) {
                result["processed_content"] = json!(self.process_content_by_type_with_priority(content, data_type, clean_content, priority, token_budget));
            } else {
                result["processed_content"] = json!(self.process_content_by_type_with_priority(raw_content, data_type, clean_content, priority, token_budget));
            }
        } else {
            // Raw content response
            result["processed_content"] = json!(self.process_content_by_type_with_priority(raw_content, data_type, clean_content, priority, token_budget));
            result["extracted_data"] = json!({
                "url": url,
                "content_type": "raw",
                "extraction_format": extraction_format,
                "priority": format!("{:?}", priority)
            });
        }

        Ok(result)
    }

    // ENHANCED: Priority-aware content processing
    fn process_content_by_type_with_priority(&self, content: &str, data_type: &str, clean_content: bool, priority: crate::filters::strategy::QueryPriority, token_budget: usize) -> String {
        let mut processed = content.to_string();

        // Apply filters conditionally with priority awareness
        if clean_content && std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            // Use token budget aware extraction
            let max_tokens = token_budget / 2; // Reserve tokens for formatting
            processed = ResponseFilter::extract_high_value_financial_data(&processed, max_tokens);
        }

        match data_type {
            "financial" => {
                // Extract financial indicators with priority awareness
                processed = self.enhance_financial_content_with_priority(&processed, priority);
            }
            "ecommerce" => {
                // Extract product information (only for higher priority)
                if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
                    processed = self.enhance_ecommerce_content(&processed);
                }
            }
            "news" => {
                // Extract article structure (only for higher priority)
                if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
                    processed = self.enhance_news_content(&processed);
                }
            }
            "social" => {
                // Extract social media structure (only for higher priority)
                if matches!(priority, crate::filters::strategy::QueryPriority::Critical | crate::filters::strategy::QueryPriority::High) {
                    processed = self.enhance_social_content(&processed);
                }
            }
            _ => {
                // General content processing
                if ResponseFilter::contains_financial_data(&processed) {
                    processed = self.enhance_financial_content_with_priority(&processed, priority);
                }
            }
        }

        // Apply size limits conditionally with priority awareness
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let max_length = match priority {
                crate::filters::strategy::QueryPriority::Critical => 15000,
                crate::filters::strategy::QueryPriority::High => 10000,
                crate::filters::strategy::QueryPriority::Medium => 5000,
                crate::filters::strategy::QueryPriority::Low => 2000,
            };
            
            if processed.len() > max_length {
                processed = ResponseFilter::smart_truncate_preserving_financial_data(&processed, max_length);
            }
        }

        processed
    }

    // ENHANCED: Priority-aware financial content enhancement
    fn enhance_financial_content_with_priority(&self, content: &str, priority: crate::filters::strategy::QueryPriority) -> String {
        let mut enhanced = String::new();
        
        // Add markers based on priority
        match priority {
            crate::filters::strategy::QueryPriority::Critical => {
                enhanced.push_str("💰 **CRITICAL Financial Data Extracted**\n\n");
            }
            crate::filters::strategy::QueryPriority::High => {
                enhanced.push_str("📊 **High Priority Financial Data Extracted**\n\n");
            }
            _ => {
                enhanced.push_str("📊 **Financial Data Extracted**\n\n");
            }
        }
        
        // Look for key financial indicators with priority filtering
        if content.contains("price") || content.contains("$") || content.contains("₹") {
            enhanced.push_str("💰 **Price Information Detected**\n");
        }
        
        // Only add detailed markers for higher priority
        if !matches!(priority, crate::filters::strategy::QueryPriority::Low) {
            if content.contains("market cap") || content.contains("volume") {
                enhanced.push_str("📈 **Market Data Available**\n");
            }
            if content.contains("revenue") || content.contains("earnings") {
                enhanced.push_str("💼 **Financial Statements Found**\n");
            }
        }
        
        enhanced.push_str("\n");
        enhanced.push_str(content);
        enhanced
    }

    fn enhance_ecommerce_content(&self, content: &str) -> String {
        let mut enhanced = String::new();
        enhanced.push_str("🛒 **E-commerce Data Extracted**\n\n");
        enhanced.push_str(content);
        enhanced
    }

    fn enhance_news_content(&self, content: &str) -> String {
        let mut enhanced = String::new();
        enhanced.push_str("📰 **News Article Extracted**\n\n");
        enhanced.push_str(content);
        enhanced
    }

    fn enhance_social_content(&self, content: &str) -> String {
        let mut enhanced = String::new();
        enhanced.push_str("📱 **Social Media Content Extracted**\n\n");
        enhanced.push_str(content);
        enhanced
    }
}