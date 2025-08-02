// src/filters/strategy.rs - PATCHED: Enhanced token budget management with predictive allocation
use crate::tool::{ToolResult, McpContent};
use crate::filters::response_filter::ResponseFilter;
use serde_json::Value;
use regex::Regex;
use std::sync::OnceLock;
use serde_json::json;
use std::sync::Mutex;
use std::collections::HashMap;

// ENHANCED: Dynamic size limits with predictive budgeting
pub const MAX_RESPONSE_SIZE: usize = 400;       
pub const MAX_CONTENT_LENGTH: usize = 200;      
pub const MIN_CONTENT_LENGTH: usize = 10;       
pub const SUMMARY_MAX_LENGTH: usize = 150;      
pub const EMERGENCY_MAX_LENGTH: usize = 50;     

// ENHANCED: Smarter token budget tracking with call prediction
pub const TOTAL_TOKEN_BUDGET: usize = 4_500;    
static GLOBAL_TOKEN_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();
static CALL_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();
// NEW: Track query priorities for better allocation
static PRIORITY_QUERIES: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
// NEW: Track successful extractions for learning
static EXTRACTION_SUCCESS: OnceLock<Mutex<HashMap<String, f32>>> = OnceLock::new();

static PRICE_REGEX: OnceLock<Regex> = OnceLock::new();
static MARKET_CAP_REGEX: OnceLock<Regex> = OnceLock::new();
static PE_REGEX: OnceLock<Regex> = OnceLock::new();
// NEW: Additional critical financial patterns
static VOLUME_REGEX: OnceLock<Regex> = OnceLock::new();
static DIVIDEND_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseStrategy;

#[derive(Debug, Clone)]
pub enum ResponseType {
    Empty,           
    Error,           
    Skip,            
    Emergency,       // 8-12 tokens
    KeyMetrics,      // 15-25 tokens
    Summary,         // 25-40 tokens
    Minimal,         // 40-60 tokens
    Filtered,        // 60-80 tokens
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryPriority {
    Critical,    // Price, current value queries
    High,        // Market cap, PE ratios
    Medium,      // Other financial metrics
    Low,         // General company info
}

impl ResponseStrategy {
    /// Check if truncate filtering is enabled
    fn is_truncate_filter_enabled() -> bool {
        std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false)
    }

    /// Initialize global counters with enhanced tracking
    fn init_counters() {
        GLOBAL_TOKEN_COUNTER.get_or_init(|| Mutex::new(0));
        CALL_COUNTER.get_or_init(|| Mutex::new(0));
        PRIORITY_QUERIES.get_or_init(|| Mutex::new(HashMap::new()));
        EXTRACTION_SUCCESS.get_or_init(|| Mutex::new(HashMap::new()));
    }
    
    /// NEW: Classify query priority for better token allocation
    pub fn classify_query_priority(query: &str) -> QueryPriority {
        let query_lower = query.to_lowercase();
        
        // Critical: Real-time pricing data
        if query_lower.contains("price") || query_lower.contains("current") || 
           query_lower.contains("ltp") || query_lower.contains("quote") ||
           query_lower.contains("live") {
            return QueryPriority::Critical;
        }
        
        // High: Key financial metrics
        if query_lower.contains("market cap") || query_lower.contains("mcap") ||
           query_lower.contains("pe ratio") || query_lower.contains("p/e") ||
           query_lower.contains("valuation") || query_lower.contains("dividend") {
            return QueryPriority::High;
        }
        
        // Medium: Other financial data
        if query_lower.contains("financial") || query_lower.contains("ratio") ||
           query_lower.contains("volume") || query_lower.contains("revenue") ||
           query_lower.contains("profit") {
            return QueryPriority::Medium;
        }
        
        // Low: Everything else
        QueryPriority::Low
    }
    
