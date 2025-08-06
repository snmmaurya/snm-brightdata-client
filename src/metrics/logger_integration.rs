// src/metrics/logger_integration.rs - Complete version with MCP session support
// Integration layer to work with existing snm-brightdata-client/src/extras/logger.rs

use crate::metrics::BRIGHTDATA_METRICS;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

/// Enhanced logging that works with both old and new systems with MCP session support
pub struct EnhancedLogger;

impl EnhancedLogger {
    /// Log BrightData request with both old and new loggers with MCP session context
    pub async fn log_brightdata_request_enhanced(
        execution_id: &str,
        zone: &str,
        url: &str,
        payload: Value,
        status: u16,
        response_headers: HashMap<String, String>,
        data_format: &str,
        raw_content: &str,
        filtered_content: Option<&str>,
        duration: std::time::Duration,
        mcp_session_id: Option<&str>, // New parameter for MCP session tracking
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        
        // Log to existing logger (snm-brightdata-client/src/extras/logger.rs)
        if let Err(e) = crate::extras::logger::JSON_LOGGER.log_brightdata_request(
            execution_id,
            zone,
            url,
            payload.clone(),
            status,
            response_headers.clone(),
            data_format
        ).await {
            log::warn!("Failed to log to existing logger: {}", e);
        }
        
        // Log to new metrics logger with MCP session context
        if let Err(e) = BRIGHTDATA_METRICS.log_call(
            execution_id,
            url,
            zone,
            "raw", // format
            Some(data_format),
            payload,
            status,
            response_headers,
            raw_content,
            filtered_content,
            duration.as_millis() as u64,
            None, // anthropic_request_id (can be enhanced later)
            mcp_session_id, // Pass MCP session ID
        ).await {
            log::warn!("Failed to log to metrics logger: {}", e);
        } else {
            if let Some(session_id) = mcp_session_id {
                log::info!("📊 Logged to metrics with session context: {}", session_id);
            } else {
                log::info!("📊 Logged to metrics without session context");
            }
        }
        
        Ok(())
    }
    
    /// Helper to time operations and log them with MCP session context
    pub async fn time_and_log_operation<F, R, E>(
        execution_id: &str,
        zone: &str,
        url: &str,
        payload: Value,
        data_format: &str,
        operation: F,
        mcp_session_id: Option<&str>, // New parameter
    ) -> Result<(R, String), E>
    where
        F: std::future::Future<Output = Result<(R, String, u16, HashMap<String, String>), E>>,
    {
        let start_time = Instant::now();
        
        match operation.await {
            Ok((result, content, status, headers)) => {
                let duration = start_time.elapsed();
                
                // Log the successful operation with session context
                if let Err(e) = Self::log_brightdata_request_enhanced(
                    execution_id,
                    zone,
                    url,
                    payload,
                    status,
                    headers,
                    data_format,
                    &content,
                    None, // No filtered content yet
                    duration,
                    mcp_session_id, // Pass session ID
                ).await {
                    log::warn!("Failed to log operation: {}", e);
                }
                
                Ok((result, content))
            }
            Err(e) => {
                let duration = start_time.elapsed();
                
                // Log the failed operation with session context
                let empty_headers = HashMap::new();
                if let Err(log_err) = Self::log_brightdata_request_enhanced(
                    execution_id,
                    zone,
                    url,
                    payload,
                    500, // Internal error
                    empty_headers,
                    data_format,
                    "Operation failed",
                    None,
                    duration,
                    mcp_session_id, // Pass session ID
                ).await {
                    log::warn!("Failed to log failed operation: {}", log_err);
                }
                
                Err(e)
            }
        }
    }
    
    /// Initialize new MCP session in metrics
    pub async fn initialize_mcp_session(session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("🎯 Initializing MCP session in enhanced logger: {}", session_id);
        
        BRIGHTDATA_METRICS.mark_new_session(session_id).await?;
        
        Ok(())
    }
    
    /// Get metrics for current or specific MCP session
    pub fn get_session_metrics(session_id: &str) -> serde_json::Value {
        let session_calls = BRIGHTDATA_METRICS.get_calls_for_session(session_id);
        let session_call_count = BRIGHTDATA_METRICS.get_session_call_count(session_id);
        
        let mut service_breakdown = HashMap::new();
        let mut total_duration = 0u64;
        let mut successful_calls = 0u64;
        let mut total_data_kb = 0.0f64;
        
        for call in &session_calls {
            let service_name = format!("{:?}", call.service);
            *service_breakdown.entry(service_name).or_insert(0u64) += 1;
            total_duration += call.duration_ms;
            if call.success {
                successful_calls += 1;
            }
            total_data_kb += call.filtered_data_size_kb;
        }
        
        serde_json::json!({
            "session_id": session_id,
            "total_calls": session_call_count,
            "successful_calls": successful_calls,
            "success_rate": if session_call_count > 0 { 
                successful_calls as f64 / session_call_count as f64 * 100.0 
            } else { 
                0.0 
            },
            "total_duration_ms": total_duration,
            "average_duration_ms": if session_call_count > 0 { 
                total_duration as f64 / session_call_count as f64 
            } else { 
                0.0 
            },
            "total_data_kb": total_data_kb,
            "average_data_kb": if session_call_count > 0 { 
                total_data_kb / session_call_count as f64 
            } else { 
                0.0 
            },
            "service_breakdown": service_breakdown,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Get all MCP sessions summary
    pub fn get_all_sessions_summary() -> serde_json::Value {
        let all_sessions = BRIGHTDATA_METRICS.get_all_sessions();
        let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
        
        let sessions_data: Vec<serde_json::Value> = all_sessions.iter()
            .map(|(session_id, call_count)| {
                serde_json::json!({
                    "session_id": session_id,
                    "call_count": call_count,
                    "percentage_of_total": if total_calls > 0 { 
                        *call_count as f64 / total_calls as f64 * 100.0 
                    } else { 
                        0.0 
                    }
                })
            })
            .collect();
        
        serde_json::json!({
            "total_sessions": all_sessions.len(),
            "total_calls_across_all_sessions": total_calls,
            "sessions": sessions_data,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
}