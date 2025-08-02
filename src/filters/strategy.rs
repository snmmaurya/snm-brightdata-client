// src/filters/strategy.rs - Ultra-compact version with global token budget management
use crate::tool::{ToolResult, McpContent};
use crate::filters::response_filter::ResponseFilter;
use serde_json::Value;
use regex::Regex;
use std::sync::OnceLock;
use serde_json::json;
use std::sync::Mutex;

// HYPER-AGGRESSIVE size limits for 5K token budget across ALL calls
pub const MAX_RESPONSE_SIZE: usize = 400;       // ~100 tokens per response (was 1,500)
pub const MAX_CONTENT_LENGTH: usize = 200;      // ~50 tokens (was 800)
pub const MIN_CONTENT_LENGTH: usize = 10;       // Minimal threshold (was 20)
pub const SUMMARY_MAX_LENGTH: usize = 150;      // ~40 tokens (was 300)
pub const EMERGENCY_MAX_LENGTH: usize = 50;     // Emergency mode: ~12 tokens

// Global token budget tracking
pub const TOTAL_TOKEN_BUDGET: usize = 4_500;    // 500 token safety buffer
static GLOBAL_TOKEN_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();
static CALL_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();

static PRICE_REGEX: OnceLock<Regex> = OnceLock::new();
static MARKET_CAP_REGEX: OnceLock<Regex> = OnceLock::new();
static PE_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseStrategy;

#[derive(Debug, Clone)]
pub enum ResponseType {
    Empty,           // Return minimal/empty response (for backward compatibility)
    Error,           // Return error message (for backward compatibility) 
    Skip,            // Skip response to save tokens
    Emergency,       // Absolute minimum (10-15 tokens)
    KeyMetrics,      // Essential data only (20-40 tokens)
    Summary,         // Ultra-compact summary (40-60 tokens)
    Minimal,         // Basic summary (60-80 tokens)
    Filtered,        // Filtered content (80-100 tokens)
}

impl ResponseStrategy {
    /// Initialize global counters
    fn init_counters() {
        GLOBAL_TOKEN_COUNTER.get_or_init(|| Mutex::new(0));
        CALL_COUNTER.get_or_init(|| Mutex::new(0));
    }
    
    /// Get current token usage and remaining budget
    pub fn get_token_budget_status() -> (usize, usize) {
        Self::init_counters();
        let used = *GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        let remaining = TOTAL_TOKEN_BUDGET.saturating_sub(used);
        (used, remaining)
    }
    
    /// Record token usage for a response
    fn record_token_usage(tokens: usize) {
        Self::init_counters();
        
        let mut counter = GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        *counter += tokens;
        
        let mut call_counter = CALL_COUNTER.get().unwrap().lock().unwrap();
        *call_counter += 1;
    }
    
    /// Reset token counters (call at start of new session)
    pub fn reset_token_budget() {
        Self::init_counters();
        
        let mut counter = GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        *counter = 0;
        
        let mut call_counter = CALL_COUNTER.get().unwrap().lock().unwrap();
        *call_counter = 0;
    }
    
    /// Determine response type with aggressive token budget management
    pub fn determine_response_type(content: &str, query: &str) -> ResponseType {
        if query.trim().is_empty() || content.trim().is_empty() {
            return ResponseType::Empty;
        }
        
        let (used_tokens, remaining_tokens) = Self::get_token_budget_status();
        
        // If we're running very low on tokens, skip or emergency mode only
        if remaining_tokens < 100 {
            if !ResponseFilter::contains_financial_data(content) {
                return ResponseType::Skip;
            }
            return ResponseType::Emergency;
        }
        
        // If low on tokens, prioritize essential data only
        if remaining_tokens < 300 {
            if ResponseFilter::is_error_page(content) {
                return ResponseType::Skip;
            }
            return ResponseType::KeyMetrics;
        }
        
        // Regular logic but more aggressive
        if ResponseFilter::is_error_page(content) {
            return ResponseType::Skip; // Skip errors to save tokens
        }
        
        let quality_score = ResponseFilter::get_content_quality_score(content);
        
        // More aggressive thresholds
        match quality_score {
            0..=30 => ResponseType::Skip,           // Skip low quality
            31..=50 => ResponseType::Emergency,     // Emergency mode
            51..=70 => ResponseType::KeyMetrics,    // Essential only
            71..=85 => ResponseType::Summary,       // Compact summary
            _ => {
                if content.len() > MAX_CONTENT_LENGTH {
                    ResponseType::Summary
                } else {
                    ResponseType::Filtered
                }
            }
        }
    }
    
