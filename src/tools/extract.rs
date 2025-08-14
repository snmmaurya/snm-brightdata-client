// src/tools/extract.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::extras::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy};
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
        "Extract structured data from a webpage using BrightData with WEB_UNLOCKER_ZONE"
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
                }
            },
            "required": ["url", "session_id"]
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

        let clean_content = parameters
            .get("clean_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let schema = parameters.get("schema").cloned();

        let execution_id = self.generate_execution_id();
        
        match self.extract_with_brightdata(url, data_type, extraction_format, clean_content, schema, &execution_id).await {
            Ok(result) => {
                let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                
                // Create formatted response based on DEDUCT_DATA setting
                let formatted_response = self.create_formatted_extract_response(
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
                let empty_response = json!({
                    "url": url,
                    "data_type": data_type,
                    "status": "no_data",
                    "reason": "brightdata_error",
                    "execution_id": execution_id
                });
                
                Ok(ToolResult::success_with_raw(
                    vec![McpContent::text("📊 **No Data Available**\n\nPlease try again with a different URL or check if the website is accessible.".to_string())],
                    empty_response
                ))
            }
        }
    }
}

impl Extractor {
    /// Check if data reduction is enabled via DEDUCT_DATA environment variable only
    fn is_data_reduction_enabled(&self) -> bool {
        std::env::var("DEDUCT_DATA")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// Create formatted response with DEDUCT_DATA control
    fn create_formatted_extract_response(
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
        format!("extract_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"))
    }

    /// Extract data with BrightData using only WEB_UNLOCKER_ZONE
    async fn extract_with_brightdata(
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

        // Return raw content directly without processing
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
                "data_reduction_enabled": self.is_data_reduction_enabled()
            },
            "success": true
        }))
    }
}