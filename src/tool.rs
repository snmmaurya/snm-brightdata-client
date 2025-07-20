// src/tool.rs
use async_trait::async_trait;
use crate::error::BrightDataError;
use serde_json::Value;

#[async_trait]
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, parameters: Value) -> Result<Value, BrightDataError>;
}