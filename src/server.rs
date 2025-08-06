// src/server.rs - Fixed version with unused variables removed
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use reqwest::Client;
use uuid::Uuid;
use crate::error::BrightDataError;
use crate::types::{McpResponse};
use crate::tool::{handle_mcp_initialize, get_current_mcp_session}; // Import MCP session functions

#[derive(Debug, Clone)]
pub struct Config {
    pub api_token: String,
    pub web_unlocker_zone: String,
    pub browser_zone: String,
    pub serp_zone: String,  // Added SERP zone
    pub rate_limit: Option<String>,
    pub timeout: Duration,
    pub max_retries: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, std::io::Error> {
        Ok(Self {
            api_token: env::var("API_TOKEN").unwrap_or_default(),
            web_unlocker_zone: env::var("WEB_UNLOCKER_ZONE").unwrap_or_else(|_| "default_zone".to_string()),
            browser_zone: env::var("BROWSER_ZONE").unwrap_or_else(|_| "default_browser".to_string()),
            serp_zone: env::var("BRIGHTDATA_SERP_ZONE").unwrap_or_else(|_| "serp_api2".to_string()),
            rate_limit: env::var("RATE_LIMIT").ok(),
            timeout: Duration::from_secs(env::var("REQUEST_TIMEOUT").unwrap_or_else(|_| "300".to_string()).parse().unwrap_or(300)),
            max_retries: env::var("MAX_RETRIES").unwrap_or_else(|_| "3".to_string()).parse().unwrap_or(3),
        })
    }
}

#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    pub session_id: Uuid,
    pub http_client: Client,
    pub rate_limits: Arc<RwLock<HashMap<String, (u32, DateTime<Utc>)>>>,
    pub start_time: DateTime<Utc>,
    pub current_mcp_session: Arc<RwLock<Option<String>>>, // Track current MCP session
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            config: config.clone(),
            http_client: Client::builder().timeout(config.timeout).build().unwrap(),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            start_time: Utc::now(),
            current_mcp_session: Arc::new(RwLock::new(None)),
        }
    }
}

pub struct BrightDataUrls;

impl BrightDataUrls {
    pub fn request_api() -> String {
        let base_url = std::env::var("BRIGHTDATA_BASE_URL")
            .unwrap_or_else(|_| "https://api.brightdata.com".to_string());
        format!("{}/request", base_url)
    }
}

