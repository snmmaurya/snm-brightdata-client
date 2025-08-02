// src/types.rs - Complete version with existing + MCP types
use serde::{Deserialize, Serialize};
use serde_json::Value;

// EXISTING TYPES (preserved)
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

// NEW MCP PROTOCOL TYPES (added)
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    #[serde(rename = "jsonrpc")]
    pub json_rpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.to_string(),
                data,
            }),
        }
    }
}