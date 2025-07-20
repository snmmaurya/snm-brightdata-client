// src/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyResponse {
    pub status: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<ToolError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolError {
    pub code: i64,
    pub message: String,
}