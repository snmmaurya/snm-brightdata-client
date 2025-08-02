// src/bin/snm_server.rs - Complete version with MCP session handling and metrics endpoints
use actix_web::{post, web, App, HttpServer, middleware::Logger, HttpRequest, get};
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
use snm_brightdata_client::tool::{ToolResolver, get_current_mcp_session};
use snm_brightdata_client::metrics::{
    BRIGHTDATA_METRICS,
    get_total_calls,
    get_service_calls,
    get_service_metrics,
    get_call_sequence,
    get_configuration_analysis,
    get_metrics_summary,
    EnhancedLogger,
};

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    tool: String,
    parameters: Value,
}

#[post("/sse")]
async fn sse_tool(
    req: HttpRequest,
    req_body: web::Json<InvokeRequest>,
    _state: web::Data<AppState>
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let expected_token = std::env::var("MCP_AUTH_TOKEN").unwrap_or_default();
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(8);

    // Auth check
    if !auth_header.starts_with("Bearer ") || !auth_header.ends_with(&expected_token) {
        let _ = tx.send(Ok(Event::Data(Data::new("Unauthorized")))).await;
        return Sse::from_stream(ReceiverStream::new(rx));
    }

    let tool_name = req_body.tool.clone();
    let parameters = req_body.parameters.clone();

    // Get current MCP session for context
    let current_session = get_current_mcp_session();

    // Use centralized ToolResolver instead of manual tool matching
    tokio::spawn(async move {
        let start_time = chrono::Utc::now();
        let _ = tx.send(Ok(Event::Comment(format!("Tool '{}' execution started [Session: {:?}]", tool_name, current_session).into()))).await;

        let resolver = ToolResolver::default();
        let result = match resolver.resolve(&tool_name) {
            Some(tool) => {
                let _ = tx.send(Ok(Event::Data(Data::new(format!("[progress] Executing {} tool...", tool_name))))).await;
                
                match tool.execute(parameters).await {
                    Ok(tool_result) => {
                        // Send MCP-compatible content
                        for (i, content) in tool_result.content.iter().enumerate() {
                            let event_data = if tool_result.content.len() > 1 {
                                format!("[content-{}] {}", i + 1, content.text)
                            } else {
                                content.text.clone()
                            };
                            let _ = tx.send(Ok(Event::Data(Data::new(event_data)))).await;
                        }
                        
                        // Send session context if available
                        if let Some(session_id) = &tool_result.session_id {
                            let _ = tx.send(Ok(Event::Data(Data::new(format!("[session] {}", session_id))))).await;
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
            Ok(_) => Event::Data(Data::new(format!("[completed] Tool '{}' executed successfully [Session: {:?}]", tool_name, current_session))),
            Err(e) => Event::Data(Data::new(format!("[error] {}", e))),
        };

        let end_time = chrono::Utc::now();
        let _ = tx.send(Ok(final_message)).await;
        let _ = tx.send(Ok(Event::Comment(format!("Finished in {}ms", 
                                                (end_time - start_time).num_milliseconds()).into()))).await;
    });

    Sse::from_stream(ReceiverStream::new(rx))
}

// Simplified direct tool invocation using ToolResolver
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
            "error": "Unauthorized"
        })));
    }

    let tool_name = &req_body.tool;
    let parameters = req_body.parameters.clone();
    let current_session = get_current_mcp_session();

    let resolver = ToolResolver::default();
    match resolver.resolve(tool_name) {
        Some(tool) => {
            match tool.execute(parameters).await {
                Ok(tool_result) => {
                    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "tool": tool_name,
                        "content": tool_result.content,
                        "is_error": tool_result.is_error.unwrap_or(false),
                        "raw_data": tool_result.raw_value,
                        "session_id": tool_result.session_id,
                        "current_mcp_session": current_session
                    })))
                },
                Err(e) => {
                    Ok(actix_web::HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "tool": tool_name,
                        "error": format!("Tool execution failed: {}", e),
                        "current_mcp_session": current_session
                    })))
                }
            }
        },
        None => {
            Ok(actix_web::HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "tool": tool_name,
                "error": format!("Tool '{}' not found", tool_name),
                "available_tools": resolver.get_available_tool_names(),
                "current_mcp_session": current_session
            })))
        }
    }
}

// Simplified tools list using ToolResolver
#[get("/tools")]
async fn list_tools() -> Result<actix_web::HttpResponse, actix_web::Error> {
    let resolver = ToolResolver::default();
    let tools = resolver.list_tools();
    let current_session = get_current_mcp_session();
    
    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "tools": tools,
        "count": tools.len(),
        "current_mcp_session": current_session
    })))
}

