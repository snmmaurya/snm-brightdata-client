// snm-brightdata-client/src/lib.rs - Simple update, no new complex modules
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
pub mod controllers;  // Add only this line

// Optional re-exports (your existing ones unchanged)
pub use extras::logger;
pub use client::BrightDataClient;
pub use config::BrightDataConfig;
pub use server::{
    AppState, BrightDataUrls, Config,
    cors_handler, handle_mcp_request, health_check,
};

// Re-export your existing metrics API (unchanged)
pub use metrics::{
    get_total_calls,
    get_service_calls, 
    get_service_metrics,
    get_call_sequence,
    get_configuration_analysis,
    generate_text_report,
    get_metrics_summary,
    export_all_metrics,
    BrightDataService,
    BrightDataCall,
    ServiceMetrics,
};

// Re-export ONLY the metrics controllers (simple addition)
pub use controllers::metrics_controller::{
    metrics_overview,
    metrics_health,
    metrics_detailed,
    metrics_export,
    metrics_dashboard,
};