    /// Create hyper-efficient responses with token tracking
    pub fn create_response(
        content: &str, 
        query: &str, 
        market: &str, 
        source: &str, 
        raw_data: Value,
        response_type: ResponseType
    ) -> ToolResult {
        let response_text = match response_type {
            ResponseType::Empty => {
                return ToolResult::success_with_text("Query required".to_string());
            }
            
            ResponseType::Error => {
                return ToolResult::success_with_text(format!("❌ No data for {}", Self::ultra_abbreviate_query(query)));
            }
            
            ResponseType::Skip => {
                return ToolResult::success_with_text("".to_string());
            }
            
            ResponseType::Emergency => {
                Self::create_emergency_response(content, query)
            }
            
            ResponseType::KeyMetrics => {
                Self::extract_key_metrics_only(content, query)
            }
            
            ResponseType::Summary => {
                Self::create_hyper_compact_summary(content, query, market)
            }
            
            ResponseType::Minimal => {
                Self::create_minimal_summary(content, query, market)
            }
            
            ResponseType::Filtered => {
                Self::create_token_efficient_filtered_content(content, query, market, source)
            }
        };
        
        // Estimate and record token usage
        let estimated_tokens = ResponseFilter::estimate_tokens(&response_text);
        Self::record_token_usage(estimated_tokens);
        
        if response_text.is_empty() {
            ToolResult::success_with_text("".to_string())
        } else {
            let mcp_content = vec![McpContent::text(response_text)];
            ToolResult::success_with_raw(mcp_content, json!({"ok": true}))
        }
    }
    
    /// NEW: Emergency response (absolute minimum - 10-15 tokens)
    fn create_emergency_response(content: &str, query: &str) -> String {
        let price_regex = PRICE_REGEX.get_or_init(|| {
            Regex::new(r"[₹$]\s*([0-9,]+)").unwrap()
        });
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        
        if let Some(cap) = price_regex.captures(content) {
            format!("{}:₹{}", abbrev_query, &cap[1])
        } else {
            format!("{}:N/A", abbrev_query)
        }
    }
    
    /// Ultra-abbreviate queries to save maximum tokens
    fn ultra_abbreviate_query(query: &str) -> String {
        let q = query.to_uppercase();
        
        match q.as_str() {
            "TATAMOTORS" => "TM",
            "RELIANCE" => "REL",
            "TCS" | "TATACONSULTANCYSERVICES" => "TCS",
            "HDFCBANK" => "HDFC",
            "BHARTIAIRTEL" => "BRTI",
            "INFOSYS" => "INFY",
            "WIPRO" => "WIP",
            "MARUTI" => "MAR",
            _ => {
                if q.len() <= 4 {
                    return q;
                }
                // Take first 4 chars or abbreviate intelligently
                if q.len() > 8 {
                    &q[..4]
                } else {
                    &q[..std::cmp::min(q.len(), 6)]
                }
            }
        }.to_string()
    }
    
