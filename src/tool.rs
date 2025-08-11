// src/tool.rs - Fixed version with compilation errors resolved
use async_trait::async_trait;
use crate::error::BrightDataError;
use crate::extras::logger::{JSON_LOGGER, ExecutionLog};
use crate::metrics::{BRIGHTDATA_METRICS, EnhancedLogger};
use serde_json::Value;
use serde::{Deserialize, Serialize};
use log::{info, error};
use std::time::Instant;
use std::collections::HashMap;

// MCP Session Manager for metrics
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

lazy_static::lazy_static! {
    static ref MCP_SESSION_MANAGER: Arc<Mutex<McpSessionManager>> = Arc::new(Mutex::new(McpSessionManager::new()));
}

#[derive(Debug)]
struct McpSessionManager {
    current_session_id: Option<String>,
    session_counter: AtomicU64,
    session_start_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl McpSessionManager {
    fn new() -> Self {
        Self {
            current_session_id: None,
            session_counter: AtomicU64::new(0),
            session_start_time: None,
        }
    }
    
    fn start_new_session(&mut self) -> String {
        let session_count = self.session_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let session_id = format!("mcp_session_{}", session_count);
        
        self.current_session_id = Some(session_id.clone());
        self.session_start_time = Some(chrono::Utc::now());
        
        info!("🎯 MCP Session {} started - resetting metrics", session_id);
        session_id
    }
    
    fn get_current_session(&self) -> Option<String> {
        self.current_session_id.clone()
    }
}

// MCP-compatible content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>, // For base64 encoded data like images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>, // MIME type for binary content
}

impl McpContent {
    pub fn text(text: String) -> Self {
        Self {
            content_type: "text".to_string(),
            text,
            data: None,
            media_type: None,
        }
    }

    pub fn image(data: String, media_type: String) -> Self {
        Self {
            content_type: "image".to_string(),
            text: "Screenshot captured".to_string(),
            data: Some(data),
            media_type: Some(media_type),
        }
    }
}

// Enhanced tool result for MCP compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<McpContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    // Preserve backward compatibility - raw value for existing integrations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<Value>,
    // Add execution metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    // Add session metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl ToolResult {
    pub fn success(content: Vec<McpContent>) -> Self {
        let session_id = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        Self {
            content,
            is_error: Some(false),
            raw_value: None,
            execution_id: None,
            session_id,
        }
    }

    pub fn success_with_text(text: String) -> Self {
        let session_id = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        Self {
            content: vec![McpContent::text(text)],
            is_error: Some(false),
            raw_value: None,
            execution_id: None,
            session_id,
        }
    }

    pub fn success_with_raw(content: Vec<McpContent>, raw: Value) -> Self {
        let session_id = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        Self {
            content,
            is_error: Some(false),
            raw_value: Some(raw),
            execution_id: None,
            session_id,
        }
    }

    pub fn success_with_execution_id(content: Vec<McpContent>, raw: Value, execution_id: String) -> Self {
        let session_id = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        Self {
            content,
            is_error: Some(false),
            raw_value: Some(raw),
            execution_id: Some(execution_id),
            session_id,
        }
    }

    pub fn error(message: String) -> Self {
        let session_id = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        Self {
            content: vec![McpContent::text(format!("Error: {}", message))],
            is_error: Some(true),
            raw_value: None,
            execution_id: None,
            session_id,
        }
    }

    // Backward compatibility method
    pub fn from_legacy_value(value: Value) -> Self {
        let session_id = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        let text = if let Some(raw_text) = value.get("raw").and_then(|v| v.as_str()) {
            raw_text.to_string()
        } else {
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        };

        Self {
            content: vec![McpContent::text(text)],
            is_error: Some(false),
            raw_value: Some(value),
            execution_id: None,
            session_id,
        }
    }
}

