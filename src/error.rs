// src/error.rs - Enhanced version with additional error types
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrightDataError {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Unexpected error: {0}")]
    Unexpected(#[from] anyhow::Error),

    #[error("Tool call failed: {0}")]
    ToolError(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Zone not found: {0}")]
    ZoneNotFound(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}