    /// Enhanced key metrics extraction with token efficiency
    fn extract_key_metrics_only(content: &str, query: &str) -> String {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        let max_chars = std::cmp::min(
            MAX_CONTENT_LENGTH,
            remaining_tokens * 3 // 3 chars per token
        );
        
        let price_regex = PRICE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current)[:\s]*[₹$]?\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
            Regex::new(r"(?i)market\s*cap[:\s]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|billion|b)?").unwrap()
        });
        
        let pe_regex = PE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)p[/:]?e[:\s]*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let mut metrics = Vec::new();
        let abbrev_query = Self::ultra_abbreviate_query(query);
        
        if let Some(caps) = price_regex.captures(content) {
            metrics.push(format!("₹{}", &caps[1]));
        }
        
        if let Some(caps) = market_cap_regex.captures(content) {
            let unit = caps.get(2).map_or("", |m| m.as_str());
            metrics.push(format!("MC₹{}{}", &caps[1], unit));
        }
        
        if let Some(caps) = pe_regex.captures(content) {
            metrics.push(format!("PE{}", &caps[1]));
        }
        
        let result = if metrics.is_empty() {
            format!("{}:No data", abbrev_query)
        } else {
            format!("{}:{}", abbrev_query, metrics.join("|"))
        };
        
        // Ensure within token budget
        if result.len() > max_chars {
            format!("{}…", &result[..max_chars.saturating_sub(1)])
        } else {
            result
        }
    }
    
    /// NEW: Hyper-compact summary (20-40 tokens max)
    fn create_hyper_compact_summary(content: &str, query: &str, market: &str) -> String {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // If very low on tokens, use emergency mode
        if remaining_tokens < 150 {
            return Self::create_emergency_response(content, query);
        }
        
        let metrics = Self::extract_key_metrics_only(content, query);
        
        // If we got metrics, return them
        if !metrics.contains("No data") {
            return metrics;
        }
        
        // Try to extract first financial sentence
        let sentences: Vec<&str> = content.split('.')
            .map(|s| s.trim())
            .filter(|s| s.len() > 5 && (s.contains("₹") || s.contains("$")))
            .take(1)
            .collect();
        
        if let Some(sentence) = sentences.first() {
            let max_chars = std::cmp::min(SUMMARY_MAX_LENGTH, remaining_tokens * 3);
            let truncated = if sentence.len() > max_chars {
                format!("{}…", &sentence[..max_chars.saturating_sub(1)])
            } else {
                sentence.to_string()
            };
            format!("{}:{}", Self::ultra_abbreviate_query(query), truncated)
        } else {
            format!("{}:No data", Self::ultra_abbreviate_query(query))
        }
    }
    
    /// Enhanced minimal summary with aggressive token management
    fn create_minimal_summary(content: &str, query: &str, market: &str) -> String {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        if remaining_tokens < 200 {
            return Self::create_hyper_compact_summary(content, query, market);
        }
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let abbrev_market = match market {
            "indian" => "IN",
            "us" => "US",
            "global" => "GL",
            _ => "",
        };
        
        let price_regex = PRICE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current)[:\s]*[₹$]?\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
            Regex::new(r"(?i)market\s*cap[:\s]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|billion|b)?").unwrap()
        });
        
        let mut parts = Vec::new();
        
        if let Some(caps) = price_regex.captures(content) {
            parts.push(format!("₹{}", &caps[1]));
        }
        
        if let Some(caps) = market_cap_regex.captures(content) {
            let unit = caps.get(2).map_or("", |m| m.as_str());
            parts.push(format!("MC₹{}{}", &caps[1], unit));
        }
        
        let base = if abbrev_market.is_empty() {
            format!("{}:", abbrev_query)
        } else {
            format!("{}({}):", abbrev_query, abbrev_market)
        };
        
        let result = if parts.is_empty() {
            format!("{}No data", base)
        } else {
            format!("{}{}", base, parts.join("|"))
        };
        
        // Ensure within budget
        let max_chars = std::cmp::min(SUMMARY_MAX_LENGTH, remaining_tokens * 3);
        if result.len() > max_chars {
            format!("{}…", &result[..max_chars.saturating_sub(1)])
        } else {
            result
        }
    }
    
    /// Token-efficient filtered content
    fn create_token_efficient_filtered_content(content: &str, query: &str, market: &str, source: &str) -> String {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        let max_chars = std::cmp::min(MAX_CONTENT_LENGTH, remaining_tokens * 3);
        
        // Use the enhanced financial data extraction
        let filtered = ResponseFilter::extract_high_value_financial_data(content, remaining_tokens / 4);
        
        if filtered == "No data" {
            return Self::create_hyper_compact_summary(content, query, market);
        }
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let result = format!("{}: {}", abbrev_query, filtered);
        
        if result.len() > max_chars {
            format!("{}…", &result[..max_chars.saturating_sub(1)])
        } else {
            result
        }
    }
    
    /// Apply HYPER-AGGRESSIVE size limits with token awareness
    pub fn apply_size_limits(mut result: ToolResult) -> ToolResult {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // If very low on tokens, truncate aggressively
        let max_response_chars = if remaining_tokens < 200 {
            EMERGENCY_MAX_LENGTH
        } else {
            std::cmp::min(MAX_RESPONSE_SIZE, remaining_tokens * 3)
        };
        
        let total_size: usize = result.content.iter()
            .map(|c| c.text.len())
            .sum();
            
        if total_size > max_response_chars {
            result.content = result.content.into_iter()
                .map(|mut content| {
                    if content.text.len() > max_response_chars {
                        content.text.truncate(max_response_chars.saturating_sub(3));
                        content.text.push_str("…");
                    }
                    content
                })
                .take(1) // Only keep first content block
                .collect();
        }
        
        result
    }
    
    /// Enhanced check for next source with token budget awareness
    pub fn should_try_next_source(content: &str) -> bool {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // If very low on tokens, be more selective
        if remaining_tokens < 300 {
            return false; // Don't try more sources if low on budget
        }
        
        if content.len() < MIN_CONTENT_LENGTH {
            return true;
        }
        
        if ResponseFilter::is_error_page(content) {
            return true;
        }
        
        // If no financial indicators and we have budget, try next
        let has_financial = ResponseFilter::contains_financial_data(content);
        !has_financial && remaining_tokens > 500
    }
    
    /// Token-efficient error response
    pub fn create_error_response(query: &str, _error_msg: &str) -> ToolResult {
        let abbrev_query = Self::ultra_abbreviate_query(query);
        ToolResult::success_with_text(format!("{}:Error", abbrev_query))
    }
    
    /// Enhanced financial response with token budget management
    pub fn create_financial_response(
        data_type: &str,
        query: &str, 
        market: &str, 
        _source: &str, 
        content: &str,
        _raw_data: Value
    ) -> ToolResult {
        let response_type = Self::determine_response_type(content, query);
        
        let final_result = Self::create_response(
            content, query, market, _source, _raw_data, response_type
        );
        
        Self::apply_size_limits(final_result)
    }
}