    /// ENHANCED: Predictive token allocation based on remaining budget and call patterns
    pub fn get_recommended_token_allocation(query: &str) -> usize {
        if !Self::is_truncate_filter_enabled() {
            return MAX_RESPONSE_SIZE; // Full allocation if filtering disabled
        }

        Self::init_counters();
        let (used_tokens, remaining_tokens) = Self::get_token_budget_status();
        let call_count = *CALL_COUNTER.get().unwrap().lock().unwrap();
        
        // Estimate remaining calls based on usage pattern
        let estimated_remaining_calls = if call_count < 3 {
            25 // Conservative estimate for early calls
        } else if call_count < 10 {
            std::cmp::max(15, 35 - call_count)
        } else {
            std::cmp::max(5, 40 - call_count)
        };
        
        let base_allocation = if estimated_remaining_calls > 0 {
            remaining_tokens / estimated_remaining_calls
        } else {
            EMERGENCY_MAX_LENGTH
        };
        
        // Adjust based on query priority
        let priority = Self::classify_query_priority(query);
        let priority_multiplier = match priority {
            QueryPriority::Critical => 2.0,   // 2x allocation for critical queries
            QueryPriority::High => 1.5,       // 1.5x for high priority
            QueryPriority::Medium => 1.0,     // Normal allocation
            QueryPriority::Low => 0.7,        // Reduced allocation
        };
        
        let allocated_tokens = (base_allocation as f32 * priority_multiplier) as usize;
        
        // Apply bounds based on priority
        let max_tokens = match priority {
            QueryPriority::Critical => MAX_CONTENT_LENGTH,
            QueryPriority::High => SUMMARY_MAX_LENGTH,
            QueryPriority::Medium => SUMMARY_MAX_LENGTH / 2,
            QueryPriority::Low => EMERGENCY_MAX_LENGTH,
        };
        
        std::cmp::min(allocated_tokens, max_tokens)
    }
    
    /// Get current token usage and remaining budget
    pub fn get_token_budget_status() -> (usize, usize) {
        if !Self::is_truncate_filter_enabled() {
            return (0, TOTAL_TOKEN_BUDGET);
        }

        Self::init_counters();
        let used = *GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        let remaining = TOTAL_TOKEN_BUDGET.saturating_sub(used);
        (used, remaining)
    }
    
    /// ENHANCED: Record token usage with priority tracking and success metrics
    fn record_token_usage(tokens: usize, query: &str, success: bool) {
        if !Self::is_truncate_filter_enabled() {
            return;
        }

        Self::init_counters();
        
        // Record total usage
        let mut counter = GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        *counter += tokens;
        
        let mut call_counter = CALL_COUNTER.get().unwrap().lock().unwrap();
        *call_counter += 1;
        
        // Track priority queries for future optimization
        let priority = Self::classify_query_priority(query);
        if matches!(priority, QueryPriority::Critical | QueryPriority::High) {
            let mut priorities = PRIORITY_QUERIES.get().unwrap().lock().unwrap();
            *priorities.entry(query.to_string()).or_insert(0) += tokens;
        }
        
        // Track extraction success rates
        let query_type = Self::get_query_type(query);
        let mut success_tracker = EXTRACTION_SUCCESS.get().unwrap().lock().unwrap();
        let current_rate = success_tracker.get(&query_type).copied().unwrap_or(0.5);
        let new_rate = if success {
            (current_rate * 0.9) + 0.1 // Increase success rate
        } else {
            current_rate * 0.9 // Decrease success rate
        };
        success_tracker.insert(query_type, new_rate);
    }
    
    /// NEW: Get query type for success tracking
    fn get_query_type(query: &str) -> String {
        let query_lower = query.to_lowercase();
        if query_lower.contains("price") { "price".to_string() }
        else if query_lower.contains("market cap") { "mcap".to_string() }
        else if query_lower.contains("pe") { "pe".to_string() }
        else { "general".to_string() }
    }
    
    /// Reset token counters (call at start of new session)
    pub fn reset_token_budget() {
        Self::init_counters();
        
        let mut counter = GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        *counter = 0;
        
        let mut call_counter = CALL_COUNTER.get().unwrap().lock().unwrap();
        *call_counter = 0;
        
        // Keep priority tracking and success rates across sessions for learning
    }
    
