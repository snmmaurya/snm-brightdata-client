// src/bin/snm_server.rs
use actix_web::{post, web, App, HttpServer, middleware::Logger, HttpRequest};
use actix_web_lab::sse::{self, Sse, Event};
use actix_web_lab::sse::Data;
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;
use std::convert::Infallible;
use std::env;
use dotenv::dotenv;

use snm_brightdata_client::server::{
    AppState, Config,
    handle_mcp_request,
    health_check,
    cors_handler,
};
use snm_brightdata_client::tool::ToolResolver;
use snm_brightdata_client::tools::{
    scrape::ScrapeMarkdown,
    search::SearchEngine,
    extract::Extractor,
    screenshot::ScreenshotTool,
};

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    tool: String,
    parameters: Value,
}

#[post("/sse")]
async fn sse_tool(
    req: HttpRequest,
    req_body: web::Json<InvokeRequest>,
    _state: web::Data<AppState> // Made optional since we're using direct tool execution
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let expected_token = std::env::var("MCP_AUTH_TOKEN").unwrap_or_default();
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(8);

    // 🔒 Auth check
    if !auth_header.starts_with("Bearer ") || !auth_header.ends_with(&expected_token) {
        let _ = tx.send(Ok(Event::Data(Data::new("Unauthorized")))).await;
        return Sse::from_stream(ReceiverStream::new(rx));
    }

    let tool_name = req_body.tool.clone();
    let parameters = req_body.parameters.clone();

    // 🔄 Spawn tool execution in background with enhanced MCP support
    tokio::spawn(async move {
        let start_time = chrono::Utc::now();
        let _ = tx.send(Ok(Event::Comment(format!("Tool '{}' execution started at {}", tool_name, start_time).into()))).await;

        // Use ToolResolver for consistent tool execution
        let resolver = ToolResolver::default();
        let result = match resolver.resolve(&tool_name) {
            Some(tool) => {
                // Send progress update
                let _ = tx.send(Ok(Event::Data(Data::new(format!("[progress] Executing {} tool...", tool_name))))).await;
                
                // Execute tool with MCP-compatible response
                match tool.execute(parameters).await {
                    Ok(tool_result) => {
                        // Send MCP-compatible content
                        for (i, content) in tool_result.content.iter().enumerate() {
                            let event_data = match content.content_type.as_str() {
                                "text" => {
                                    if tool_result.content.len() > 1 {
                                        format!("[content-{}] {}", i + 1, content.text)
                                    } else {
                                        content.text.clone()
                                    }
                                },
                                "image" => {
                                    format!("[image] {}\nImage data available (length: {} chars)", 
                                           content.text,
                                           content.data.as_ref().map(|d| d.len()).unwrap_or(0))
                                },
                                _ => {
                                    format!("[{}] {}", content.content_type, content.text)
                                }
                            };
                            let _ = tx.send(Ok(Event::Data(Data::new(event_data)))).await;
                        }
                        Ok(())
                    },
                    Err(e) => Err(format!("Tool execution error: {}", e))
                }
            },
            None => Err(format!("Tool '{}' not found", tool_name))
        };

        // Send final result
        let final_message = match result {
            Ok(_) => Event::Data(Data::new(format!("[completed] Tool '{}' executed successfully", tool_name))),
            Err(e) => Event::Data(Data::new(format!("[error] {}", e))),
        };

        let end_time = chrono::Utc::now();
        let _ = tx.send(Ok(final_message)).await;
        let _ = tx.send(Ok(Event::Comment(format!("Tool '{}' finished at {} (duration: {}ms)", 
                                                tool_name, 
                                                end_time, 
                                                (end_time - start_time).num_milliseconds()).into()))).await;
    });

    Sse::from_stream(ReceiverStream::new(rx))
}

// Enhanced direct tool invocation endpoint
#[post("/invoke")]
async fn invoke_tool(
    req: HttpRequest,
    req_body: web::Json<InvokeRequest>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let expected_token = std::env::var("MCP_AUTH_TOKEN").unwrap_or_default();
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if !auth_header.starts_with("Bearer ") || !auth_header.ends_with(&expected_token) {
        return Ok(actix_web::HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Unauthorized",
            "message": "Invalid or missing authorization token"
        })));
    }

    let tool_name = &req_body.tool;
    let parameters = req_body.parameters.clone();

    // Use ToolResolver for consistent tool execution
    let resolver = ToolResolver::default();
    match resolver.resolve(tool_name) {
        Some(tool) => {
            match tool.execute(parameters).await {
                Ok(tool_result) => {
                    // Return MCP-compatible response
                    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "tool": tool_name,
                        "content": tool_result.content,
                        "is_error": tool_result.is_error.unwrap_or(false),
                        "raw_data": tool_result.raw_value
                    })))
                },
                Err(e) => {
                    Ok(actix_web::HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "tool": tool_name,
                        "error": format!("Tool execution failed: {}", e)
                    })))
                }
            }
        },
        None => {
            Ok(actix_web::HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "tool": tool_name,
                "error": format!("Tool '{}' not found", tool_name)
            })))
        }
    }
}

// Enhanced tools list endpoint
#[actix_web::get("/tools")]
async fn list_tools() -> Result<actix_web::HttpResponse, actix_web::Error> {
    let resolver = ToolResolver::default();
    let tools = resolver.list_tools();
    
    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "tools": tools,
        "count": tools.len()
    })))
}

// Enhanced health check endpoint
#[actix_web::get("/health")]
async fn enhanced_health_check(
    state: web::Data<AppState>
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "snm-brightdata-mcp-server",
        "session_id": state.session_id,
        "uptime_seconds": (chrono::Utc::now() - state.start_time).num_seconds(),
        "version": env!("CARGO_PKG_VERSION"),
        "available_tools": ["scrape_website", "search_web", "extract_data", "take_screenshot"],
        "mcp_compatible": true
    })))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    let config = Config::from_env().expect("Missing config");
    let state = web::Data::new(AppState::new(config.clone()));
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_address = format!("0.0.0.0:{}", port);

    println!("🚀 Enhanced BrightData MCP HTTP Server running on http://{}", bind_address);
    println!("📋 Available endpoints:");
    println!("  • POST /mcp          - MCP JSON-RPC protocol");
    println!("  • POST /sse          - Server-Sent Events tool execution");
    println!("  • POST /invoke       - Direct tool invocation");
    println!("  • GET  /tools        - List available tools");
    println!("  • GET  /health       - Health check");
    println!("📚 Available tools: scrape_website, search_web, extract_data, take_screenshot");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Logger::default())
            .route("/mcp", web::post().to(handle_mcp_request))
            .route("/health", web::get().to(health_check))
            .service(enhanced_health_check)
            .service(list_tools)
            .service(invoke_tool)
            .service(sse_tool)
            .default_service(web::to(cors_handler))
    })
    .bind(&bind_address)?
    .run()
    .await
}