// src/rpc_client.rs - Cleaned up to use ToolResolver
use crate::types::{ToolCallRequest, ToolCallResponse};
use crate::error::BrightDataError;
use crate::tool::ToolResolver;
use serde_json::Value;

pub struct RpcClient;

impl RpcClient {
    /// Use the centralized ToolResolver instead of hardcoded tool matching
    pub async fn call_tool(tool_name: &str, parameters: Value) -> Result<Value, BrightDataError> {
        let resolver = ToolResolver::default();
        
        match resolver.resolve(tool_name) {
            Some(tool) => {
                // Use legacy method for backward compatibility
                tool.execute_legacy(parameters).await
            },
            None => Err(BrightDataError::ToolError(format!(
                "Unknown tool: {}",
                tool_name
            )))
        }
    }

    /// Get list of available tools
    pub fn list_available_tools() -> Vec<&'static str> {
        ToolResolver::default().get_available_tool_names()
    }
}