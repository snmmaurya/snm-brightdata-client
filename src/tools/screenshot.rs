// src/tools/screenshot.rs - Enhanced with browser parameters and smart device selection
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use async_trait::async_trait;
use serde_json::{Value, json};
use reqwest::Client;
use std::time::Duration;
use std::collections::HashMap;
use log::info;

pub struct ScreenshotTool;

#[async_trait]
impl Tool for ScreenshotTool {
    fn name(&self) -> &str {
        "take_screenshot"
    }

    fn description(&self) -> &str {
        "Take a screenshot of a webpage using BrightData Browser with enhanced device and browser simulation"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to screenshot"
                },
                "width": {
                    "type": "integer",
                    "description": "Screenshot width",
                    "default": 1280,
                    "minimum": 320,
                    "maximum": 1920
                },
                "height": {
                    "type": "integer",
                    "description": "Screenshot height", 
                    "default": 720,
                    "minimum": 240,
                    "maximum": 1080
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture full page height",
                    "default": false
                },
                "device_type": {
                    "type": "string",
                    "enum": ["desktop", "mobile", "tablet"],
                    "description": "Device type simulation",
                    "default": "desktop"
                },
                "browser": {
                    "type": "string",
                    "enum": ["chrome", "firefox", "safari", "edge"],
                    "description": "Browser to simulate",
                    "default": "chrome"
                },
                "wait_time": {
                    "type": "integer",
                    "description": "Wait time in seconds before taking screenshot",
                    "minimum": 0,
                    "maximum": 30,
                    "default": 3
                },
                "quality": {
                    "type": "string",
                    "enum": ["low", "medium", "high"],
                    "description": "Screenshot quality",
                    "default": "high"
                },
                "format": {
                    "type": "string",
                    "enum": ["png", "jpeg", "webp"],
                    "description": "Screenshot format",
                    "default": "png"
                },
                "dark_mode": {
                    "type": "boolean",
                    "description": "Enable dark mode if supported by site",
                    "default": false
                },
                "disable_animations": {
                    "type": "boolean",
                    "description": "Disable CSS animations for stable screenshots",
                    "default": true
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

        let width = parameters
            .get("width")
            .and_then(|v| v.as_i64())
            .unwrap_or(1280);

        let height = parameters
            .get("height")
            .and_then(|v| v.as_i64())
            .unwrap_or(720);

        let full_page = parameters
            .get("full_page")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let device_type = parameters
            .get("device_type")
            .and_then(|v| v.as_str())
            .unwrap_or("desktop");

        let browser = parameters
            .get("browser")
            .and_then(|v| v.as_str())
            .unwrap_or("chrome");

        let wait_time = parameters
            .get("wait_time")
            .and_then(|v| v.as_i64())
            .unwrap_or(3);

        let quality = parameters
            .get("quality")
            .and_then(|v| v.as_str())
            .unwrap_or("high");

        let format = parameters
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("png");

        let dark_mode = parameters
            .get("dark_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let disable_animations = parameters
            .get("disable_animations")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let execution_id = format!("screenshot_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));
        
        let result = self.screenshot_with_brightdata_enhanced(
            url, width, height, full_page, device_type, browser, wait_time, 
            quality, format, dark_mode, disable_animations, &execution_id
        ).await?;

        let mcp_content = vec![McpContent::text(format!(
            "📸 **Enhanced Screenshot captured from: {}**\n\nDimensions: {}x{} | Device: {} | Browser: {} | Format: {}\nFull page: {} | Quality: {} | Wait time: {}s | Dark mode: {}\nZone: {} | Execution ID: {}\n\nNote: Screenshot data available in raw response",
            url,
            width,
            height,
            device_type,
            browser,
            format,
            full_page,
            quality,
            wait_time,
            dark_mode,
            result.get("zone").and_then(|z| z.as_str()).unwrap_or("unknown"),
            execution_id
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl ScreenshotTool {
    async fn screenshot_with_brightdata_enhanced(
        &self, 
        url: &str, 
        width: i64, 
        height: i64, 
        full_page: bool,
        device_type: &str,
        browser: &str,
        wait_time: i64,
        quality: &str,
        format: &str,
        dark_mode: bool,
        disable_animations: bool,
        execution_id: &str
    ) -> Result<Value, BrightDataError> {
        let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
            .or_else(|_| std::env::var("API_TOKEN"))
            .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

        let zone = std::env::var("BROWSER_ZONE")
            .or_else(|_| std::env::var("BRIGHTDATA_BROWSER_ZONE"))
            .unwrap_or_else(|_| "default_browser".to_string());

        info!("📸 Enhanced screenshot of {} ({}x{}, {}, {}) using zone: {} (execution: {})", 
              url, width, height, device_type, browser, zone, execution_id);

        // Get device-specific dimensions and user agent settings
        let (final_width, final_height, user_agent_type) = self.get_device_settings(device_type, width, height);

        let mut payload = json!({
            "url": url,
            "zone": zone,
            "format": "raw",
            "data_format": "screenshot",
            "viewport": {
                "width": final_width,
                "height": final_height
            },
            "full_page": full_page,
            "screenshot_options": {
                "quality": self.get_quality_value(quality),
                "format": format,
                "wait_time": wait_time * 1000, // Convert to milliseconds
                "disable_animations": disable_animations
            }
        });

        // Add browser simulation parameters (similar to SERP API)
        if user_agent_type == "mobile" {
            payload["brd_mobile"] = json!(1);
        } else if user_agent_type == "tablet" {
            payload["brd_mobile"] = json!("ipad");
        } else {
            payload["brd_mobile"] = json!(0);
        }

        // Browser selection
        payload["brd_browser"] = json!(browser);

        // Dark mode simulation
        if dark_mode {
            payload["color_scheme"] = json!("dark");
        }

        // Add JavaScript to inject for better screenshot stability
        let mut js_injection = Vec::new();
        
        if disable_animations {
            js_injection.push("document.querySelectorAll('*').forEach(el => { el.style.animationDuration = '0s'; el.style.transitionDuration = '0s'; });".to_string());
        }
        
        if dark_mode {
            js_injection.push("document.documentElement.setAttribute('data-theme', 'dark');".to_string());
        }

        if !js_injection.is_empty() {
            payload["js_injection"] = json!(js_injection.join(" "));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        let response = client
            .post(&format!("{}/request", base_url))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| BrightDataError::ToolError(format!("Enhanced screenshot request failed: {}", e)))?;

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
            "screenshot"
        ).await {
            log::warn!("Failed to log BrightData request: {}", e);
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BrightDataError::ToolError(format!(
                "BrightData enhanced screenshot error {}: {}",
                status, error_text
            )));
        }

        let content = response.text().await
            .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

        Ok(json!({
            "screenshot_data": content,
            "url": url,
            "zone": zone,
            "viewport": {
                "width": final_width,
                "height": final_height
            },
            "device_type": device_type,
            "browser": browser,
            "full_page": full_page,
            "quality": quality,
            "format": format,
            "wait_time": wait_time,
            "dark_mode": dark_mode,
            "disable_animations": disable_animations,
            "execution_id": execution_id,
            "success": true
        }))
    }

    fn get_device_settings(&self, device_type: &str, width: i64, height: i64) -> (i64, i64, &str) {
        match device_type {
            "mobile" => {
                // Use mobile dimensions if not specified
                let mobile_width = if width == 1280 { 375 } else { width };
                let mobile_height = if height == 720 { 667 } else { height };
                (mobile_width, mobile_height, "mobile")
            }
            "tablet" => {
                // Use tablet dimensions if not specified
                let tablet_width = if width == 1280 { 768 } else { width };
                let tablet_height = if height == 720 { 1024 } else { height };
                (tablet_width, tablet_height, "tablet")
            }
            _ => {
                // Desktop dimensions
                (width, height, "desktop")
            }
        }
    }

    fn get_quality_value(&self, quality: &str) -> i32 {
        match quality {
            "low" => 50,
            "medium" => 75,
            "high" => 90,
            _ => 90
        }
    }
}