#[async_trait]
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    
    // Enhanced execute method with JSON logging AND metrics per MCP session
    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let start_time = Instant::now();
        
        // Get current MCP session
        let current_session = MCP_SESSION_MANAGER.lock().unwrap().get_current_session();
        
        // Start execution logging (existing system)
        let execution_log = JSON_LOGGER.start_execution(self.name(), parameters.clone()).await;
        let execution_id = execution_log.execution_id.clone(); // Clone the ID to use after move
        
        info!("🚀 Starting execution: {} (ID: {}) [Session: {:?}]", 
            self.name(), execution_id, current_session);

        // Execute the actual tool logic
        let result = self.execute_internal(parameters.clone()).await;
        let duration = start_time.elapsed();

        // Complete logging based on result
        match &result {
            Ok(tool_result) => {
                let response_json = serde_json::to_value(tool_result).unwrap_or(serde_json::json!({}));
                
                // Log to existing JSON system
                if let Err(e) = JSON_LOGGER.complete_execution(
                    execution_log, // This moves execution_log
                    response_json.clone(),
                    true,
                    None,
                ).await {
                    error!("Failed to log successful execution: {}", e);
                }
                
                // Log to metrics system with session context (using cloned execution_id)
                if let Err(e) = log_tool_metrics(
                    &execution_id,
                    self.name(),
                    &parameters,
                    tool_result,
                    duration.as_millis() as u64,
                    true,
                    None,
                    current_session.as_deref(),
                ).await {
                    error!("Failed to log metrics: {}", e);
                } else {
                    info!("📊 Metrics logged successfully for {} [Session: {:?}]", self.name(), current_session);
                }
                
                info!("✅ Execution completed successfully: {}", self.name());
            }
            Err(error) => {
                let error_json = serde_json::json!({
                    "error": error.to_string(),
                    "tool": self.name()
                });
                
                // Log to existing JSON system
                if let Err(e) = JSON_LOGGER.complete_execution(
                    execution_log, // This moves execution_log
                    error_json,
                    false,
                    Some(error.to_string()),
                ).await {
                    error!("Failed to log failed execution: {}", e);
                }
                
                // Log error to metrics system with session context (using cloned execution_id)
                if let Err(e) = log_tool_error_metrics(
                    &format!("error_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f")),
                    self.name(),
                    &parameters,
                    &error.to_string(),
                    duration.as_millis() as u64,
                    current_session.as_deref(),
                ).await {
                    error!("Failed to log error metrics: {}", e);
                }
                
                error!("❌ Execution failed: {} - {}", self.name(), error);
            }
        }

        result
    }

    // Internal method that tools should implement (instead of execute)
    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError>;
    
    // Legacy method for backward compatibility
    async fn execute_legacy(&self, parameters: Value) -> Result<Value, BrightDataError> {
        let result = self.execute(parameters).await?;
        if let Some(raw) = result.raw_value {
            Ok(raw)
        } else if !result.content.is_empty() {
            Ok(serde_json::json!({
                "content": result.content[0].text
            }))
        } else {
            Ok(serde_json::json!({}))
        }
    }
}

// MCP Session Management Functions
pub fn handle_mcp_initialize() -> String {
    let session_id = {
        MCP_SESSION_MANAGER.lock().unwrap().start_new_session()
    }; // Clone the session_id to return it
    
    // Reset metrics for new session
    let session_id_clone = session_id.clone(); // Clone for the async block
    tokio::spawn(async move {
        if let Err(e) = reset_metrics_for_new_session(&session_id_clone).await {
            error!("Failed to reset metrics for new session: {}", e);
        }
    });
    
    session_id // Return the original
}

pub fn get_current_mcp_session() -> Option<String> {
    MCP_SESSION_MANAGER.lock().unwrap().get_current_session()
}

