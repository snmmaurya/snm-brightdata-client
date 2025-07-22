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
    state: web::Data<AppState>
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let expected_token = std::env::var("MCP_AUTH_TOKEN").unwrap_or_default();
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(8);

    if !auth_header.starts_with("Bearer ") || !auth_header.ends_with(&expected_token) {
        let _ = tx.send(Ok(Event::Data(Data::new("Unauthorized")))).await;
        return Sse::from_stream(ReceiverStream::new(rx));
    }

    let tool_name = req_body.tool.clone();
    let parameters = req_body.parameters.clone();
    let cloned_state = state.clone();

    tokio::spawn(async move {
        let result = match tool_name.as_str() {
            "scrape_website" => snm_brightdata_client::server::handle_scrape_website(&parameters, &cloned_state).await,
            "search_web" => snm_brightdata_client::server::handle_search_web(&parameters, &cloned_state).await,
            "extract_data" => snm_brightdata_client::server::handle_extract_placeholder(&parameters).await,
            _ => Err("Tool not found".to_string()),
        };

        let message = match result {
            Ok(content) => Event::Data(Data::new(content)),
            Err(e) => Event::Data(Data::new(format!("Error: {}", e))),
        };

        let _ = tx.send(Ok(message)).await;
    });

    Sse::from_stream(ReceiverStream::new(rx))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    let config = Config::from_env().expect("Missing config");
    let state = web::Data::new(AppState::new(config.clone()));
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_address = format!("0.0.0.0:{}", port);

    println!("🚀 BrightData MCP HTTP Server running on http://{}", bind_address);

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Logger::default())
            .route("/mcp", web::post().to(handle_mcp_request))
            .route("/health", web::get().to(health_check))
            .service(sse_tool)
            .default_service(web::to(cors_handler))
    })
    .bind(&bind_address)?
    .run()
    .await
}