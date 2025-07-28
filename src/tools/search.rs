// src/tools/search.rs - Safe version with timeouts and limits
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;
use scraper::{Html, Selector};
use log::{info, error, debug, warn};

const MAX_RESPONSE_SIZE: usize = 1_000_000; // 1MB limit
const REQUEST_TIMEOUT: u64 = 30; // 30 seconds
const MAX_RESULTS: usize = 5; // Limit results

pub struct SearchEngine;

#[async_trait]
impl Tool for SearchEngine {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search the web using BrightData SERP proxy and extract results"
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
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let engine = parameters
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("google");

        info!("🔍 Starting web search for query: '{}'", query);
        
        // Add timeout wrapper
        let search_future = self.search_with_brightdata(query, engine);
        let timeout_duration = Duration::from_secs(REQUEST_TIMEOUT);
        
        let result = match tokio::time::timeout(timeout_duration, search_future).await {
            Ok(result) => result?,
            Err(_) => {
                error!("⏱️ Search request timed out after {} seconds", REQUEST_TIMEOUT);
                return Err(BrightDataError::ToolError("Search request timed out".into()));
            }
        };
        
        // Try structured JSON first
        if let Some(organic_results) = result.get("organic").and_then(|v| v.as_array()) {
            if !organic_results.is_empty() {
                return self.format_structured_results(query, organic_results, &result);
            }
        }
        
        // Fallback to HTML parsing with size check
        if let Some(html_content) = result.as_object()
            .and_then(|obj| obj.get("body"))
            .and_then(|body| body.as_str()) {
            
            // Check response size
            if html_content.len() > MAX_RESPONSE_SIZE {
                warn!("⚠️ Response too large: {} bytes, truncating", html_content.len());
                let truncated = &html_content[..MAX_RESPONSE_SIZE];
                return self.parse_html_results(query, truncated);
            }
            
            info!("📄 Parsing HTML content ({} bytes)", html_content.len());
            return self.parse_html_results(query, html_content);
        }

        error!("❌ No valid search results found for query: '{}'", query);
        Err(BrightDataError::ToolError("No valid search results found".into()))
    }
}

impl SearchEngine {
    async fn search_with_brightdata(&self, query: &str, engine: &str) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = "https://api.brightdata.com";
        let search_url = self.build_search_url(engine, query);
        let zone = std::env::var("BRIGHTDATA_SERP_ZONE")
            .unwrap_or_else(|_| "serp_api2".to_string());

        let payload = json!({
            "url": search_url,
            "zone": zone,
            "format": "raw"  // Use raw format to avoid JSON parsing issues
        });

        info!("🌐 Making BrightData request to: {}", search_url);
        debug!("📦 Payload: {}", payload);

        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT))
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

        let status = response.status();
        info!("📡 BrightData response status: {}", status);
        
        if !status.is_success() {
            let err_text = response.text().await.unwrap_or_default();
            error!("❌ BrightData API error {}: {}", status, err_text);
            return Err(BrightDataError::ToolError(format!(
                "BrightData error {}: {}",
                status, err_text
            )));
        }

        // Get response text with size limit
        let response_text = response.text().await
            .map_err(|e| BrightDataError::ToolError(format!("Failed to read response: {}", e)))?;

        info!("✅ Received response, length: {} bytes", response_text.len());

        // Return as JSON wrapper for HTML content
        Ok(json!({
            "body": response_text,
            "format": "html",
            "success": true
        }))
    }

    fn format_structured_results(&self, query: &str, organic_results: &[Value], full_result: &Value) -> Result<ToolResult, BrightDataError> {
        let mut formatted_results = Vec::new();
        
        // Limit results to prevent huge responses
        for (i, result) in organic_results.iter().take(MAX_RESULTS).enumerate() {
            let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("No title");
            let link = result.get("link").and_then(|l| l.as_str()).unwrap_or("");
            let description = result.get("description").and_then(|d| d.as_str()).unwrap_or("");
            
            formatted_results.push(format!(
                "{}. **{}**\n   {}\n   Link: {}\n", 
                i + 1, title, description, link
            ));
        }

        let content_text = format!("🔍 **Search Results for '{}'**\n\n{}", query, formatted_results.join("\n"));
        let mcp_content = vec![McpContent::text(content_text)];
        
        info!("✅ Returning {} structured search results", organic_results.len().min(MAX_RESULTS));
        Ok(ToolResult::success_with_raw(mcp_content, full_result.clone()))
    }

    fn parse_html_results(&self, query: &str, html_content: &str) -> Result<ToolResult, BrightDataError> {
        info!("🔧 Starting HTML parsing for {} bytes", html_content.len());
        
        let document = Html::parse_document(html_content);
        let mut results = Vec::new();
        
        // Simple, safe selector that won't cause infinite loops
        if let Ok(selector) = Selector::parse("a[href*='http']") {
            let mut count = 0;
            for element in document.select(&selector) {
                // Hard limit to prevent infinite loops
                count += 1;
                if count > 20 {
                    warn!("⚠️ Reached maximum link extraction limit (20)");
                    break;
                }
                
                if let Some(href) = element.value().attr("href") {
                    let text = element.text().collect::<String>().trim().to_string();
                    
                    // Basic filtering
                    if text.len() > 5 && text.len() < 200 && 
                       !text.to_lowercase().contains("sign in") &&
                       !href.contains("accounts.google.com") {
                        results.push((text, href.to_string()));
                        
                        // Stop when we have enough results
                        if results.len() >= MAX_RESULTS {
                            break;
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            return Err(BrightDataError::ToolError("No search results found in HTML".into()));
        }

        // Format results with limit
        let formatted_results: Vec<String> = results.iter().take(MAX_RESULTS).enumerate().map(|(i, (title, url))| {
            format!("{}. **{}**\n   Link: {}\n", i + 1, title, url)
        }).collect();

        let content_text = format!("🔍 **Search Results for '{}'**\n\n{}", 
                                 query, formatted_results.join("\n"));

        let mcp_content = vec![McpContent::text(content_text)];
        info!("✅ Extracted {} results from HTML", results.len());

        let raw_result = json!({
            "query": query,
            "results": results.iter().take(MAX_RESULTS).map(|(title, url)| json!({
                "title": title,
                "url": url
            })).collect::<Vec<_>>(),
            "source": "html_parsed"
        });

        Ok(ToolResult::success_with_raw(mcp_content, raw_result))
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
}