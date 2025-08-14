// snm-brightdata-client/src/lib.rs - Cleaned up version
pub mod config;
pub mod error;
pub mod types;
pub mod client;
pub mod rpc_client;
pub mod tool;
pub mod tools;
pub mod server;
pub mod extras;
pub mod filters;
pub mod metrics;
pub mod symbols;
pub mod services;

// Core re-exports
pub use client::BrightDataClient;
pub use config::BrightDataConfig;
pub use server::{
    AppState, BrightDataUrls, Config,
    cors_handler, handle_mcp_request, health_check,
};

// Tool system re-exports
pub use tool::{Tool, ToolResult, McpContent, ToolResolver};

// Metrics re-exports
pub use metrics::{
    get_total_calls,
    get_service_metrics,
    get_metrics_summary,
    BrightDataService,
    BrightDataCall,
    ServiceMetrics,
};