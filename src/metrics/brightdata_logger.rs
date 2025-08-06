// src/metrics/brightdata_logger.rs - Complete fixed version with proper path handling
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use chrono::{DateTime, Utc};

/// BrightData service types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BrightDataService {
    Crawl,
    Browse, 
    SERP,
    WebUnlocker,
    McpSession,
}

impl From<&str> for BrightDataService {
    fn from(zone: &str) -> Self {
        match zone.to_lowercase().as_str() {
            z if z.contains("serp") => BrightDataService::SERP,
            z if z.contains("crawl") => BrightDataService::Crawl,
            z if z.contains("browser") => BrightDataService::Browse,
            z if z.contains("mcp_session") => BrightDataService::McpSession,
            _ => BrightDataService::WebUnlocker,
        }
    }
}

/// Data formats supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFormat {
    Raw,
    Markdown,
    JSON,
    HTML,
    Screenshot,
    SessionEvent,
    Unknown(String),
}

impl From<&str> for DataFormat {
    fn from(format: &str) -> Self {
        match format.to_lowercase().as_str() {
            "raw" => DataFormat::Raw,
            "markdown" => DataFormat::Markdown,
            "json" => DataFormat::JSON,
            "html" => DataFormat::HTML,
            "screenshot" => DataFormat::Screenshot,
            "session_event" => DataFormat::SessionEvent,
            "session_start" => DataFormat::SessionEvent,
            _ => DataFormat::Unknown(format.to_string()),
        }
    }
}

/// Configuration used for BrightData calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightDataConfig {
    pub zone: String,
    pub format: String,
    pub data_format: Option<String>,
    pub timeout_seconds: u64,
    pub viewport: Option<Value>,
    pub custom_headers: Option<HashMap<String, String>>,
    pub full_page: Option<bool>,
}

/// Individual call metrics with MCP session support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightDataCall {
    pub call_id: String,
    pub timestamp: DateTime<Utc>,
    pub service: BrightDataService,
    pub sequence_number: u64,
    pub anthropic_request_id: Option<String>,
    pub mcp_session_id: Option<String>,
    
    // Request details
    pub url: String,
    pub query: Option<String>,
    pub config: BrightDataConfig,
    
    // Response details
    pub status_code: u16,
    pub response_headers: HashMap<String, String>,
    pub data_format: DataFormat,
    pub raw_data_size_kb: f64,
    pub filtered_data_size_kb: f64,
    pub truncated: bool,
    pub truncation_reason: Option<String>,
    
    // Response data content
    pub raw_response_data: Option<String>,
    pub filtered_response_data: Option<String>,
    
    // Performance
    pub duration_ms: u64,
    pub success: bool,
    pub error_message: Option<String>,
    
    // Content analysis
    pub content_quality_score: u8,
    pub contains_financial_data: bool,
    pub is_navigation_heavy: bool,
    pub is_error_page: bool,
}

/// Aggregated metrics per service with session support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetrics {
    pub service: BrightDataService,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub total_data_kb: f64,
    pub average_data_size_kb: f64,
    pub total_duration_ms: u64,
    pub average_duration_ms: f64,
    pub most_used_format: DataFormat,
    pub most_used_zone: String,
    pub truncation_rate: f64,
    pub unique_sessions: u64,
}

/// Main metrics logger with MCP session support and FIXED path handling
pub struct BrightDataMetricsLogger {
    call_counter: AtomicU64,
    calls: Arc<Mutex<Vec<BrightDataCall>>>,
    service_counters: Arc<Mutex<HashMap<BrightDataService, AtomicU64>>>,
    session_counters: Arc<Mutex<HashMap<String, AtomicU64>>>,
    log_file_path: String,
}