async fn reset_metrics_for_new_session(session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🔄 Resetting metrics for new MCP session: {}", session_id);
    
    // Log session start to metrics - FIXED: Add missing anthropic_request_id parameter
    BRIGHTDATA_METRICS.log_call(
        &format!("session_start_{}", session_id),
        &format!("mcp://session/{}", session_id),
        "mcp_session",
        "json",
        Some("session_start"),
        serde_json::json!({
            "event": "mcp_initialize",
            "session_id": session_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }),
        200,
        HashMap::new(),
        &format!("MCP session {} initialized", session_id),
        None,
        0,
        None, // anthropic_request_id
        Some(session_id), // mcp_session_id
    ).await?;
    
    Ok(())
}

// Helper function to log tool metrics with session context
async fn log_tool_metrics(
    execution_id: &str,
    tool_name: &str,
    parameters: &Value,
    tool_result: &ToolResult,
    duration_ms: u64,
    success: bool,
    error_message: Option<&str>,
    session_id: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    // Extract BrightData details if available
    let (url, zone, format) = extract_brightdata_details(parameters, tool_result);
    
    // Get content for analysis
    let content = if !tool_result.content.is_empty() {
        &tool_result.content[0].text
    } else {
        "No content"
    };
    
    if let (Some(url), Some(zone), Some(format)) = (&url, &zone, &format) {
        // This is a BrightData tool - use enhanced logger
        EnhancedLogger::log_brightdata_request_enhanced(
            execution_id,
            zone,
            url,
            parameters.clone(),
            if success { 200 } else { 500 },
            HashMap::new(),
            format,
            content,
            None, // filtered_content
            std::time::Duration::from_millis(duration_ms),
            session_id,
        ).await?;
        
        info!("📊 Logged BrightData tool {} to metrics [Session: {:?}]", tool_name, session_id);
    } else {
        // This is a non-BrightData tool - log directly to metrics
        // FIXED: Add missing anthropic_request_id parameter
        BRIGHTDATA_METRICS.log_call(
            execution_id,
            &format!("tool://{}", tool_name),
            "local_tool",
            "json",
            Some("tool_output"),
            parameters.clone(),
            if success { 200 } else { 500 },
            HashMap::new(),
            content,
            None,
            duration_ms,
            None, // anthropic_request_id
            session_id,
        ).await?;
        
        info!("📊 Logged generic tool {} to metrics [Session: {:?}]", tool_name, session_id);
    }
    
    Ok(())
}

// Helper function to log error metrics with session context
async fn log_tool_error_metrics(
    execution_id: &str,
    tool_name: &str,
    parameters: &Value,
    error_message: &str,
    duration_ms: u64,
    session_id: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    // FIXED: Add missing anthropic_request_id parameter
    BRIGHTDATA_METRICS.log_call(
        execution_id,
        &format!("tool://{}", tool_name),
        "error",
        "json",
        Some("error"),
        parameters.clone(),
        500,
        HashMap::new(),
        &format!("Error: {}", error_message),
        None,
        duration_ms,
        None, // anthropic_request_id
        session_id,
    ).await?;
    
    info!("📊 Logged error metrics for {} [Session: {:?}]", tool_name, session_id);
    Ok(())
}

// Extract BrightData details from tool parameters and results
fn extract_brightdata_details(parameters: &Value, tool_result: &ToolResult) -> (Option<String>, Option<String>, Option<String>) {
    let mut url = None;
    let mut zone = None;
    let mut format = None;
    
    // Try to extract from parameters
    if let Some(param_url) = parameters.get("url").and_then(|v| v.as_str()) {
        url = Some(param_url.to_string());
    }
    
    // Try to extract from query (for search tools)
    if let Some(query) = parameters.get("query").and_then(|v| v.as_str()) {
        if url.is_none() {
            url = Some(format!("search:{}", query));
        }
    }
    
    // Try to extract from tool result
    if let Some(raw_value) = &tool_result.raw_value {
        if let Some(result_url) = raw_value.get("url").and_then(|v| v.as_str()) {
            url = Some(result_url.to_string());
        }
        if let Some(result_zone) = raw_value.get("zone").and_then(|v| v.as_str()) {
            zone = Some(result_zone.to_string());
        }
        if let Some(result_format) = raw_value.get("format").and_then(|v| v.as_str()) {
            format = Some(result_format.to_string());
        }
    }
    
    // Set defaults if not found
    if zone.is_none() {
        zone = Some(std::env::var("WEB_UNLOCKER_ZONE").unwrap_or_else(|_| "default".to_string()));
    }
    
    if format.is_none() {
        format = Some("markdown".to_string());
    }
    
    (url, zone, format)
}