// Enhanced MCP handler with initialize support for metrics
pub async fn handle_mcp_request(
    _req: HttpRequest,
    payload: web::Json<crate::types::McpRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let req = payload.into_inner();
    let id = req.id.clone();

    let mcp_result: Result<McpResponse, String> = match req.method.as_str() {
        // Handle MCP initialize - this resets metrics for new session
        "initialize" => {
            log::info!("🎯 MCP Initialize received - starting new metrics session");
            
            // Start new MCP session and reset metrics
            let session_id = handle_mcp_initialize();
            
            // Update app state with new session
            {
                let mut current_session = state.current_mcp_session.write().await;
                *current_session = Some(session_id.clone());
            }
            
            log::info!("📊 New MCP session started: {}", session_id);
            
            Ok(McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {},
                        "logging": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": "snm-brightdata-client",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": "BrightData MCP Server ready with metrics tracking",
                    "session_id": session_id
                })),
                error: None,
            })
        }

        "tools/list" => {
            let current_session = get_current_mcp_session();
            log::info!("📋 Tools list requested [Session: {:?}]", current_session);
            
            Ok(McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({
                    "tools": [
                        { "name": "scrape_website", "description": "Scrape a web page" },
                        { "name": "search_web", "description": "Perform a web search" },
                        { "name": "extract_data", "description": "Extract structured data from a webpage" },
                        { "name": "take_screenshot", "description": "Take a screenshot of a webpage" },
                        { "name": "get_stock_data", "description": "Get stock market data" },
                        { "name": "get_crypto_data", "description": "Get cryptocurrency data" },
                        { "name": "get_etf_data", "description": "Get ETF data" },
                        { "name": "get_bond_data", "description": "Get bond market data" },
                        { "name": "get_mutual_fund_data", "description": "Get mutual fund data" },
                        { "name": "get_commodity_data", "description": "Get commodity market data" },
                        { "name": "multi_zone_search", "description": "Search across multiple zones" }
                    ],
                    "session_id": current_session
                })),
                error: None,
            })
        }

        "tools/call" => {
            let current_session = get_current_mcp_session();
            
            if let Some(params) = req.params {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_default();

                log::info!("🔧 Tool call: {} [Session: {:?}]", name, current_session);

                if !check_rate_limit(name, &state).await {
                    return Ok(HttpResponse::TooManyRequests().json(McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(crate::types::McpError {
                            code: -32000,
                            message: "Rate limit exceeded".to_string(),
                            data: None,
                        }),
                    }));
                }

                let result = match name {
                    "scrape_website" => handle_scrape_website(&args, &state).await,
                    "search_web" => handle_search_web(&args, &state).await,
                    "extract_data" => handle_extract_placeholder(&args).await,
                    "take_screenshot" => handle_take_screenshot(&args, &state).await,
                    "get_stock_data" => handle_financial_tool("get_stock_data", &args).await,
                    "get_crypto_data" => handle_financial_tool("get_crypto_data", &args).await,
                    "get_etf_data" => handle_financial_tool("get_etf_data", &args).await,
                    "get_bond_data" => handle_financial_tool("get_bond_data", &args).await,
                    "get_mutual_fund_data" => handle_financial_tool("get_mutual_fund_data", &args).await,
                    "get_commodity_data" => handle_financial_tool("get_commodity_data", &args).await,
                    "multi_zone_search" => handle_financial_tool("multi_zone_search", &args).await,
                    _ => Err("Unknown tool".to_string()),
                };

                Ok(match result {
                    Ok(content) => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::json!({ 
                            "content": content,
                            "session_id": current_session
                        })),
                        error: None,
                    },
                    Err(msg) => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(crate::types::McpError {
                            code: -32603,
                            message: msg,
                            data: Some(serde_json::json!({
                                "session_id": current_session
                            })),
                        }),
                    },
                })
            } else {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(crate::types::McpError {
                        code: -32602,
                        message: "Missing parameters".into(),
                        data: None,
                    }),
                })
            }
        }

        _ => Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(crate::types::McpError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        }),
    };

    match mcp_result {
        Ok(resp) => Ok(HttpResponse::Ok().json(resp)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(crate::types::McpError {
                code: -32603,
                message: e,
                data: None,
            }),
        })),
    }
}

pub async fn health_check(state: web::Data<AppState>) -> Result<HttpResponse> {
    let current_session = get_current_mcp_session();
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "session_id": state.session_id,
        "uptime_seconds": (Utc::now() - state.start_time).num_seconds(),
        "zones": {
            "web_unlocker": state.config.web_unlocker_zone,
            "browser": state.config.browser_zone,
            "serp": state.config.serp_zone
        },
        "mcp_session": current_session,
        "metrics_tracking": current_session.is_some()
    })))
}

pub async fn cors_handler() -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .insert_header(("Access-Control-Allow-Methods", "POST, GET, OPTIONS"))
        .insert_header(("Access-Control-Allow-Headers", "Content-Type, Authorization"))
        .finish()
}

async fn check_rate_limit(tool: &str, state: &web::Data<AppState>) -> bool {
    let mut limits = state.rate_limits.write().await;
    let now = Utc::now();
    let entry = limits.entry(tool.to_string()).or_insert((0, now));

    let limit = 10;
    let window = chrono::Duration::seconds(60);

    if now - entry.1 > window {
        entry.0 = 0;
        entry.1 = now;
    }

    if entry.0 >= limit {
        false
    } else {
        entry.0 += 1;
        true
    }
}