// NEW: Metrics test endpoint with MCP session support
#[get("/metrics/test")]
async fn test_metrics() -> Result<actix_web::HttpResponse, actix_web::Error> {
    log::info!("🧪 Testing metrics system...");
    
    let current_session = get_current_mcp_session();
    
    // Test 1: Add some test data
    let mut test_results = Vec::new();
    
    for i in 1..=3 {
        let execution_id = format!("test_metrics_{}", i);
        let zone = match i {
            1 => "serp_api2",
            2 => "web_unlocker",
            3 => "browser_zone",
            _ => "default",
        };
        
        let result = BRIGHTDATA_METRICS.log_call(
            &execution_id,
            &format!("https://test{}.example.com", i),
            zone,
            "raw",
            Some("markdown"),
            serde_json::json!({
                "test": true,
                "iteration": i,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
            200,
            HashMap::new(),
            &format!("Test content for metrics iteration {}", i),
            Some(&format!("Filtered test content {}", i)),
            1000 + (i as u64 * 100),
            Some(&format!("anthropic_test_{}", i)),
            current_session.as_deref(), // Pass current MCP session
        ).await;
        
        test_results.push(serde_json::json!({
            "test_number": i,
            "zone": zone,
            "success": result.is_ok(),
            "error": result.err().map(|e| e.to_string()),
            "mcp_session": current_session.clone()
        }));
    }
    
    // Test 2: Check metrics retrieval
    let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
    let service_metrics = BRIGHTDATA_METRICS.get_service_metrics();
    let calls = BRIGHTDATA_METRICS.get_calls_by_sequence();
    let all_sessions = BRIGHTDATA_METRICS.get_all_sessions();
    
    // Test 3: Check handler functions
    let handler_results = serde_json::json!({
        "get_total_calls": get_total_calls(),
        "get_service_calls": get_service_calls(),
        "get_service_metrics_count": get_service_metrics().get("service_metrics")
            .and_then(|v| v.as_object())
            .map(|obj| obj.len())
            .unwrap_or(0),
        "get_call_sequence_count": get_call_sequence().get("call_sequence")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0),
        "get_configuration_analysis": get_configuration_analysis(),
        "get_metrics_summary": get_metrics_summary(),
    });
    
    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
        "test_timestamp": chrono::Utc::now().to_rfc3339(),
        "current_mcp_session": current_session,
        "test_summary": {
            "description": "Metrics system test results with MCP session support",
            "total_calls_in_system": total_calls,
            "services_tracked": service_metrics.len(),
            "call_records": calls.len(),
            "mcp_sessions_tracked": all_sessions.len(),
        },
        "mcp_sessions": all_sessions,
        "test_data_insertion": test_results,
        "handler_function_results": handler_results,
        "file_check": {
            "metrics_log_file": "logs/brightdata_metrics.jsonl",
            "logs_directory": "logs/",
        },
        "recommendations": if total_calls == 0 {
            vec![
                "No calls recorded - check if tools are calling BRIGHTDATA_METRICS.log_call()",
                "Verify environment variables are set",
                "Check if logs directory has write permissions",
                "Try calling MCP initialize first"
            ]
        } else {
            vec!["Metrics system appears to be working correctly with MCP session support"]
        }
    })))
}

// NEW: MCP session metrics endpoint
#[get("/metrics/session/{session_id}")]
async fn get_session_metrics(
    path: web::Path<String>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let session_id = path.into_inner();
    let session_metrics = EnhancedLogger::get_session_metrics(&session_id);
    
    Ok(actix_web::HttpResponse::Ok().json(session_metrics))
}

// NEW: All sessions overview
#[get("/metrics/sessions")]
async fn get_all_sessions() -> Result<actix_web::HttpResponse, actix_web::Error> {
    let sessions_summary = EnhancedLogger::get_all_sessions_summary();
    let current_session = get_current_mcp_session();
    
    let mut response = sessions_summary;
    response["current_active_session"] = serde_json::json!(current_session);
    
    Ok(actix_web::HttpResponse::Ok().json(response))
}

