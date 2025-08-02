// crates/snm-brightdata-client/src/extras/logger.rs
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::fs;
use chrono::{DateTime, Utc};
use log::{info, error};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub execution_id: String,
    pub tool_name: String,
    pub timestamp: DateTime<Utc>,
    pub request_data: RequestData,
    pub response_data: Option<ResponseData>,
    pub execution_metadata: ExecutionMetadata,
    pub brightdata_details: Option<BrightDataDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestData {
    pub parameters: Value,
    pub method: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseData {
    pub content: Value,
    pub status: String,
    pub error: Option<String>,
    pub content_length: usize,
    pub processing_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub memory_usage_mb: Option<f64>,
    pub cpu_usage_percent: Option<f64>,
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrightDataDetails {
    pub zone_used: String,
    pub target_url: String,
    pub request_payload: Value,
    pub response_status: u16,
    pub response_headers: HashMap<String, String>,
    pub data_format: String,
    pub cost_estimate: Option<f64>,
}

pub struct JsonLogger {
    log_directory: String,
}

impl JsonLogger {
    pub fn new(log_directory: Option<String>) -> Self {
        let log_dir = log_directory.unwrap_or_else(|| "logs".to_string());
        Self {
            log_directory: log_dir,
        }
    }

    pub async fn ensure_log_directory(&self) -> Result<(), std::io::Error> {
        if !Path::new(&self.log_directory).exists() {
            fs::create_dir_all(&self.log_directory).await?;
        }
        Ok(())
    }

    pub async fn start_execution(&self, tool_name: &str, parameters: Value) -> ExecutionLog {
        let execution_id = self.generate_execution_id(tool_name);
        let timestamp = Utc::now();

        ExecutionLog {
            execution_id: execution_id.clone(),
            tool_name: tool_name.to_string(),
            timestamp,
            request_data: RequestData {
                parameters,
                method: "execute".to_string(),
                user_agent: None,
                ip_address: None,
                headers: HashMap::new(),
            },
            response_data: None,
            execution_metadata: ExecutionMetadata {
                start_time: timestamp,
                end_time: None,
                duration_ms: None,
                memory_usage_mb: None,
                cpu_usage_percent: None,
                success: false,
            },
            brightdata_details: None,
        }
    }

    pub async fn complete_execution(
        &self,
        mut log: ExecutionLog,
        response: Value,
        success: bool,
        error: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let end_time = Utc::now();
        let duration = (end_time - log.execution_metadata.start_time).num_milliseconds() as u64;

        log.execution_metadata.end_time = Some(end_time);
        log.execution_metadata.duration_ms = Some(duration);
        log.execution_metadata.success = success;

        log.response_data = Some(ResponseData {
            content_length: response.to_string().len(),
            content: response,
            status: if success { "success".to_string() } else { "error".to_string() },
            error,
            processing_time_ms: duration,
        });

        self.save_execution_log(&log).await?;
        Ok(())
    }

    pub async fn log_brightdata_request(
        &self,
        execution_id: &str,
        zone: &str,
        url: &str,
        payload: Value,
        response_status: u16,
        response_headers: HashMap<String, String>,
        data_format: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let brightdata_log = json!({
            "execution_id": execution_id,
            "timestamp": Utc::now().to_rfc3339(),
            "zone_used": zone,
            "target_url": url,
            "request_payload": payload,
            "response_status": response_status,
            "response_headers": response_headers,
            "data_format": data_format,
            "cost_estimate": self.estimate_cost(zone, &payload)
        });

        let filename = format!("{}/brightdata_{}.json", self.log_directory, execution_id);
        self.write_json_file(&filename, &brightdata_log).await?;
        
        info!("💾 BrightData request logged: {}", filename);
        Ok(())
    }

    async fn save_execution_log(&self, log: &ExecutionLog) -> Result<(), Box<dyn std::error::Error>> {
        self.ensure_log_directory().await?;

        // Save main execution log
        let main_filename = format!("{}/execution_{}.json", self.log_directory, log.execution_id);
        let log_json = serde_json::to_value(log)?;
        self.write_json_file(&main_filename, &log_json).await?;

        // Save separate request file
        let request_filename = format!("{}/request_{}.json", self.log_directory, log.execution_id);
        let request_json = serde_json::to_value(&log.request_data)?;
        self.write_json_file(&request_filename, &request_json).await?;

        // Save separate response file (if available)
        if let Some(ref response_data) = log.response_data {
            let response_filename = format!("{}/response_{}.json", self.log_directory, log.execution_id);
            let response_json = serde_json::to_value(response_data)?;
            self.write_json_file(&response_filename, &response_json).await?;
        }

        // Save daily summary
        self.update_daily_summary(log).await?;

        info!("💾 Execution logged: {}", main_filename);
        Ok(())
    }

    async fn write_json_file(&self, filename: &str, data: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let pretty_json = serde_json::to_string_pretty(data)?;
        fs::write(filename, pretty_json).await?;
        Ok(())
    }

    async fn update_daily_summary(&self, log: &ExecutionLog) -> Result<(), Box<dyn std::error::Error>> {
        let date = log.timestamp.format("%Y-%m-%d");
        let summary_filename = format!("{}/daily_summary_{}.json", self.log_directory, date);

        // Load existing summary or create new
        let mut summary = if Path::new(&summary_filename).exists() {
            let content = fs::read_to_string(&summary_filename).await?;
            serde_json::from_str::<Value>(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({
                "date": date.to_string(),
                "total_executions": 0,
                "successful_executions": 0,
                "failed_executions": 0,
                "tools_used": {},
                "total_processing_time_ms": 0,
                "average_processing_time_ms": 0.0
            })
        };

        // Update summary
        summary["total_executions"] = json!(summary["total_executions"].as_u64().unwrap_or(0) + 1);
        
        if log.execution_metadata.success {
            summary["successful_executions"] = json!(summary["successful_executions"].as_u64().unwrap_or(0) + 1);
        } else {
            summary["failed_executions"] = json!(summary["failed_executions"].as_u64().unwrap_or(0) + 1);
        }

        // Update tool usage
        let mut tools_used = summary["tools_used"].as_object().cloned().unwrap_or_default();
        let current_count = tools_used.get(&log.tool_name)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        tools_used.insert(log.tool_name.clone(), json!(current_count + 1));
        summary["tools_used"] = json!(tools_used);

        // Update timing
        if let Some(duration) = log.execution_metadata.duration_ms {
            let total_time = summary["total_processing_time_ms"].as_u64().unwrap_or(0) + duration;
            summary["total_processing_time_ms"] = json!(total_time);
            
            let total_executions = summary["total_executions"].as_u64().unwrap_or(1);
            let avg_time = total_time as f64 / total_executions as f64;
            summary["average_processing_time_ms"] = json!(avg_time);
        }

        summary["last_updated"] = json!(Utc::now().to_rfc3339());

        self.write_json_file(&summary_filename, &summary).await?;
        Ok(())
    }

    fn generate_execution_id(&self, tool_name: &str) -> String {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S%.3f");
        format!("{}_{}", tool_name, timestamp)
    }

    fn estimate_cost(&self, zone: &str, _payload: &Value) -> Option<f64> {
        // Simple cost estimation based on zone
        match zone {
            z if z.contains("serp") => Some(0.01), // $0.01 per SERP request
            z if z.contains("browser") => Some(0.05), // $0.05 per browser request
            _ => Some(0.001), // $0.001 per web unlocker request
        }
    }

    // Utility methods for querying logs
    pub async fn get_execution_logs_by_tool(&self, tool_name: &str) -> Result<Vec<ExecutionLog>, Box<dyn std::error::Error>> {
        let mut logs = Vec::new();
        let mut dir = fs::read_dir(&self.log_directory).await?;

        while let Some(entry) = dir.next_entry().await? {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with("execution_") && filename.contains(tool_name) {
                let content = fs::read_to_string(entry.path()).await?;
                if let Ok(log) = serde_json::from_str::<ExecutionLog>(&content) {
                    logs.push(log);
                }
            }
        }

        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(logs)
    }

    pub async fn get_daily_summary(&self, date: &str) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let summary_filename = format!("{}/daily_summary_{}.json", self.log_directory, date);
        
        if Path::new(&summary_filename).exists() {
            let content = fs::read_to_string(summary_filename).await?;
            let summary = serde_json::from_str::<Value>(&content)?;
            Ok(Some(summary))
        } else {
            Ok(None)
        }
    }
}

// Singleton instance for global access
lazy_static::lazy_static! {
    pub static ref JSON_LOGGER: JsonLogger = JsonLogger::new(
        std::env::var("LOG_DIRECTORY").ok()
    );
}