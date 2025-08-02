// src/tools/multi_zone_search.rs - PATCHED: Enhanced with priority-aware filtering and token budget management
use crate::tool::{Tool, ToolResult, McpContent};
use crate::tools::search::SearchEngine;
use crate::error::BrightDataError;
use crate::logger::JSON_LOGGER;
use crate::filters::{ResponseFilter, ResponseStrategy, ResponseType};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use log::{info, warn};

pub struct MultiZoneSearch;

#[async_trait]
impl Tool for MultiZoneSearch {
    fn name(&self) -> &str {
        "multi_zone_search"
    }

    fn description(&self) -> &str {
        "Performs the same search query across multiple BrightData zones in parallel with intelligent filtering and priority management"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { 
                    "type": "string",
                    "description": "Search query to execute across multiple zones"
                },
                "engine": {
                    "type": "string",
                    "enum": ["google", "bing", "yandex", "duckduckgo"],
                    "default": "google",
                    "description": "Search engine to use"
                },
                "zones": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of BrightData zone names to run parallel searches"
                },
                "priority_mode": {
                    "type": "string",
                    "enum": ["balanced", "speed", "quality", "comprehensive"],
                    "default": "balanced",
                    "description": "Priority mode for zone execution"
                },
                "max_concurrent": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10,
                    "default": 5,
                    "description": "Maximum number of concurrent zone searches"
                }
            },
            "required": ["query", "zones"]
        })
    }

    // FIXED: Add the missing execute method that delegates to execute_internal
    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        self.execute_internal(parameters).await
    }

    async fn execute_internal(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
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

        let priority_mode = parameters
            .get("priority_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("balanced");

        let max_concurrent = parameters
            .get("max_concurrent")
            .and_then(|v| v.as_i64())
            .unwrap_or(5) as usize;

        if zones.is_empty() {
            return Err(BrightDataError::ToolError("No zones provided".into()));
        }

        // ENHANCED: Priority classification and token allocation
        let query_priority = ResponseStrategy::classify_query_priority(&query);
        let recommended_tokens = ResponseStrategy::get_recommended_token_allocation(&query);

        // Early validation using strategy only if TRUNCATE_FILTER is enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let response_type = ResponseStrategy::determine_response_type("", &query);
            if matches!(response_type, ResponseType::Empty) {
                return Ok(ResponseStrategy::create_response("", &query, "multi_zone_search", "validation", json!({}), response_type));
            }

            // Budget check for multi-zone searches (more expensive)
            let (_, remaining_tokens) = ResponseStrategy::get_token_budget_status();
            let required_tokens = zones.len() * 200; // Estimate tokens per zone
            if remaining_tokens < required_tokens && !matches!(query_priority, crate::filters::strategy::QueryPriority::Critical) {
                return Ok(ResponseStrategy::create_response("", &query, "multi_zone_search", "budget_limit", json!({}), ResponseType::Skip));
            }
        }

        let execution_id = format!("multizone_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f"));

        info!("🔍 Priority {} multi-zone search: '{}' across {} zones (execution: {})", 
              format!("{:?}", query_priority), query, zones.len(), execution_id);

        // ENHANCED: Priority-aware zone optimization
        let optimized_zones = self.optimize_zones_by_priority(&zones, &query_priority, priority_mode, max_concurrent);
        let zone_token_budget = recommended_tokens / optimized_zones.len().max(1);

        let mut handles = vec![];
        for (zone_idx, zone) in optimized_zones.into_iter().enumerate() {
            let q = query.clone();
            let e = engine.clone();
            let exec_id = format!("{}_{}", execution_id, zone_idx);
            let zone_priority = if zone_idx == 0 { 
                query_priority.clone() 
            } else { 
                // Lower priority for subsequent zones
                match query_priority {
                    crate::filters::strategy::QueryPriority::Critical => crate::filters::strategy::QueryPriority::High,
                    crate::filters::strategy::QueryPriority::High => crate::filters::strategy::QueryPriority::Medium,
                    _ => crate::filters::strategy::QueryPriority::Low,
                }
            };

            let handle = tokio::spawn(async move {
                let tool = SearchEngine;
                let mut args = json!({
                    "query": q,
                    "engine": e
                });

                // Add priority processing hints for filtering-enabled mode
                if std::env::var("TRUNCATE_FILTER")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false) {
                    
                    args["processing_priority"] = json!(format!("{:?}", zone_priority));
                    args["token_budget"] = json!(zone_token_budget);
                    args["execution_id"] = json!(exec_id);
                    args["zone_name"] = json!(zone);
                }

                let result = tool.execute_internal(args).await;
                (result, zone, zone_priority, exec_id)
            });
            handles.push(handle);
        }

        // ENHANCED: Collect and process results with priority awareness
        let mut results = vec![];
        let mut zone_results = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;

        for handle in handles {
            match handle.await {
                Ok((Ok(tool_result), zone, zone_priority, exec_id)) => {
                    success_count += 1;
                    
                    // Apply filtering based on environment variable
                    let processed_content = if std::env::var("TRUNCATE_FILTER")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false) {
                        
                        let mut zone_content = String::new();
                        for content in &tool_result.content {
                            zone_content.push_str(&content.text);
                            zone_content.push('\n');
                        }

                        // Apply priority-aware filtering
                        let filtered_content = if ResponseFilter::is_error_page(&zone_content) {
                            format!("❌ [{} Zone] Error page detected", zone)
                        } else if ResponseStrategy::should_try_next_source(&zone_content) {
                            format!("⚠️ [{} Zone] Low quality content", zone)
                        } else {
                            let max_tokens = zone_token_budget / 2; // Reserve tokens for formatting
                            let filtered = ResponseFilter::extract_high_value_financial_data(&zone_content, max_tokens);
                            
                            // Ultra-compact formatting for filtered mode
                            let truncated_content = if filtered.len() > 500 {
                                format!("{}...", &filtered[..497])
                            } else {
                                filtered
                            };
                            format!("📍 [{}]: {}", 
                                ResponseStrategy::ultra_abbreviate_query(&zone), 
                                truncated_content
                            )
                        };

                        filtered_content
                    } else {
                        // No filtering - standard formatting
                        let mut zone_content = format!("📍 [{} Zone] (Priority: {:?}, ID: {})\n", zone, zone_priority, exec_id);
                        for content in &tool_result.content {
                            zone_content.push_str(&content.text);
                            zone_content.push('\n');
                        }
                        zone_content
                    };

                    results.push(McpContent::text(processed_content.clone()));
                    zone_results.push(json!({
                        "zone": zone,
                        "priority": format!("{:?}", zone_priority),
                        "execution_id": exec_id,
                        "success": true,
                        "content_length": processed_content.len()
                    }));

                    // Log successful zone result
                    if let Err(e) = JSON_LOGGER.log_brightdata_request(
                        &exec_id,
                        &zone,
                        &query,
                        json!({"multi_zone": true, "priority": format!("{:?}", zone_priority)}),
                        200,
                        HashMap::new(),
                        "multi_zone_success"
                    ).await {
                        warn!("Failed to log multi-zone success: {}", e);
                    }
                }
                Ok((Err(err), zone, zone_priority, exec_id)) => {
                    error_count += 1;
                    let error_msg = if std::env::var("TRUNCATE_FILTER")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false) {
                        let truncated_err = if err.to_string().len() > 100 {
                            format!("{}...", &err.to_string()[..97])
                        } else {
                            err.to_string()
                        };
                        format!("❌ [{}]: {}", ResponseStrategy::ultra_abbreviate_query(&zone), truncated_err)
                    } else {
                        format!("❌ [{} Zone] Failed: {}", zone, err)
                    };

                    results.push(McpContent::text(error_msg));
                    zone_results.push(json!({
                        "zone": zone,
                        "priority": format!("{:?}", zone_priority),
                        "execution_id": exec_id,
                        "success": false,
                        "error": err.to_string()
                    }));

                    // Log failed zone result
                    if let Err(e) = JSON_LOGGER.log_brightdata_request(
                        &exec_id,
                        &zone,
                        &query,
                        json!({"multi_zone": true, "priority": format!("{:?}", zone_priority)}),
                        500,
                        HashMap::new(),
                        "multi_zone_error"
                    ).await {
                        warn!("Failed to log multi-zone error: {}", e);
                    }
                }
                Err(join_err) => {
                    error_count += 1;
                    let error_msg = if std::env::var("TRUNCATE_FILTER")
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false) {
                        let truncated_join_err = if join_err.to_string().len() > 100 {
                            format!("{}...", &join_err.to_string()[..97])
                        } else {
                            join_err.to_string()
                        };
                        format!("❌ Task: {}", truncated_join_err)
                    } else {
                        format!("❌ Zone task join failed: {}", join_err)
                    };
                    results.push(McpContent::text(error_msg));
                }
            }
        }

        // Add summary if not in filtered mode
        if !std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            
            let summary = format!(
                "\n🎯 **Multi-Zone Search Summary**\nQuery: {}\nEngine: {}\nPriority: {:?}\nZones: {} | Success: {} | Errors: {}\nExecution ID: {}",
                query, engine, query_priority, zones.len(), success_count, error_count, execution_id
            );
            results.insert(0, McpContent::text(summary));
        }

        // Create final result with metadata
        let final_result = ToolResult::success_with_raw(
            results,
            json!({
                "query": query,
                "engine": engine,
                "priority": format!("{:?}", query_priority),
                "token_budget": recommended_tokens,
                "priority_mode": priority_mode,
                "max_concurrent": max_concurrent,
                "execution_id": execution_id,
                "zones_executed": zone_results,
                "success_count": success_count,
                "error_count": error_count,
                "total_zones": zones.len()
            })
        );

        // Apply size limits only if filtering enabled
        if std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false) {
            Ok(ResponseStrategy::apply_size_limits(final_result))
        } else {
            Ok(final_result)
        }
    }
}

