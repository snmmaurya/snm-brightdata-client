// src/bin/snm_server.rs
use actix_web::{web, App, HttpServer, HttpResponse, Result, middleware::Logger, HttpRequest};
use std::env;
use dotenv::dotenv;

use snm_brightdata_client::server::{AppState, Config, BrightDataUrls, handle_mcp_request, health_check, cors_handler};

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
            .default_service(web::to(cors_handler))
    })
    .bind(&bind_address)?
    .run()
    .await
}