// Enhanced tool resolver with schema support
pub struct ToolResolver;

impl Default for ToolResolver {
    fn default() -> Self {
        Self
    }
}

impl ToolResolver {
    pub fn resolve(&self, name: &str) -> Option<Box<dyn Tool + Send + Sync>> {
        match name {
            // Core tools
            // "search_web" => Some(Box::new(crate::tools::search::SearchEngine)),
            "extract_data" => Some(Box::new(crate::tools::extract::Extractor)),
            "scrape_website" => Some(Box::new(crate::tools::scrape::Scraper)),
            // "take_screenshot" => Some(Box::new(crate::tools::screenshot::ScreenshotTool)),
            
            // Financial tools - using individual modules
            "get_stock_data" => Some(Box::new(crate::tools::stock::StockDataTool)),
            "get_crypto_data" => Some(Box::new(crate::tools::crypto::CryptoDataTool)),
            "get_etf_data" => Some(Box::new(crate::tools::etf::ETFDataTool)),
            "get_bond_data" => Some(Box::new(crate::tools::bond::BondDataTool)),
            "get_mutual_fund_data" => Some(Box::new(crate::tools::mutual_fund::MutualFundDataTool)),
            "get_commodity_data" => Some(Box::new(crate::tools::commodity::CommodityDataTool)),
            
            // Additional tools if needed
            "multi_zone_search" => Some(Box::new(crate::tools::multi_zone_search::MultiZoneSearch)),
            
            _ => None,
        }
    }

    pub fn get_extract_data_tool(&self) -> Option<Box<dyn Tool + Send + Sync>> {
        self.resolve("extract_data")
    }