impl BrightDataMetricsLogger {
    pub fn new(log_file_path: &str) -> Self {
        // FIXED: Ensure proper file path, not directory path
        let path = std::path::Path::new(log_file_path);
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("Warning: Failed to create log directory {:?}: {}", parent, e);
            }
        }
        
        // FIXED: Ensure the path points to a file, not a directory
        let final_path = if path.is_dir() || log_file_path.ends_with('/') || log_file_path.ends_with('\\') {
            // If it's a directory, append default filename
            let dir_path = if log_file_path.ends_with('/') || log_file_path.ends_with('\\') {
                log_file_path.trim_end_matches('/').trim_end_matches('\\')
            } else {
                log_file_path
            };
            format!("{}/brightdata_metrics.jsonl", dir_path)
        } else if !log_file_path.contains('.') {
            // If no extension, add .jsonl
            format!("{}.jsonl", log_file_path)
        } else {
            // Already a proper file path
            log_file_path.to_string()
        };
        
        println!("📊 BrightData metrics logger initialized with file: {}", final_path);
        
        Self {
            call_counter: AtomicU64::new(0),
            calls: Arc::new(Mutex::new(Vec::new())),
            service_counters: Arc::new(Mutex::new(HashMap::new())),
            session_counters: Arc::new(Mutex::new(HashMap::new())),
            log_file_path: final_path,
        }
    }
    
    /// Log a BrightData call with full metrics and MCP session support
    pub async fn log_call(
        &self,
        execution_id: &str,
        url: &str,
        zone: &str,
        format: &str,
        data_format: Option<&str>,
        payload: Value,
        status_code: u16,
        response_headers: HashMap<String, String>,
        raw_content: &str,
        filtered_content: Option<&str>,
        duration_ms: u64,
        anthropic_request_id: Option<&str>,
        mcp_session_id: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sequence = self.call_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let service = BrightDataService::from(zone);
        
        // Update session counter if session_id provided
        if let Some(session_id) = mcp_session_id {
            let mut session_counters = self.session_counters.lock().unwrap();
            session_counters.entry(session_id.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::SeqCst);
        }
        
        // Analyze content safely
        let content_quality_score = if raw_content.len() > 0 {
            crate::filters::ResponseFilter::get_content_quality_score(raw_content)
        } else {
            0
        };
        
        let contains_financial_data = if raw_content.len() > 0 {
            crate::filters::ResponseFilter::contains_financial_data(raw_content)
        } else {
            false
        };
        
        let is_navigation_heavy = if raw_content.len() > 0 {
            crate::filters::ResponseFilter::is_mostly_navigation(raw_content)
        } else {
            false
        };
        
        let is_error_page = if raw_content.len() > 0 {
            crate::filters::ResponseFilter::is_error_page(raw_content)
        } else {
            status_code >= 400
        };
        
        // Calculate sizes
        let raw_data_size_kb = raw_content.len() as f64 / 1024.0;
        let filtered_data_size_kb = filtered_content
            .map(|c| c.len() as f64 / 1024.0)
            .unwrap_or(raw_data_size_kb);
        
        // Determine truncation
        let (truncated, truncation_reason) = if let Some(filtered) = filtered_content {
            if filtered.len() < raw_content.len() {
                let reason = if filtered.contains("...") {
                    Some("Size limit exceeded".to_string())
                } else if filtered.len() < raw_content.len() / 2 {
                    Some("Navigation/boilerplate removed".to_string())
                } else {
                    Some("Content filtered".to_string())
                };
                (true, reason)
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };
        
        // Extract config from payload
        let config = BrightDataConfig {
            zone: zone.to_string(),
            format: format.to_string(),
            data_format: data_format.map(|s| s.to_string()),
            timeout_seconds: 90,
            viewport: payload.get("viewport").cloned(),
            custom_headers: None,
            full_page: payload.get("full_page").and_then(|v| v.as_bool()),
        };
        
        let call = BrightDataCall {
            call_id: execution_id.to_string(),
            timestamp: Utc::now(),
            service: service.clone(),
            sequence_number: sequence,
            anthropic_request_id: anthropic_request_id.map(|s| s.to_string()),
            mcp_session_id: mcp_session_id.map(|s| s.to_string()),
            
            url: url.to_string(),
            query: payload.get("query").and_then(|v| v.as_str()).map(|s| s.to_string())
                .or_else(|| {
                    if url.contains("search?") || url.contains("quote/") {
                        url.split('/').last().map(|s| s.to_string())
                    } else {
                        None
                    }
                }),
            config,
            
            status_code,
            response_headers,
            data_format: DataFormat::from(data_format.unwrap_or(format)),
            raw_data_size_kb,
            filtered_data_size_kb,
            truncated,
            truncation_reason,
            
            raw_response_data: Some(raw_content.to_string()),
            filtered_response_data: filtered_content.map(|s| s.to_string()),
            
            duration_ms,
            success: status_code >= 200 && status_code < 400,
            error_message: if status_code >= 400 {
                Some(format!("HTTP {}", status_code))
            } else {
                None
            },
            
            content_quality_score,
            contains_financial_data,
            is_navigation_heavy,
            is_error_page,
        };
        
        // Store call
        {
            let mut calls = self.calls.lock().unwrap();
            calls.push(call.clone());
        }
        
        // Update service counter
        {
            let mut counters = self.service_counters.lock().unwrap();
            counters.entry(service).or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::SeqCst);
        }
        
        // FIXED: Write to log file with proper error handling
        if let Err(e) = self.write_call_to_log(&call).await {
            eprintln!("Warning: Failed to write to metrics log file '{}': {}", self.log_file_path, e);
            // Don't return error, just log warning
        }
        
        Ok(())
    }
    
    /// Get call count for a specific service
    pub fn get_service_call_count(&self, service: &BrightDataService) -> u64 {
        let counters = self.service_counters.lock().unwrap();
        counters.get(service)
            .map(|counter| counter.load(Ordering::SeqCst))
            .unwrap_or(0)
    }
    
    /// Get total call count
    pub fn get_total_call_count(&self) -> u64 {
        self.call_counter.load(Ordering::SeqCst)
    }
    
    /// Get call count for a specific MCP session
    pub fn get_session_call_count(&self, session_id: &str) -> u64 {
        let session_counters = self.session_counters.lock().unwrap();
        session_counters.get(session_id)
            .map(|counter| counter.load(Ordering::SeqCst))
            .unwrap_or(0)
    }
    
    /// Get all session IDs and their call counts
    pub fn get_all_sessions(&self) -> HashMap<String, u64> {
        let session_counters = self.session_counters.lock().unwrap();
        session_counters.iter()
            .map(|(session_id, counter)| (session_id.clone(), counter.load(Ordering::SeqCst)))
            .collect()
    }
    
    /// Get calls for a specific MCP session
    pub fn get_calls_for_session(&self, session_id: &str) -> Vec<BrightDataCall> {
        let calls = self.calls.lock().unwrap();
        calls.iter()
            .filter(|call| call.mcp_session_id.as_deref() == Some(session_id))
            .cloned()
            .collect()
    }
    
    /// Get service metrics with session information
    pub fn get_service_metrics(&self) -> HashMap<BrightDataService, ServiceMetrics> {
        let calls = self.calls.lock().unwrap();
        let mut metrics: HashMap<BrightDataService, ServiceMetrics> = HashMap::new();
        
        for call in calls.iter() {
            let entry = metrics.entry(call.service.clone()).or_insert_with(|| ServiceMetrics {
                service: call.service.clone(),
                total_calls: 0,
                successful_calls: 0,
                failed_calls: 0,
                total_data_kb: 0.0,
                average_data_size_kb: 0.0,
                total_duration_ms: 0,
                average_duration_ms: 0.0,
                most_used_format: DataFormat::Raw,
                most_used_zone: String::new(),
                truncation_rate: 0.0,
                unique_sessions: 0,
            });
            
            entry.total_calls += 1;
            if call.success {
                entry.successful_calls += 1;
            } else {
                entry.failed_calls += 1;
            }
            entry.total_data_kb += call.filtered_data_size_kb;
            entry.total_duration_ms += call.duration_ms;
        }
        
        // Calculate averages and most used values
        for (service, metric) in metrics.iter_mut() {
            if metric.total_calls > 0 {
                metric.average_data_size_kb = metric.total_data_kb / metric.total_calls as f64;
                metric.average_duration_ms = metric.total_duration_ms as f64 / metric.total_calls as f64;
                
                let service_calls: Vec<_> = calls.iter()
                    .filter(|c| &c.service == service)
                    .collect();
                
                let mut format_counts: HashMap<String, u32> = HashMap::new();
                let mut zone_counts: HashMap<String, u32> = HashMap::new();
                let mut truncated_count = 0;
                let mut unique_sessions = std::collections::HashSet::new();
                
                for call in service_calls.iter() {
                    *format_counts.entry(format!("{:?}", call.data_format)).or_insert(0) += 1;
                    *zone_counts.entry(call.config.zone.clone()).or_insert(0) += 1;
                    if call.truncated {
                        truncated_count += 1;
                    }
                    if let Some(session_id) = &call.mcp_session_id {
                        unique_sessions.insert(session_id.clone());
                    }
                }
                
                if let Some((most_format, _)) = format_counts.iter().max_by_key(|(_, &count)| count) {
                    metric.most_used_format = DataFormat::Unknown(most_format.clone());
                }
                
                if let Some((most_zone, _)) = zone_counts.iter().max_by_key(|(_, &count)| count) {
                    metric.most_used_zone = most_zone.clone();
                }
                
                metric.truncation_rate = (truncated_count as f64 / service_calls.len() as f64) * 100.0;
                metric.unique_sessions = unique_sessions.len() as u64;
            }
        }
        
        metrics
    }
    
    /// Get calls in sequence order
    pub fn get_calls_by_sequence(&self) -> Vec<BrightDataCall> {
        let mut calls = self.calls.lock().unwrap().clone();
        calls.sort_by_key(|c| c.sequence_number);
        calls
    }
    
    /// Mark new MCP session
    pub async fn mark_new_session(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("📊 Marking new MCP session: {}", session_id);
        
        {
            let mut session_counters = self.session_counters.lock().unwrap();
            session_counters.insert(session_id.to_string(), AtomicU64::new(0));
        }
        
        self.log_call(
            &format!("session_marker_{}", session_id),
            &format!("mcp://session/{}", session_id),
            "mcp_session",
            "json",
            Some("session_start"),
            serde_json::json!({
                "event": "session_start",
                "session_id": session_id,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
            200,
            HashMap::new(),
            &format!("MCP session {} started", session_id),
            None,
            0,
            None,
            Some(session_id),
        ).await?;
        
        Ok(())
    }
    
    /// FIXED: Write individual call to log file with proper error handling
    async fn write_call_to_log(&self, call: &BrightDataCall) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(&self.log_file_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .await?;
        
        let log_entry = serde_json::to_string(call)?;
        file.write_all(format!("{}\n", log_entry).as_bytes()).await?;
        file.flush().await?;
        
        Ok(())
    }
    
    /// Generate comprehensive metrics report
    pub async fn generate_metrics_report(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let service_metrics = self.get_service_metrics();
        let total_calls = self.get_total_call_count();
        let calls_by_sequence = self.get_calls_by_sequence();
        let all_sessions = self.get_all_sessions();
        
        let mut report = String::new();
        report.push_str("# BrightData Metrics Report\n\n");
        report.push_str(&format!("Generated: {}\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        report.push_str(&format!("Total Calls: {}\n", total_calls));
        report.push_str(&format!("Total MCP Sessions: {}\n\n", all_sessions.len()));
        
        if !all_sessions.is_empty() {
            report.push_str("## MCP Session Breakdown\n\n");
            for (session_id, call_count) in all_sessions.iter() {
                report.push_str(&format!("- Session {}: {} calls\n", session_id, call_count));
            }
            report.push_str("\n");
        }
        
        report.push_str("## Service Breakdown\n\n");
        for (service, metrics) in service_metrics.iter() {
            report.push_str(&format!("### {:?}\n", service));
            report.push_str(&format!("- Total Calls: {}\n", metrics.total_calls));
            report.push_str(&format!("- Success Rate: {:.1}%\n", 
                (metrics.successful_calls as f64 / metrics.total_calls as f64) * 100.0));
            report.push_str(&format!("- Average Data Size: {:.2} KB\n", metrics.average_data_size_kb));
            report.push_str(&format!("- Average Duration: {:.1} ms\n", metrics.average_duration_ms));
            report.push_str(&format!("- Most Used Zone: {}\n", metrics.most_used_zone));
            report.push_str(&format!("- Truncation Rate: {:.1}%\n", metrics.truncation_rate));
            report.push_str(&format!("- Unique Sessions: {}\n", metrics.unique_sessions));
            report.push_str("\n");
        }
        
        report.push_str("## Call Sequence\n\n");
        for (i, call) in calls_by_sequence.iter().enumerate() {
            let session_info = call.mcp_session_id.as_deref().unwrap_or("no-session");
            report.push_str(&format!("{}. [{:?}] {} - {:.2} KB -> {:.2} KB ({}) [Session: {}]\n",
                i + 1,
                call.service,
                call.url,
                call.raw_data_size_kb,
                call.filtered_data_size_kb,
                if call.success { "✅" } else { "❌" },
                session_info
            ));
        }
        
        Ok(report)
    }
}

// FIXED: Global instance with proper path handling
lazy_static::lazy_static! {
    pub static ref BRIGHTDATA_METRICS: BrightDataMetricsLogger = {
        let log_path = std::env::var("BRIGHTDATA_METRICS_LOG_PATH")
            .unwrap_or_else(|_| "logs/brightdata_metrics.jsonl".to_string());
        
        // Ensure we have a proper file path, not a directory
        let final_path = if log_path.ends_with('/') || log_path.ends_with('\\') {
            format!("{}brightdata_metrics.jsonl", log_path)
        } else if std::path::Path::new(&log_path).is_dir() {
            format!("{}/brightdata_metrics.jsonl", log_path)
        } else if !log_path.contains('.') {
            format!("{}.jsonl", log_path)
        } else {
            log_path
        };
        
        println!("📊 Initializing BrightData metrics with file: {}", final_path);
        BrightDataMetricsLogger::new(&final_path)
    };
}