    /// ENHANCED: Smarter response type determination with predictive elements
    pub fn determine_response_type(content: &str, query: &str) -> ResponseType {
        if query.trim().is_empty() || content.trim().is_empty() {
            return ResponseType::Empty;
        }

        if !Self::is_truncate_filter_enabled() {
            return ResponseType::Filtered;
        }
        
        let (_, remaining_tokens) = Self::get_token_budget_status();
        let priority = Self::classify_query_priority(query);
        let recommended_allocation = Self::get_recommended_token_allocation(query);
        
        // Emergency mode if very low on tokens
        if remaining_tokens < 50 {
            return if ResponseFilter::contains_financial_data(content) {
                ResponseType::Emergency
            } else {
                ResponseType::Skip
            };
        }
        
        // Low token mode - be very selective
        if remaining_tokens < 200 {
            if ResponseFilter::is_error_page(content) {
                return ResponseType::Skip;
            }
            return if matches!(priority, QueryPriority::Critical | QueryPriority::High) {
                ResponseType::Emergency
            } else {
                ResponseType::Skip
            };
        }
        
        // Regular logic with priority consideration
        if ResponseFilter::is_error_page(content) {
            return ResponseType::Skip;
        }
        
        let quality_score = ResponseFilter::get_content_quality_score(content);
        
        // Priority-adjusted thresholds
        let (skip_threshold, emergency_threshold, key_threshold, summary_threshold) = match priority {
            QueryPriority::Critical => (20, 40, 60, 75),  // More lenient for critical
            QueryPriority::High => (30, 50, 70, 80),      // Standard thresholds
            QueryPriority::Medium => (40, 60, 75, 85),    // Stricter for medium
            QueryPriority::Low => (50, 70, 80, 90),       // Very strict for low
        };
        
        match quality_score {
            score if score <= skip_threshold => ResponseType::Skip,
            score if score <= emergency_threshold => ResponseType::Emergency,
            score if score <= key_threshold => ResponseType::KeyMetrics,
            score if score <= summary_threshold => ResponseType::Summary,
            _ => {
                if content.len() > recommended_allocation * 4 { // 4 chars per token estimate
                    ResponseType::Summary
                } else {
                    ResponseType::Filtered
                }
            }
        }
    }
    
    /// Create hyper-efficient responses with enhanced token tracking
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
        
        // Record usage with success tracking
        if Self::is_truncate_filter_enabled() {
            let estimated_tokens = ResponseFilter::estimate_tokens(&response_text);
            let success = !response_text.contains("No data") && !response_text.contains("N/A");
            Self::record_token_usage(estimated_tokens, query, success);
        }
        
