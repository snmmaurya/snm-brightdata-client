// src/metrics/handler.rs - Fixed version with warning removed
use serde_json::{json, Value};
use crate::metrics::{BRIGHTDATA_METRICS, BrightDataService};
use std::collections::HashMap;

/// Get total call count by Anthropic
pub fn get_total_calls() -> Value {
    let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
    
    json!({
        "total_calls_by_anthropic": total_calls,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Get service-specific call counts
pub fn get_service_calls() -> Value {
    let services = vec![
        BrightDataService::Crawl,
        BrightDataService::Browse,
        BrightDataService::SERP,
        BrightDataService::WebUnlocker,
    ];
    
    let mut service_counts = HashMap::new();
    
    for service in services {
        let count = BRIGHTDATA_METRICS.get_service_call_count(&service);
        service_counts.insert(format!("{:?}", service), count);
    }
    
    json!({
        "service_call_counts": service_counts,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Get detailed service metrics
pub fn get_service_metrics() -> Value {
    let metrics = BRIGHTDATA_METRICS.get_service_metrics();
    
    json!({
        "service_metrics": metrics,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Get call sequence with detailed information
pub fn get_call_sequence() -> Value {
    let calls = BRIGHTDATA_METRICS.get_calls_by_sequence();
    
    let simplified_calls: Vec<_> = calls.iter().map(|call| {
        json!({
            "sequence": call.sequence_number,
            "service": format!("{:?}", call.service),
            "url": call.url,
            "data_format": format!("{:?}", call.data_format),
            "raw_size_kb": call.raw_data_size_kb,
            "filtered_size_kb": call.filtered_data_size_kb,
            "truncated": call.truncated,
            "truncation_reason": call.truncation_reason,
            "success": call.success,
            "duration_ms": call.duration_ms,
            "timestamp": call.timestamp.to_rfc3339(),
            "content_quality_score": call.content_quality_score,
            "contains_financial_data": call.contains_financial_data,
            "is_navigation_heavy": call.is_navigation_heavy,
            "is_error_page": call.is_error_page,
            "mcp_session_id": call.mcp_session_id
        })
    }).collect();
    
    json!({
        "call_sequence": simplified_calls,
        "total_calls": calls.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Get configuration analysis showing what's configurable
pub fn get_configuration_analysis() -> Value {
    let calls = BRIGHTDATA_METRICS.get_calls_by_sequence();
    
    let mut zone_usage = HashMap::new();
    let mut format_usage = HashMap::new();
    let mut data_format_usage = HashMap::new();
    let mut configurable_options = HashMap::new();
    
    for call in calls.iter() {
        // Count zone usage
        *zone_usage.entry(call.config.zone.clone()).or_insert(0) += 1;
        
        // Count format usage
        *format_usage.entry(call.config.format.clone()).or_insert(0) += 1;
        
        // Count data format usage
        let data_format = call.config.data_format.as_deref().unwrap_or("none");
        *data_format_usage.entry(data_format.to_string()).or_insert(0) += 1;
        
        // Analyze configurable options
        if call.config.viewport.is_some() {
            *configurable_options.entry("viewport".to_string()).or_insert(0) += 1;
        }
        if call.config.full_page.is_some() {
            *configurable_options.entry("full_page".to_string()).or_insert(0) += 1;
        }
    }
    
    json!({
        "configurable_options": {
            "zones": {
                "available": ["default", "serp_api2", "mcp_unlocker", "browser_zone"],
                "usage_count": zone_usage,
                "description": "Zone determines the service type and proxy routing"
            },
            "formats": {
                "available": ["raw", "json", "html"],
                "usage_count": format_usage,
                "description": "Response format from BrightData API"
            },
            "data_formats": {
                "available": ["markdown", "json", "html", "screenshot"],
                "usage_count": data_format_usage,
                "description": "Content processing format"
            },
            "other_options": {
                "viewport": "Configurable for screenshots (width, height)",
                "full_page": "Boolean for full page screenshots",
                "timeout": "Configurable request timeout",
                "custom_headers": "Can add custom HTTP headers",
                "usage_count": configurable_options
            }
        },
        "truncation_conditions": {
            "size_limit": "Content over 10KB is truncated",
            "navigation_filter": "Navigation/boilerplate content is removed",
            "error_page_filter": "Error pages (502, 404, etc.) are filtered out",
            "quality_threshold": "Content below quality score 40 is summarized"
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Generate comprehensive metrics report as text
pub async fn generate_text_report() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    BRIGHTDATA_METRICS.generate_metrics_report().await
}

/// Get metrics summary for quick overview
pub fn get_metrics_summary() -> Value {
    let total_calls = BRIGHTDATA_METRICS.get_total_call_count();
    let service_metrics = BRIGHTDATA_METRICS.get_service_metrics();
    let calls = BRIGHTDATA_METRICS.get_calls_by_sequence();
    
    let mut total_data_kb = 0.0;
    let mut total_filtered_kb = 0.0;
    let mut total_successful = 0;
    let mut total_tokens_estimated = 0.0;
    let mut total_duration_ms = 0;
    
    // Calculate totals
    for (_, metrics) in service_metrics.iter() {
        total_data_kb += metrics.total_data_kb;
        total_successful += metrics.successful_calls;
        total_duration_ms += metrics.total_duration_ms;
    }
    
    for call in calls.iter() {
        total_filtered_kb += call.filtered_data_size_kb;
        // Estimate tokens based on data size (rough approximation)
        total_tokens_estimated += call.raw_data_size_kb * 0.75; // ~0.75 tokens per KB
    }
    
    // Create detailed call summaries
    let mut call_summaries = Vec::new();
    for (index, call) in calls.iter().enumerate() {
        call_summaries.push(json!({
            "call_id": call.call_id,
            "sequence_number": index + 1,
            "timestamp": call.timestamp.to_rfc3339(),
            "service": format!("{:?}", call.service),
            "url": call.url,
            "query": call.query,
            "status_code": call.status_code,
            "success": call.success,
            "duration_ms": call.duration_ms,
            "data_sizes": {
                "raw_data_size_kb": call.raw_data_size_kb,
                "filtered_data_size_kb": call.filtered_data_size_kb,
                "truncated": call.truncated,
                "truncation_reason": call.truncation_reason
            },
            "estimated_tokens": call.raw_data_size_kb * 0.75,
            "brightdata_request": {
                "zone": call.config.zone,
                "format": call.config.format,
                "data_format": call.config.data_format,
                "timeout_seconds": call.config.timeout_seconds,
                "viewport": call.config.viewport,
                "custom_headers": call.config.custom_headers,
                "full_page": call.config.full_page
            },
            "response_headers": call.response_headers,
            "response_data": {
                "raw_response_data": call.raw_response_data,
                "filtered_response_data": call.filtered_response_data
            },
            "content_analysis": {
                "quality_score": call.content_quality_score,
                "contains_financial_data": call.contains_financial_data,
                "is_navigation_heavy": call.is_navigation_heavy,
                "is_error_page": call.is_error_page
            },
            "mcp_session_id": call.mcp_session_id,
            "anthropic_request_id": call.anthropic_request_id,
            "error_message": call.error_message
        }));
    }
    
    // Service breakdown
    let mut service_breakdown = Vec::new();
    for (service, metrics) in service_metrics.iter() {
        service_breakdown.push(json!({
            "service": format!("{:?}", service),
            "total_calls": metrics.total_calls,
            "successful_calls": metrics.successful_calls,
            "failed_calls": metrics.failed_calls,
            "success_rate_percent": if metrics.total_calls > 0 {
                (metrics.successful_calls as f64 / metrics.total_calls as f64) * 100.0
            } else {
                0.0
            },
            "total_data_kb": metrics.total_data_kb,
            "average_data_size_kb": metrics.average_data_size_kb,
            "total_duration_ms": metrics.total_duration_ms,
            "average_duration_ms": metrics.average_duration_ms,
            "most_used_zone": metrics.most_used_zone,
            "truncation_rate": metrics.truncation_rate,
            "unique_sessions": metrics.unique_sessions
        }));
    }
    
    json!({
        "summary": {
            "total_calls": total_calls,
            "success_rate_percent": if total_calls > 0 { 
                (total_successful as f64 / total_calls as f64) * 100.0 
            } else { 
                0.0 
            },
            "total_raw_data_kb": total_data_kb,
            "total_filtered_data_kb": total_filtered_kb,
            "data_reduction_percent": if total_data_kb > 0.0 {
                ((total_data_kb - total_filtered_kb) / total_data_kb) * 100.0
            } else {
                0.0
            },
            "total_estimated_tokens": total_tokens_estimated,
            "total_duration_ms": total_duration_ms,
            "average_duration_ms": if total_calls > 0 {
                total_duration_ms as f64 / total_calls as f64
            } else {
                0.0
            },
            "services_used": service_metrics.len(),
            "most_used_service": service_metrics.iter()
                .max_by_key(|(_, m)| m.total_calls)
                .map(|(s, _)| format!("{:?}", s))
                .unwrap_or_else(|| "None".to_string())
        },
        "service_breakdown": service_breakdown,
        "call_summaries": call_summaries,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}

/// Export all metrics data for external analysis
pub fn export_all_metrics() -> Value {
    let calls = BRIGHTDATA_METRICS.get_calls_by_sequence();
    let service_metrics = BRIGHTDATA_METRICS.get_service_metrics();
    
    json!({
        "export_timestamp": chrono::Utc::now().to_rfc3339(),
        "total_calls": calls.len(),
        "service_metrics": service_metrics,
        "detailed_calls": calls,
        "summary": get_metrics_summary()
    })
}

/// Reset all metrics (useful for testing)
#[cfg(feature = "reset-metrics")]
pub fn reset_metrics() -> Value {
    // This would require implementing a reset method in the logger
    // For now, just return a message
    json!({
        "message": "Metrics reset not implemented - restart application to reset",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })
}