impl MultiZoneSearch {
    // ENHANCED: Priority-aware zone optimization
    fn optimize_zones_by_priority(
        &self,
        zones: &[String],
        priority: &crate::filters::strategy::QueryPriority,
        priority_mode: &str,
        max_concurrent: usize,
    ) -> Vec<String> {
        let mut optimized_zones = zones.to_vec();

        // Limit zones based on priority and mode
        let zone_limit = match (priority, priority_mode) {
            (crate::filters::strategy::QueryPriority::Critical, "comprehensive") => max_concurrent,
            (crate::filters::strategy::QueryPriority::Critical, _) => std::cmp::min(zones.len(), max_concurrent),
            (crate::filters::strategy::QueryPriority::High, "comprehensive") => std::cmp::min(zones.len(), max_concurrent),
            (crate::filters::strategy::QueryPriority::High, _) => std::cmp::min(zones.len(), max_concurrent.saturating_sub(1)),
            (crate::filters::strategy::QueryPriority::Medium, "speed") => std::cmp::min(zones.len(), 2),
            (crate::filters::strategy::QueryPriority::Medium, _) => std::cmp::min(zones.len(), 3),
            (crate::filters::strategy::QueryPriority::Low, _) => std::cmp::min(zones.len(), 2),
        };

        // Prioritize zones based on naming patterns and reliability
        optimized_zones.sort_by(|a, b| {
            let score_a = self.calculate_zone_priority_score(a);
            let score_b = self.calculate_zone_priority_score(b);
            score_b.cmp(&score_a) // Higher scores first
        });

        optimized_zones.truncate(zone_limit);
        optimized_zones
    }

    fn calculate_zone_priority_score(&self, zone: &str) -> i32 {
        let zone_lower = zone.to_lowercase();
        let mut score = 0;

        // Prefer specific API zones
        if zone_lower.contains("serp") || zone_lower.contains("search") {
            score += 10;
        }
        if zone_lower.contains("api") {
            score += 8;
        }
        if zone_lower.contains("premium") || zone_lower.contains("pro") {
            score += 6;
        }
        if zone_lower.contains("fast") || zone_lower.contains("speed") {
            score += 4;
        }

        // Penalize likely slower zones
        if zone_lower.contains("browser") || zone_lower.contains("render") {
            score -= 3;
        }
        if zone_lower.contains("slow") || zone_lower.contains("basic") {
            score -= 5;
        }

        score
    }
}