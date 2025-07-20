// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrightDataError {
    #[error("Request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Unexpected error: {0}")]
    Unexpected(#[from] anyhow::Error),

    #[error("Tool call failed: {0}")]
    ToolError(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}