// NEW: Debug metrics endpoint with session support
#[get("/metrics/debug")]
async fn debug_metrics() -> Result<actix_web::HttpResponse, actix_web::Error> {
    use tokio::fs;
    
    log::info!("🔍 Debugging metrics system...");
    
    let current_session = get_current_mcp_session();
    
    // Check environment variables
    let env_check = serde_json::json!({
        "BRIGHTDATA_API_TOKEN": env::var("BRIGHTDATA_API_TOKEN").is_ok(),
        "API_TOKEN": env::var("API_TOKEN").is_ok(),
        "BRIGHTDATA_BASE_URL": env::var("BRIGHTDATA_BASE_URL").unwrap_or_else(|_| "default".to_string()),
        "WEB_UNLOCKER_ZONE": env::var("WEB_UNLOCKER_ZONE").unwrap_or_else(|_| "default".to_string()),
        "BRIGHTDATA_SERP_ZONE": env::var("BRIGHTDATA_SERP_ZONE").unwrap_or_else(|_| "default".to_string()),
    });
    
    // Check file system
    let logs_dir_exists = fs::metadata("logs/").await.is_ok();
    let metrics_file_exists = fs::metadata("logs/brightdata_metrics.jsonl").await.is_ok();
    let metrics_file_size = fs::metadata("logs/brightdata_metrics.jsonl").await
        .map(|m| m.len())
        .unwrap_or(0);
    
    // Check metrics system state
    let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
    let service_metrics = BRIGHTDATA_METRICS.get_service_metrics();
    let calls = BRIGHTDATA_METRICS.get_calls_by_sequence();
    let all_sessions = BRIGHTDATA_METRICS.get_all_sessions();
    
    let latest_calls = calls.iter().rev().take(5).map(|call| {
        serde_json::json!({
            "sequence": call.sequence_number,
            "call_id": call.call_id,
            "service": format!("{:?}", call.service),
            "url": call.url,
            "timestamp": call.timestamp.to_rfc3339(),
            "success": call.success,
            "duration_ms": call.duration_ms,
            "mcp_session_id": call.mcp_session_id
        })
    }).collect::<Vec<_>>();
    
    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
        "debug_timestamp": chrono::Utc::now().to_rfc3339(),
        "current_mcp_session": current_session,
        "environment_variables": env_check,
        "file_system": {
            "logs_directory_exists": logs_dir_exists,
            "metrics_file_exists": metrics_file_exists,
            "metrics_file_size_bytes": metrics_file_size,
        },
        "metrics_system_state": {
            "total_calls": total_calls,
            "services_count": service_metrics.len(),
            "call_records_count": calls.len(),
            "mcp_sessions_count": all_sessions.len(),
            "latest_calls": latest_calls,
        },
        "mcp_sessions": all_sessions,
        "service_breakdown": service_metrics.iter().map(|(service, metrics)| {
            serde_json::json!({
                "service": format!("{:?}", service),
                "total_calls": metrics.total_calls,
                "successful_calls": metrics.successful_calls,
                "failed_calls": metrics.failed_calls,
                "total_data_kb": metrics.total_data_kb,
                "unique_sessions": metrics.unique_sessions,
            })
        }).collect::<Vec<_>>(),
        "diagnosis": {
            "metrics_working": total_calls > 0,
            "files_accessible": logs_dir_exists,
            "data_being_logged": metrics_file_size > 0,
            "mcp_sessions_tracked": all_sessions.len() > 0,
            "current_session_active": current_session.is_some(),
        }
    })))
}

// Enhanced health check with MCP session info
#[get("/health")]
async fn enhanced_health_check(
    state: web::Data<AppState>
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let resolver = ToolResolver::default();
    let current_session = get_current_mcp_session();
    
    // Include metrics in health check
    let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
    let service_metrics = BRIGHTDATA_METRICS.get_service_metrics();
    let all_sessions = BRIGHTDATA_METRICS.get_all_sessions();
    
    Ok(actix_web::HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "snm-brightdata-mcp-server",
        "session_id": state.session_id,
        "uptime_seconds": (chrono::Utc::now() - state.start_time).num_seconds(),
        "version": env!("CARGO_PKG_VERSION"),
        "available_tools": resolver.get_available_tool_names(),
        "tool_count": resolver.tool_count(),
        "mcp_compatible": true,
        "current_mcp_session": current_session,
        "metrics": {
            "total_calls": total_calls,
            "services_tracked": service_metrics.len(),
            "metrics_system_active": total_calls > 0,
            "mcp_sessions_tracked": all_sessions.len(),
            "session_tracking_active": current_session.is_some(),
        }
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

    println!("🚀 SNM BrightData MCP Server running on http://{}", bind_address);
    println!("📋 Available endpoints:");
    println!("  • POST /mcp                    - MCP JSON-RPC protocol (handles initialize)");
    println!("  • POST /sse                    - Server-Sent Events tool execution");
    println!("  • POST /invoke                 - Direct tool invocation");
    println!("  • GET  /tools                  - List available tools");
    println!("  • GET  /health                 - Health check with MCP session info");
    println!("  • GET  /metrics/test           - Test metrics system");
    println!("  • GET  /metrics/debug          - Debug metrics system");
    println!("  • GET  /metrics/sessions       - All MCP sessions overview");
    println!("  • GET  /metrics/session/{{id}}  - Specific session metrics");
    
    let resolver = ToolResolver::default();
    println!("📚 Available tools ({}): {:?}", resolver.tool_count(), resolver.get_available_tool_names());
    
    // Initial metrics check
    let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
    let all_sessions = BRIGHTDATA_METRICS.get_all_sessions();
    println!("📊 Initial metrics: {} calls across {} MCP sessions", total_calls, all_sessions.len());
    
    let current_session = get_current_mcp_session();
    println!("🎯 Current MCP session: {:?}", current_session);

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
            .service(test_metrics)
            .service(debug_metrics)
            .service(get_session_metrics)
            .service(get_all_sessions)
            .default_service(web::to(cors_handler))
    })
    .bind(&bind_address)?
    .run()
    .await
}