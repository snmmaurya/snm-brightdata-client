// src/server.rs
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

#[derive(Debug, Clone)]
pub struct Config {
    pub api_token: String,
    pub web_unlocker_zone: String,
    pub browser_zone: String,
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
            rate_limit: env::var("RATE_LIMIT").ok(),
            timeout: Duration::from_secs(env::var("REQUEST_TIMEOUT").unwrap_or_else(|_| "60".to_string()).parse().unwrap_or(60)),
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
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            config: config.clone(),
            http_client: Client::builder().timeout(config.timeout).build().unwrap(),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            start_time: Utc::now(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

pub struct BrightDataUrls;

impl BrightDataUrls {
    pub const REQUEST_API: &'static str = "https://api.brightdata.com/request";
}

pub async fn handle_mcp_request(
    _req: HttpRequest,
    payload: web::Json<McpRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let req = payload.into_inner();
    let id = req.id.clone();

    // Match returns Result<McpResponse, String>
    let mcp_result: Result<McpResponse, String> = match req.method.as_str() {
        "tools/list" => Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "tools": [
                    { "name": "scrape_website", "description": "Scrape a web page" },
                    { "name": "search_web", "description": "Perform a web search" },
                    { "name": "extract_data", "description": "Extract structured data from a webpage (WIP)" }
                ]
            })),
            error: None,
        }),

        "tools/call" => {
            if let Some(params) = req.params {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_default();

                if !check_rate_limit(name, &state).await {
                    return Ok(HttpResponse::TooManyRequests().json(McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(McpError {
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
                    _ => Err("Unknown tool".to_string()),
                };

                Ok(match result {
                    Ok(content) => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::json!({ "content": content })),
                        error: None,
                    },
                    Err(msg) => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(McpError {
                            code: -32603,
                            message: msg,
                            data: None,
                        }),
                    },
                })
            } else {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(McpError {
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
            error: Some(McpError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        }),
    };

    // Wrap the unified result into an HTTP response
    match mcp_result {
        Ok(resp) => Ok(HttpResponse::Ok().json(resp)),
        Err(e) => Ok(HttpResponse::InternalServerError().json(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(McpError {
                code: -32603,
                message: e,
                data: None,
            }),
        })),
    }
}


pub async fn health_check(state: web::Data<AppState>) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "session_id": state.session_id,
        "uptime_seconds": (Utc::now() - state.start_time).num_seconds(),
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






async fn handle_scrape_website(args: &serde_json::Value, state: &web::Data<AppState>) -> Result<String, String> {
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

    let res = state.http_client
        .post(BrightDataUrls::REQUEST_API)
        .header("Authorization", format!("Bearer {}", state.config.api_token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body = res.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

async fn handle_search_web(args: &serde_json::Value, state: &web::Data<AppState>) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing 'query'")?;
    let engine = args.get("engine").and_then(|v| v.as_str()).unwrap_or("google");
    let cursor = args.get("cursor").and_then(|v| v.as_str()).unwrap_or("0");

    let search_url = build_search_url(engine, query, cursor);

    let payload = serde_json::json!({
        "url": search_url,
        "zone": state.config.web_unlocker_zone,
        "format": "raw",
        "data_format": "markdown"
    });

    let res = state.http_client
        .post(BrightDataUrls::REQUEST_API)
        .header("Authorization", format!("Bearer {}", state.config.api_token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body = res.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

async fn handle_extract_placeholder(_args: &serde_json::Value) -> Result<String, String> {
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