        if response_text.is_empty() {
            ToolResult::success_with_text("".to_string())
        } else {
            let mcp_content = vec![McpContent::text(response_text)];
            ToolResult::success_with_raw(mcp_content, json!({"ok": true}))
        }
    }
    
    /// ENHANCED: Emergency response with smarter extraction
    fn create_emergency_response(content: &str, query: &str) -> String {
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let priority = Self::classify_query_priority(query);
        
        // Priority-based extraction order
        match priority {
            QueryPriority::Critical => {
                // For critical queries, prioritize price above all
                let price_regex = PRICE_REGEX.get_or_init(|| {
                    Regex::new(r"(?i)(?:price|current|ltp)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)").unwrap()
                });
                
                if let Some(cap) = price_regex.captures(content) {
                    return format!("{}:₹{}", abbrev_query, &cap[1]);
                }
            },
            
            QueryPriority::High => {
                // For high priority, try market cap or PE
                let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
                    Regex::new(r"(?i)market\s*cap[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|b)?").unwrap()
                });
                
                if let Some(caps) = market_cap_regex.captures(content) {
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    return format!("{}:MC₹{}{}", abbrev_query, &caps[1], unit);
                }
            },
            
            _ => {
                // For others, any financial number
                let any_money_regex = Regex::new(r"[₹$]\s*([0-9,]+)").unwrap();
                if let Some(cap) = any_money_regex.captures(content) {
                    return format!("{}:₹{}", abbrev_query, &cap[1]);
                }
            }
        }
        
        format!("{}:N/A", abbrev_query)
    }
    
    /// ENHANCED: Ultra-abbreviate queries with expanded coverage
    pub fn ultra_abbreviate_query(query: &str) -> String {
        let q = query.to_uppercase().replace("LIMITED", "").replace("LTD", "");
        
        // Enhanced abbreviation map with more Indian stocks
        let abbreviations = [
            ("TATAMOTORS", "TM"),
            ("RELIANCEINDUSTRIES", "RIL"),
            ("RELIANCE", "RIL"),
            ("TATACONSULTANCYSERVICES", "TCS"),
            ("TCS", "TCS"),
            ("HDFCBANK", "HDFC"),
            ("BHARTIAIRTEL", "BHARTI"),
            ("INFOSYS", "INFY"),
            ("WIPRO", "WIP"),
            ("MARUTISUZUKI", "MARUTI"),
            ("MARUTI", "MARUTI"),
            ("STATEBANKOFINDIA", "SBI"),
            ("SBIN", "SBI"),
            ("ITCLTD", "ITC"),
            ("ITC", "ITC"),
            ("ADANIENTERPRISES", "ADANI"),
            ("KOTAKMAHINDRABANK", "KOTAK"),
            ("BAJAJFINANCE", "BAJAJFIN"),
            ("ASIANPAINTS", "ASIAN"),
            ("NESTLEINDIA", "NESTLE"),
            ("HINDUNILVR", "HUL"),
            ("ULTRACEMCO", "ULTRA"),
            ("SUNPHARMA", "SUNPH"),
            ("NTPC", "NTPC"),
            ("POWERGRIDCORP", "PGRID"),
            ("ONGC", "ONGC"),
            ("COALINDIA", "COAL"),
            ("JSWSTEEL", "JSW"),
            ("TATASTEEL", "TSTEEL"),
            ("HINDUNILVR", "HUL"),
            ("BRITANNIA", "BRIT"),
            ("DIVISLAB", "DIV"),
            ("DRREDDY", "DRR"),
            ("CIPLA", "CIPLA"),
            ("APOLLOHOSP", "APOLLO"),
            ("TECHM", "TECHM"),
            ("HCLTECH", "HCL"),
            ("LTI", "LTI"),
            ("MINDTREE", "MIND"),
        ];
        
        for (full, abbrev) in &abbreviations {
            if q.contains(full) {
                return abbrev.to_string();
            }
        }
        
        // Smart truncation for unknown queries
        if q.len() <= 5 {
            q
        } else if q.len() <= 10 {
            q[..6].to_string()
        } else {
            // For very long queries, take first 4 characters
            q[..4].to_string()
        }
    }
    
    /// ENHANCED: Key metrics extraction with priority-based ordering
    fn extract_key_metrics_only(content: &str, query: &str) -> String {
        let recommended_allocation = Self::get_recommended_token_allocation(query);
        let max_chars = recommended_allocation * 4; // 4 chars per token estimate
        
        let price_regex = PRICE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current|ltp)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
            Regex::new(r"(?i)market\s*cap[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|crore|billion|b)?").unwrap()
        });
        
        let pe_regex = PE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:p/e|pe)\s*ratio?[\s:]*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let volume_regex = VOLUME_REGEX.get_or_init(|| {
            Regex::new(r"(?i)volume[\s:]*([0-9,]+\.?[0-9]*)\s*(shares?|cr|l)?").unwrap()
        });
        
        let dividend_regex = DIVIDEND_REGEX.get_or_init(|| {
            Regex::new(r"(?i)dividend\s*yield?[\s:]*([0-9,]+\.?[0-9]*)%?").unwrap()
        });
        
        let mut metrics = Vec::new();
        let mut current_length = 0;
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let priority = Self::classify_query_priority(query);
        
        // Extract in priority order based on query type
        let extraction_order: Vec<Box<dyn Fn() -> Option<String>>> = match priority {
            QueryPriority::Critical => vec![
                Box::new(|| price_regex.captures(content).map(|caps| format!("₹{}", &caps[1]))),
                Box::new(|| market_cap_regex.captures(content).map(|caps| {
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    format!("MC₹{}{}", &caps[1], unit)
                })),
                Box::new(|| pe_regex.captures(content).map(|caps| format!("PE{}", &caps[1]))),
            ],
            QueryPriority::High => vec![
                Box::new(|| market_cap_regex.captures(content).map(|caps| {
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    format!("MC₹{}{}", &caps[1], unit)
                })),
                Box::new(|| price_regex.captures(content).map(|caps| format!("₹{}", &caps[1]))),
                Box::new(|| pe_regex.captures(content).map(|caps| format!("PE{}", &caps[1]))),
            ],
            _ => vec![
                Box::new(|| price_regex.captures(content).map(|caps| format!("₹{}", &caps[1]))),
                Box::new(|| market_cap_regex.captures(content).map(|caps| {
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    format!("MC₹{}{}", &caps[1], unit)
                })),
                Box::new(|| pe_regex.captures(content).map(|caps| format!("PE{}", &caps[1]))),
                Box::new(|| volume_regex.captures(content).map(|caps| {
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    format!("V{}{}", &caps[1], unit)
                })),
                Box::new(|| dividend_regex.captures(content).map(|caps| format!("DY{}%", &caps[1]))),
            ],
        };
        
        for extractor in extraction_order {
            if current_length >= max_chars { break; }
            
            if let Some(metric) = extractor() {
                if current_length + metric.len() + 1 <= max_chars { // +1 for separator
                    current_length += metric.len() + 1;
                    metrics.push(metric);
                }
            }
        }
        
        let result = if metrics.is_empty() {
            format!("{}:No data", abbrev_query)
        } else {
            format!("{}:{}", abbrev_query, metrics.join("|"))
        };
        
        // Ensure within budget
        if result.len() > max_chars {
            format!("{}…", &result[..max_chars.saturating_sub(1)])
        } else {
            result
        }
    }
    
    /// ENHANCED: Hyper-compact summary with priority awareness
    fn create_hyper_compact_summary(content: &str, query: &str, market: &str) -> String {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // If very low on tokens, use emergency mode
        if remaining_tokens < 100 {
            return Self::create_emergency_response(content, query);
        }
        
        // Try metrics first
        let metrics = Self::extract_key_metrics_only(content, query);
        if !metrics.contains("No data") {
            return metrics;
        }
        
        // Fallback: extract first financial sentence with priority awareness
        let priority = Self::classify_query_priority(query);
        let search_terms = match priority {
            QueryPriority::Critical => vec!["₹", "$", "price", "current"],
            QueryPriority::High => vec!["₹", "$", "market cap", "pe ratio"],
            _ => vec!["₹", "$"],
        };
        
        let sentences: Vec<&str> = content.split('.')
            .map(|s| s.trim())
            .filter(|s| s.len() > 5 && search_terms.iter().any(|term| s.to_lowercase().contains(term)))
            .take(1)
            .collect();
        
        if let Some(sentence) = sentences.first() {
            let max_chars = Self::get_recommended_token_allocation(query) * 4;
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
        
        if remaining_tokens < 150 {
            return Self::create_hyper_compact_summary(content, query, market);
        }
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let abbrev_market = match market {
            "indian" | "india" | "nse" | "bse" => "IN",
            "us" | "usa" | "nasdaq" | "nyse" => "US",
            "global" | "international" => "GL",
            _ => "",
        };
        
        let metrics = Self::extract_key_metrics_only(content, query);
        if !metrics.contains("No data") {
            return if abbrev_market.is_empty() {
                metrics
            } else {
                format!("{}({})", metrics, abbrev_market)
            };
        }
        
        let base = if abbrev_market.is_empty() {
            format!("{}:", abbrev_query)
        } else {
            format!("{}({}):", abbrev_query, abbrev_market)
        };
        
        format!("{}No data", base)
    }
    
    /// Token-efficient filtered content
    fn create_token_efficient_filtered_content(content: &str, query: &str, market: &str, source: &str) -> String {
        let recommended_allocation = Self::get_recommended_token_allocation(query);
        
        // Use the enhanced financial data extraction
        let filtered = ResponseFilter::extract_high_value_financial_data(content, recommended_allocation);
        
        if filtered == "No data" {
            return Self::create_hyper_compact_summary(content, query, market);
        }
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let result = format!("{}: {}", abbrev_query, filtered);
        
        let max_chars = recommended_allocation * 4;
        if result.len() > max_chars {
            format!("{}…", &result[..max_chars.saturating_sub(1)])
        } else {
            result
        }
    }
    
    /// Apply HYPER-AGGRESSIVE size limits with token awareness
    pub fn apply_size_limits(mut result: ToolResult) -> ToolResult {
        if !Self::is_truncate_filter_enabled() {
            return result;
        }

        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // Dynamic max response based on remaining budget
        let max_response_chars = if remaining_tokens < 100 {
            EMERGENCY_MAX_LENGTH
        } else if remaining_tokens < 300 {
            SUMMARY_MAX_LENGTH
        } else {
            std::cmp::min(MAX_RESPONSE_SIZE, remaining_tokens * 4)
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
                .take(1)
                .collect();
        }
        
        result
    }
    
    /// Enhanced check for next source with predictive budget awareness
    pub fn should_try_next_source(content: &str) -> bool {
        if !Self::is_truncate_filter_enabled() {
            return false;
        }

        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // More conservative approach when budget is low
        if remaining_tokens < 200 {
            return false;
        }
        
        // Don't try next source for very short content that might be valid
        if content.len() < MIN_CONTENT_LENGTH {
            return remaining_tokens > 500; // Only if we have plenty of budget
        }
        
        if ResponseFilter::is_error_page(content) {
            return remaining_tokens > 300;
        }
        
        // If no financial indicators and we have good budget, try next
        let has_financial = ResponseFilter::contains_financial_data(content);
        !has_financial && remaining_tokens > 800
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
    
    /// NEW: Get budget status for monitoring
    pub fn get_budget_status_string() -> String {
        let (used, remaining) = Self::get_token_budget_status();
        let efficiency = if TOTAL_TOKEN_BUDGET > 0 {
            (used as f32 / TOTAL_TOKEN_BUDGET as f32) * 100.0
        } else { 0.0 };
        
        format!("Tokens: {}/{} ({:.1}% used, {} remaining)", 
                used, TOTAL_TOKEN_BUDGET, efficiency, remaining)
    }
    
    /// NEW: Force emergency mode for debugging
    pub fn force_emergency_mode() -> bool {
        let (_, remaining) = Self::get_token_budget_status();
        remaining < 100
    }
}