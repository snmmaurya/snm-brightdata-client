// src/lib.rs
pub mod config;
pub mod error;
pub mod types;
pub mod client;
pub mod rpc_client;
pub mod tool;
pub mod tools;
pub mod server;

// Optional re-exports from the correct module
pub use server::{
    AppState, BrightDataUrls, Config,
    cors_handler, handle_mcp_request, health_check,
};
