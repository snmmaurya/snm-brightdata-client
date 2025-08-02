// src/metrics/integration_layer.rs - Integration without modifying existing functions
use crate::metrics::enhanced_tracker::{
    ENHANCED_TRACKER, EnhancedCallRecord, BrightDataService, DataFormatType, 
    CallConfiguration, TruncationRules
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Integration wrapper that captures metrics without modifying existing tool functions
pub struct MetricsIntegration;

impl MetricsIntegration {
    /// Wrap any BrightData call with enhanced metrics collection
    pub async fn capture_brightdata_call<F, Fut, T>(
        anthropic_request_id: Option<&str>,
        execution_id: &str,
        zone: &str,
        target_url: &str,
        data_format: &str,
        request_payload: Value,
        custom_config: Option<CallConfiguration>,
        operation: F,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(T, String, u16, HashMap<String, String>), Box<dyn std::error::Error + Send + Sync>>>,
    {
        let start_time = Instant::now();
        
        // Create default configuration if not provided
        let config = custom_config.unwrap_or_else(|| CallConfiguration {
            zone: zone.to_string(),
            data_format: DataFormatType::from(data_format),
            custom_headers: None,
            viewport: None,
            full_page: None,
            timeout_ms: 90000,
            max_response_size_kb: Some(10),
            truncation_enabled: true,
            truncation_rules: TruncationRules::default(),
        });
        
        // Execute the operation
        let result = operation().await;
        let duration = start_time.elapsed();
        
        match result {
            Ok((response, raw_content, status_code, headers)) => {
                // Record successful call
                if let Err(e) = ENHANCED_TRACKER.record_call(
                    anthropic_request_id,
                    execution_id,
                    BrightDataService::from(zone),
                    zone,
                    data_format,
                    target_url,
                    request_payload,
                    config,
                    status_code,
                    headers,
                    &raw_content,
                    None, // processed_response - let the tracker handle it
                    duration.as_millis() as u64,
                    true,
                    None,
                ).await {
                    log::warn!("Failed to record metrics: {}", e);
                }
                
                Ok(response)
            }
            Err(error) => {
                // Record failed call
                if let Err(e) = ENHANCED_TRACKER.record_call(
                    anthropic_request_id,
                    execution_id,
                    BrightDataService::from(zone),
                    zone,
                    data_format,
                    target_url,
                    request_payload,
                    config,
                    500, // Error status
                    HashMap::new(),
                    &format!("Error: {}", error),
                    None,
                    duration.as_millis() as u64,
                    false,
                    Some(&error.to_string()),
                ).await {
                    log::warn!("Failed to record error metrics: {}", e);
                }
                
                Err(error)
            }
        }
    }
    
    /// Create a configuration for a specific tool/use case
    pub fn create_config(
        zone: &str,
        data_format: &str,
        max_size_kb: Option<u64>,
        preserve_financial: bool,
        custom_truncation_rules: Option<Vec<String>>,
    ) -> CallConfiguration {
        CallConfiguration {
            zone: zone.to_string(),
            data_format: DataFormatType::from(data_format),
            custom_headers: None,
            viewport: None,
            full_page: None,
            timeout_ms: 90000,
            max_response_size_kb: max_size_kb,
            truncation_enabled: true,
            truncation_rules: TruncationRules {
                max_size_kb: max_size_kb.unwrap_or(10),
                preserve_financial_data: preserve_financial,
                remove_navigation: true,
                quality_threshold: 40,
                custom_rules: custom_truncation_rules.unwrap_or_default(),
            },
        }
    }
    
    /// Helper to extract BrightData call details from existing tool responses
    pub fn extract_call_details(
        result: &Value,
        execution_id: &str,
    ) -> Option<(String, String, String)> {
        let url = result.get("url")
            .or_else(|| result.get("search_url"))
            .or_else(|| result.get("target_url"))
            .and_then(|v| v.as_str())?;
            
        let zone = result.get("zone")
            .or_else(|| result.get("zone_used"))
            .and_then(|v| v.as_str())?;
            
        let format = result.get("format")
            .or_else(|| result.get("data_format"))
            .and_then(|v| v.as_str())
            .unwrap_or("raw");
            
        Some((url.to_string(), zone.to_string(), format.to_string()))
    }
}

/// Decorator for existing tool functions to add metrics without modification
#[macro_export]
macro_rules! with_enhanced_metrics {
    ($anthropic_id:expr, $execution_id:expr, $zone:expr, $url:expr, $format:expr, $payload:expr, $operation:expr) => {
        crate::metrics::integration_layer::MetricsIntegration::capture_brightdata_call(
            $anthropic_id,
            $execution_id,
            $zone,
            $url,
            $format,
            $payload,
            None,
            || async { $operation },
        ).await
    };
    
    ($anthropic_id:expr, $execution_id:expr, $zone:expr, $url:expr, $format:expr, $payload:expr, $config:expr, $operation:expr) => {
        crate::metrics::integration_layer::MetricsIntegration::capture_brightdata_call(
            $anthropic_id,
            $execution_id,
            $zone,
            $url,
            $format,
            $payload,
            Some($config),
            || async { $operation },
        ).await
    };
}

/// Post-processing metrics capture for existing successful calls
pub async fn capture_existing_call_metrics(
    anthropic_request_id: Option<&str>,
    execution_id: &str,
    result: &Value,
    duration_ms: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Extract details from existing result format
    if let Some((url, zone, format)) = MetricsIntegration::extract_call_details(result, execution_id) {
        let content = result.get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
            
        let success = result.get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
            
        // Create basic configuration
        let config = MetricsIntegration::create_config(&zone, &format, Some(10), true, None);
        
        // Record the call
        ENHANCED_TRACKER.record_call(
            anthropic_request_id,
            execution_id,
            BrightDataService::from(&zone),
            &zone,
            &format,
            &url,
            result.clone(),
            config,
            if success { 200 } else { 500 },
            HashMap::new(),
            content,
            None,
            duration_ms,
            success,
            None,
        ).await?;
    }
    
    Ok(())
}

// API endpoints for your requirements
pub mod api {
    use super::*;
    use serde_json::json;
    
    /// Get number of times called by Anthropic
    pub fn get_anthropic_calls() -> Value {
        json!({
            "total_calls_by_anthropic": ENHANCED_TRACKER.get_anthropic_call_count(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Get service breakdown (Crawl, Browse, SERP)
    pub fn get_service_breakdown() -> Value {
        json!({
            "service_calls": ENHANCED_TRACKER.get_service_call_counts(),
            "data_sizes_per_service_kb": ENHANCED_TRACKER.get_data_sizes_per_service(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Get data format usage
    pub fn get_data_formats() -> Value {
        json!({
            "data_formats_used": ENHANCED_TRACKER.get_data_formats_used(),
            "description": "Raw/JSON/Markdown formats parsed and tracked",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Get data sizes per call/service/ping
    pub fn get_data_sizes() -> Value {
        let sequence = ENHANCED_TRACKER.get_call_sequence();
        let per_call: Vec<_> = sequence.iter().map(|call| {
            json!({
                "call_id": call.call_id,
                "sequence": call.sequence_number,
                "service": format!("{:?}", call.service),
                "raw_size_kb": call.raw_size_kb,
                "processed_size_kb": call.processed_size_kb,
                "final_size_kb": call.final_size_kb,
                "truncated": call.was_truncated
            })
        }).collect();
        
        json!({
            "data_sizes_per_call": per_call,
            "data_sizes_per_service": ENHANCED_TRACKER.get_data_sizes_per_service(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Get sequence of calling
    pub fn get_call_sequence() -> Value {
        let sequence = ENHANCED_TRACKER.get_call_sequence();
        let sequence_data: Vec<_> = sequence.iter().map(|call| {
            json!({
                "sequence_number": call.sequence_number,
                "call_id": call.call_id,
                "timestamp": call.timestamp.to_rfc3339(),
                "service": format!("{:?}", call.service),
                "zone": call.zone_used,
                "url": call.target_url,
                "data_format": format!("{:?}", call.data_format),
                "size_kb": call.final_size_kb,
                "duration_ms": call.duration_ms,
                "success": call.success
            })
        }).collect();
        
        json!({
            "call_sequence": sequence_data,
            "total_calls": sequence.len(),
            "sequence_info": "Calls are numbered in order of execution",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Get configurable options
    pub fn get_configurable_options() -> Value {
        ENHANCED_TRACKER.get_configurable_options_summary()
    }
    
    /// Get truncation information
    pub fn get_truncation_info() -> Value {
        let sequence = ENHANCED_TRACKER.get_call_sequence();
        let truncated_calls: Vec<_> = sequence.iter()
            .filter(|call| call.was_truncated)
            .map(|call| {
                json!({
                    "call_id": call.call_id,
                    "sequence": call.sequence_number,
                    "original_size_kb": call.raw_size_kb,
                    "final_size_kb": call.final_size_kb,
                    "truncation_reason": call.truncation_reason,
                    "truncation_method": call.truncation_method,
                    "quality_score": call.content_quality_score
                })
            }).collect();
            
        json!({
            "truncation_summary": {
                "total_calls": sequence.len(),
                "truncated_calls": truncated_calls.len(),
                "truncation_rate": truncated_calls.len() as f64 / sequence.len().max(1) as f64 * 100.0,
                "average_size_reduction": {
                    "original_avg": sequence.iter().map(|c| c.raw_size_kb).sum::<f64>() / sequence.len().max(1) as f64,
                    "final_avg": sequence.iter().map(|c| c.final_size_kb).sum::<f64>() / sequence.len().max(1) as f64
                }
            },
            "truncated_calls": truncated_calls,
            "truncation_conditions": [
                "Size exceeds configured limit (default 10KB)",
                "Quality score below threshold (default 40)",
                "High navigation content ratio",
                "Custom truncation rules applied"
            ],
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
    
    /// Export all metrics for external analysis
    pub fn export_all_metrics() -> Value {
        let sequence = ENHANCED_TRACKER.get_call_sequence();
        
        json!({
            "export_info": {
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "total_records": sequence.len(),
                "anthropic_calls": ENHANCED_TRACKER.get_anthropic_call_count()
            },
            "service_breakdown": get_service_breakdown(),
            "data_formats": get_data_formats(),
            "data_sizes": get_data_sizes(),
            "call_sequence": get_call_sequence(),
            "configurable_options": get_configurable_options(),
            "truncation_info": get_truncation_info(),
            "full_call_records": sequence
        })
    }
}