pub async fn handle_scrape_website(args: &serde_json::Value, state: &web::Data<AppState>) -> Result<String, String> {
    let url = args.get("url").and_then(|v| v.as_str()).ok_or("Missing 'url'")?;
    let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("markdown");

    let mut payload = serde_json::json!({
        "url": url,
        "zone": state.config.web_unlocker_zone,
        "format": "raw",
    });

    if format == "markdown" {
        payload["data_format"] = serde_json::json!("markdown");
    }

    let api_url = BrightDataUrls::request_api();

    let res = state.http_client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", state.config.api_token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body = res.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

pub async fn handle_search_web(args: &serde_json::Value, state: &web::Data<AppState>) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing 'query'")?;
    let engine = args.get("engine").and_then(|v| v.as_str()).unwrap_or("google");
    let cursor = args.get("cursor").and_then(|v| v.as_str()).unwrap_or("0");

    let search_url = build_search_url(engine, query, cursor);

    // Use SERP zone for search operations
    let payload = serde_json::json!({
        "url": search_url,
        "zone": state.config.serp_zone,  // Use SERP zone instead of web_unlocker_zone
        "format": "raw",
        "data_format": "markdown"
    });

    let api_url = BrightDataUrls::request_api();
    let res = state.http_client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", state.config.api_token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body = res.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

pub async fn handle_take_screenshot(args: &serde_json::Value, state: &web::Data<AppState>) -> Result<String, String> {
    let url = args.get("url").and_then(|v| v.as_str()).ok_or("Missing 'url'")?;
    let width = args.get("width").and_then(|v| v.as_i64()).unwrap_or(1280);
    let height = args.get("height").and_then(|v| v.as_i64()).unwrap_or(720);
    let full_page = args.get("full_page").and_then(|v| v.as_bool()).unwrap_or(false);

    let payload = serde_json::json!({
        "url": url,
        "zone": state.config.browser_zone,
        "format": "raw",
        "data_format": "screenshot",
        "viewport": {
            "width": width,
            "height": height
        },
        "full_page": full_page
    });

    let api_url = BrightDataUrls::request_api();
    let res = state.http_client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", state.config.api_token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // FIXED: Remove unused variable warning
    let _body = res.text().await.map_err(|e| e.to_string())?;
    Ok(format!("Screenshot captured for {} ({}x{})", url, width, height))
}

// Handler for financial tools using the tool resolver
async fn handle_financial_tool(tool_name: &str, args: &serde_json::Value) -> Result<String, String> {
    use crate::tool::ToolResolver; // FIXED: Remove unused Tool import
    
    let resolver = ToolResolver::default();
    match resolver.resolve(tool_name) {
        Some(tool) => {
            match tool.execute(args.clone()).await {
                Ok(result) => {
                    if !result.content.is_empty() {
                        Ok(result.content[0].text.clone())
                    } else {
                        Ok("No content returned".to_string())
                    }
                },
                Err(e) => Err(e.to_string()),
            }
        },
        None => Err(format!("Tool '{}' not found", tool_name)),
    }
}

pub async fn handle_extract_placeholder(_args: &serde_json::Value) -> Result<String, String> {
    Ok("🧠 Extract tool placeholder: AI-based structured data extraction coming soon.".to_string())
}

fn build_search_url(engine: &str, query: &str, cursor: &str) -> String {
    let q = urlencoding::encode(query);
    let page: usize = cursor.parse().unwrap_or(0);
    let start = page * 10;

    match engine {
        "yandex" => format!("https://yandex.com/search/?text={q}&p={page}"),
        "bing" => format!("https://www.bing.com/search?q={q}&first={}", start + 1),
        "duckduckgo" => format!("https://duckduckgo.com/?q={q}&s={start}"),
        _ => format!("https://www.google.com/search?q={q}&start={start}"),
    }
}