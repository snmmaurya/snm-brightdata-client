// src/tools/multi_zone_search.rs
use crate::tool::{Tool, ToolResult, McpContent};
use crate::tools::search::SearchEngine;
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct MultiZoneSearch;

#[async_trait]
impl Tool for MultiZoneSearch {
    fn name(&self) -> &str {
        "multi_zone_search"
    }

    fn description(&self) -> &str {
        "Performs the same search query across multiple BrightData zones in parallel"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "engine": {
                    "type": "string",
                    "enum": ["google", "bing", "yandex", "duckduckgo"],
                    "default": "google"
                },
                "zones": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of BrightData zone names to run parallel searches"
                }
            },
            "required": ["query", "zones"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing query".to_string()))?
            .to_string();

        let engine = parameters
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("google")
            .to_string();

        let zones = parameters
            .get("zones")
            .and_then(|v| v.as_array())
            .ok_or_else(|| BrightDataError::ToolError("Missing or invalid zones list".to_string()))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<String>>();

        if zones.is_empty() {
            return Err(BrightDataError::ToolError("No zones provided".into()));
        }

        let mut handles = vec![];

        for zone in zones {
            let q = query.clone();
            let e = engine.clone();

            let handle = tokio::spawn(async move {
                let tool = SearchEngine;
                let args = json!({
                    "query": q,
                    "engine": e,
                    "zone": zone
                });

                let result = tool.execute(args).await;
                (result, zone)
            });

            handles.push(handle);
        }

        let mut results = vec![];

        for handle in handles {
            match handle.await {
                Ok((Ok(tool_result), zone)) => {
                    for content in tool_result.content {
                        results.push(McpContent::text(format!("📍 [{} Zone]\n{}", zone, content.text)));
                    }
                }
                Ok((Err(err), zone)) => {
                    results.push(McpContent::text(format!("❌ [{} Zone] Failed: {}", zone, err)));
                }
                Err(join_err) => {
                    results.push(McpContent::text(format!("❌ Zone task join failed: {}", join_err)));
                }
            }
        }

        Ok(ToolResult::success(results))
    }
}