    pub fn list_tools(&self) -> Vec<Value> {
        vec![
            // Core tools
            // serde_json::json!({
            //     "name": "search_web",
            //     "description": "Search the web using various search engines via BrightData",
            //     "inputSchema": {
            //         "type": "object",
            //         "properties": {
            //             "query": {
            //                 "type": "string",
            //                 "description": "Search query"
            //             },
            //             "engine": {
            //                 "type": "string",
            //                 "enum": ["google", "bing", "yandex", "duckduckgo"],
            //                 "description": "Search engine to use",
            //                 "default": "google"
            //             },
            //             "cursor": {
            //                 "type": "string",
            //                 "description": "Pagination cursor/page number",
            //                 "default": "0"
            //             }
            //         },
            //         "required": ["query"]
            //     }
            // }),
            serde_json::json!({
                "name": "scrape_website",
                "description": "Scrap structured data from a webpage using AI analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to Scrap data from"
                        },
                        "schema": {
                            "type": "object",
                            "description": "Optional schema to guide extraction",
                            "additionalProperties": true
                        }
                    },
                    "required": ["url"]
                }
            }),
            serde_json::json!({
                "name": "extract_data",
                "description": "Extract structured data from a webpage using AI analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to extract data from"
                        },
                        "schema": {
                            "type": "object",
                            "description": "Optional schema to guide extraction",
                            "additionalProperties": true
                        }
                    },
                    "required": ["url"]
                }
            }),
            // serde_json::json!({
            //     "name": "take_screenshot",
            //     "description": "Take a screenshot of a webpage using BrightData Browser",
            //     "inputSchema": {
            //         "type": "object",
            //         "properties": {
            //             "url": {
            //                 "type": "string",
            //                 "description": "The URL to screenshot"
            //             },
            //             "width": {
            //                 "type": "integer",
            //                 "description": "Screenshot width",
            //                 "default": 1280,
            //                 "minimum": 320,
            //                 "maximum": 1920
            //             },
            //             "height": {
            //                 "type": "integer",
            //                 "description": "Screenshot height",
            //                 "default": 720,
            //                 "minimum": 240,
            //                 "maximum": 1080
            //             },
            //             "full_page": {
            //                 "type": "boolean",
            //                 "description": "Capture full page height",
            //                 "default": false
            //             }
            //         },
            //         "required": ["url"]
            //     }
            // }),

            // Financial tools
            serde_json::json!({
                "name": "get_stock_data",
                "description": "Get comprehensive stock data including prices, performance, market cap, volumes. Use for individual stocks, stock comparisons, or stock market overviews",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string", 
                            "description": "Stock symbol (e.g. ASHOKLEY, TCS, AAPL), company name, comparison query (AAPL vs MSFT), or market overview request (today's stock performance, Nifty 50 performance)" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian",
                            "description": "Market region - indian for NSE/BSE stocks, us for NASDAQ/NYSE, global for international"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_crypto_data",
                "description": "Get cryptocurrency data including prices, market cap, trading volumes. Use for individual cryptos, crypto comparisons (BTC vs ETH), or overall crypto market analysis",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Crypto symbol (BTC, ETH, ADA), crypto name (Bitcoin, Ethereum), comparison query (BTC vs ETH), or market overview (crypto market today, top cryptocurrencies)" 
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_etf_data",
                "description": "Get ETF and index fund data including NAV, holdings, performance, expense ratios",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "ETF symbol (SPY, NIFTYBEES), ETF name, or ETF market analysis query" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_bond_data",
                "description": "Get bond market data including yields, government bonds, corporate bonds, and bond market trends",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Bond type (government bonds, corporate bonds), yield query (10-year yield), or bond market analysis" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_mutual_fund_data",
                "description": "Get mutual fund data including NAV, performance, portfolio composition, and fund comparisons",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Fund name, fund symbol, fund category (equity funds, debt funds), or fund comparison query" 
                        },
                        "market": { 
                            "type": "string", 
                            "enum": ["indian", "us", "global"], 
                            "default": "indian"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "get_commodity_data",
                "description": "Get commodity prices and market data including gold, silver, oil, agricultural commodities",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { 
                            "type": "string", 
                            "description": "Commodity name (gold, silver, crude oil), commodity symbol, or commodity market overview" 
                        }
                    },
                    "required": ["query"]
                }
            })
            // serde_json::json!({
            //     "name": "multi_zone_search",
            //     "description": "Performs the same search query across multiple BrightData zones in parallel",
            //     "inputSchema": {
            //         "type": "object",
            //         "properties": {
            //             "query": { "type": "string" },
            //             "engine": {
            //                 "type": "string",
            //                 "enum": ["google", "bing", "yandex", "duckduckgo"],
            //                 "default": "google"
            //             },
            //             "zones": {
            //                 "type": "array",
            //                 "items": { "type": "string" },
            //                 "description": "List of BrightData zone names to run parallel searches"
            //             }
            //         },
            //         "required": ["query", "zones"]
            //     }
            // })
        ]
    }

    // Helper method to get all available tool names
    pub fn get_available_tool_names(&self) -> Vec<&'static str> {
        vec![
            // "search_web", 
            "extract_data",
            "scrape_website",
            // "take_screenshot",
            "get_stock_data",
            "get_crypto_data",
            "get_etf_data",
            "get_bond_data",
            "get_mutual_fund_data",
            "get_commodity_data",
            // "multi_zone_search"
        ]
    }

    // Helper method to check if a tool exists
    pub fn tool_exists(&self, name: &str) -> bool {
        self.get_available_tool_names().contains(&name)
    }

    // Helper method to get tool count
    pub fn tool_count(&self) -> usize {
        self.get_available_tool_names().len()
    }
}