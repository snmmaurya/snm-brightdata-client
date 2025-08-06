// src/filters/strategy.rs - Enhanced with stock-specific response strategies
use crate::tool::{ToolResult, McpContent};
use crate::filters::response_filter::ResponseFilter;
use serde_json::Value;
use regex::Regex;
use std::sync::OnceLock;
use serde_json::json;
use std::sync::Mutex;
use std::collections::HashMap;

// ENHANCED: Dynamic size limits optimized for stock data
pub const MAX_RESPONSE_SIZE: usize = 200000;       // Increased for stock data
pub const MAX_CONTENT_LENGTH: usize = 300;      // More room for stock metrics
pub const MIN_CONTENT_LENGTH: usize = 15;       // Stock data can be very concise
pub const SUMMARY_MAX_LENGTH: usize = 200;      // Adequate for stock summaries
pub const EMERGENCY_MAX_LENGTH: usize = 80;     // Enough for key stock metrics

// ENHANCED: Token budget management for stock queries
pub const TOTAL_TOKEN_BUDGET: usize = 4_500;    
static GLOBAL_TOKEN_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();
static CALL_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();
static PRIORITY_QUERIES: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
static EXTRACTION_SUCCESS: OnceLock<Mutex<HashMap<String, f32>>> = OnceLock::new();

// Enhanced regex patterns for stock data
static PRICE_REGEX: OnceLock<Regex> = OnceLock::new();
static MARKET_CAP_REGEX: OnceLock<Regex> = OnceLock::new();
static PE_REGEX: OnceLock<Regex> = OnceLock::new();
static VOLUME_REGEX: OnceLock<Regex> = OnceLock::new();
static DIVIDEND_REGEX: OnceLock<Regex> = OnceLock::new();
// NEW: Stock-specific patterns
static STOCK_CHANGE_REGEX: OnceLock<Regex> = OnceLock::new();
static STOCK_SYMBOL_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseStrategy;

#[derive(Debug, Clone)]
pub enum ResponseType {
    Empty,           
    Error,           
    Skip,            
    Emergency,       // 12-20 tokens (key metrics only)
    KeyMetrics,      // 20-40 tokens (price, mcap, pe)
    Summary,         // 40-80 tokens (formatted summary)
    Minimal,         // 80-120 tokens (brief analysis)
    Filtered,        // 120-200 tokens (comprehensive)
    StockFormatted,  // NEW: Formatted markdown response
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryPriority {
    Critical,    // Real-time price queries, urgent trading info
    High,        // Key financial metrics (MCap, P/E, Volume)
    Medium,      // Other financial ratios and analysis
    Low,         // General company information
}

impl ResponseStrategy {
    /// Check if data reduction is enabled via environment variable
    fn is_data_reduction_enabled() -> bool {
        std::env::var("DATA_REDUCTION")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// Check if truncate filtering is enabled (supports both new and legacy env vars)
    fn is_truncate_filter_enabled() -> bool {
        // Check both new DATA_REDUCTION and legacy TRUNCATE_FILTER
        Self::is_data_reduction_enabled() || 
        std::env::var("TRUNCATE_FILTER")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// Initialize global counters with enhanced tracking
    fn init_counters() {
        GLOBAL_TOKEN_COUNTER.get_or_init(|| Mutex::new(0));
        CALL_COUNTER.get_or_init(|| Mutex::new(0));
        PRIORITY_QUERIES.get_or_init(|| Mutex::new(HashMap::new()));
        EXTRACTION_SUCCESS.get_or_init(|| Mutex::new(HashMap::new()));
    }
    
    /// ENHANCED: Stock-specific query priority classification
    pub fn classify_query_priority(query: &str) -> QueryPriority {
        let query_lower = query.to_lowercase();
        
        // Critical: Real-time pricing and urgent trading data
        if query_lower.contains("price") || query_lower.contains("current") || 
           query_lower.contains("ltp") || query_lower.contains("quote") ||
           query_lower.contains("live") || query_lower.contains("real time") ||
           query_lower.contains("now") || query_lower.contains("today") {
            return QueryPriority::Critical;
        }
        
        // High: Key financial metrics for investment decisions
        if query_lower.contains("market cap") || query_lower.contains("mcap") ||
           query_lower.contains("pe ratio") || query_lower.contains("p/e") ||
           query_lower.contains("valuation") || query_lower.contains("dividend") ||
           query_lower.contains("volume") || query_lower.contains("eps") ||
           query_lower.contains("book value") {
            return QueryPriority::High;
        }
        
        // Medium: Other financial analysis
        if query_lower.contains("financial") || query_lower.contains("ratio") ||
           query_lower.contains("revenue") || query_lower.contains("profit") ||
           query_lower.contains("analysis") || query_lower.contains("performance") ||
           query_lower.contains("52 week") || query_lower.contains("beta") {
            return QueryPriority::Medium;
        }
        
        // Check if it's a likely stock symbol (high priority for exact symbols)
        if Self::is_likely_stock_symbol(&query_lower) {
            return QueryPriority::High;
        }
        
        // Low: General company information
        QueryPriority::Low
    }
    
    /// NEW: Check if query looks like a stock symbol
    fn is_likely_stock_symbol(query: &str) -> bool {
        let clean = query.trim().to_uppercase();
        
        if clean.len() < 2 || clean.len() > 12 {
            return false;
        }

        // Check for stock symbol patterns
        let has_stock_suffix = clean.ends_with(".NS") || clean.ends_with(".BO") || 
                              clean.ends_with(".BSE") || clean.ends_with(".NSE");
        
        if has_stock_suffix {
            return true;
        }
        
        // Common Indian stock symbols
        let indian_stocks = [
            "TCS", "RELIANCE", "HDFCBANK", "INFY", "WIPRO", "TATAMOTORS", 
            "MARUTI", "SBIN", "ITC", "BHARTIAIRTEL", "KOTAKBANK", "ASIANPAINT",
            "ULTRACEMCO", "NESTLEIND", "SUNPHARMA", "ONGC", "NTPC", "POWERGRID"
        ];
        
        if indian_stocks.contains(&clean.as_str()) {
            return true;
        }
        
        // General pattern: mostly letters, some numbers allowed
        let valid_chars = clean.chars().all(|c| c.is_alphanumeric());
        let has_letters = clean.chars().any(|c| c.is_alphabetic());
        let letter_ratio = clean.chars().filter(|c| c.is_alphabetic()).count() as f32 / clean.len() as f32;
        
        valid_chars && has_letters && letter_ratio >= 0.6
    }
    
    /// ENHANCED: Stock-aware token allocation with DATA_REDUCTION control
    pub fn get_recommended_token_allocation(query: &str) -> usize {
        // If data reduction is disabled, return full allocation
        if !Self::is_data_reduction_enabled() {
            return MAX_RESPONSE_SIZE;
        }

        Self::init_counters();
        let (_used_tokens, remaining_tokens) = Self::get_token_budget_status();
        let call_count = *CALL_COUNTER.get().unwrap().lock().unwrap();
        
        // Estimate remaining calls based on usage pattern
        let estimated_remaining_calls = if call_count < 3 {
            20 // More conservative for stock queries
        } else if call_count < 10 {
            std::cmp::max(12, 30 - call_count)
        } else {
            std::cmp::max(5, 35 - call_count)
        };
        
        let base_allocation = if estimated_remaining_calls > 0 {
            remaining_tokens / estimated_remaining_calls
        } else {
            EMERGENCY_MAX_LENGTH
        };
        
        // Enhanced priority-based allocation for stock data
        let priority = Self::classify_query_priority(query);
        let priority_multiplier = match priority {
            QueryPriority::Critical => 2.5,   // Even higher for critical stock data
            QueryPriority::High => 2.0,       // More allocation for key metrics
            QueryPriority::Medium => 1.2,     // Slightly more for analysis
            QueryPriority::Low => 0.8,        // Reduced for general info
        };
        
        let allocated_tokens = (base_allocation as f32 * priority_multiplier) as usize;
        
        // Apply bounds based on priority with stock-specific limits
        let max_tokens = match priority {
            QueryPriority::Critical => MAX_CONTENT_LENGTH,
            QueryPriority::High => SUMMARY_MAX_LENGTH,
            QueryPriority::Medium => SUMMARY_MAX_LENGTH * 2 / 3,
            QueryPriority::Low => EMERGENCY_MAX_LENGTH,
        };
        
        std::cmp::min(allocated_tokens, max_tokens)
    }
    
    /// Get current token usage and remaining budget with DATA_REDUCTION control
    pub fn get_token_budget_status() -> (usize, usize) {
        // If data reduction is disabled, return unlimited budget
        if !Self::is_data_reduction_enabled() {
            return (0, TOTAL_TOKEN_BUDGET);
        }

        Self::init_counters();
        let used = *GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        let remaining = TOTAL_TOKEN_BUDGET.saturating_sub(used);
        (used, remaining)
    }
    
    /// Enhanced response type determination with DATA_REDUCTION control
    pub fn determine_response_type(content: &str, query: &str) -> ResponseType {
        if query.trim().is_empty() || content.trim().is_empty() {
            return ResponseType::Empty;
        }

        // If data reduction is disabled, use formatted response
        if !Self::is_data_reduction_enabled() {
            return ResponseType::StockFormatted;
        }
        
        let (_, remaining_tokens) = Self::get_token_budget_status();
        let priority = Self::classify_query_priority(query);
        let recommended_allocation = Self::get_recommended_token_allocation(query);
        
        // Emergency mode if very low on tokens
        if remaining_tokens < 80 {
            return if ResponseFilter::contains_valid_stock_data(content) {
                ResponseType::Emergency
            } else {
                ResponseType::Skip
            };
        }
        
        // Low token mode - prioritize stock data
        if remaining_tokens < 300 {
            if ResponseFilter::is_error_page(content) {
                return ResponseType::Skip;
            }
            return if matches!(priority, QueryPriority::Critical | QueryPriority::High) {
                if ResponseFilter::contains_valid_stock_data(content) {
                    ResponseType::KeyMetrics
                } else {
                    ResponseType::Emergency
                }
            } else {
                ResponseType::Skip
            };
        }
        
        // Check for error pages
        if ResponseFilter::is_error_page(content) {
            return ResponseType::Skip;
        }
        
        // Enhanced quality assessment for stock data
        let quality_score = ResponseFilter::get_content_quality_score(content);
        let has_valid_stock_data = ResponseFilter::contains_valid_stock_data(content);
        
        // Priority-adjusted thresholds with stock data consideration
        let (skip_threshold, emergency_threshold, key_threshold, summary_threshold) = match priority {
            QueryPriority::Critical => (25, 45, 65, 80),  // More lenient for critical
            QueryPriority::High => (35, 55, 75, 85),      // Standard thresholds
            QueryPriority::Medium => (45, 65, 80, 90),    // Stricter for medium
            QueryPriority::Low => (55, 75, 85, 95),       // Very strict for low
        };
        
        // Boost score if we have valid stock data
        let adjusted_score = if has_valid_stock_data {
            std::cmp::min(100, quality_score + 15)
        } else {
            quality_score
        };
        
        match adjusted_score {
            score if score <= skip_threshold => ResponseType::Skip,
            score if score <= emergency_threshold => ResponseType::Emergency,
            score if score <= key_threshold => ResponseType::KeyMetrics,
            score if score <= summary_threshold => ResponseType::Summary,
            _ => {
                // For high-quality content, choose based on allocation
                if content.len() > recommended_allocation * 6 { // 6 chars per token for formatted content
                    ResponseType::Summary
                } else if has_valid_stock_data {
                    ResponseType::StockFormatted
                } else {
                    ResponseType::Filtered
                }
            }
        }
    }
    
    /// Record token usage with enhanced stock-specific tracking and DATA_REDUCTION control
    fn record_token_usage(tokens: usize, query: &str, success: bool) {
        // Only record usage if data reduction is enabled
        if !Self::is_data_reduction_enabled() {
            return;
        }

        Self::init_counters();
        
        // Record total usage
        let mut counter = GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        *counter += tokens;
        
        let mut call_counter = CALL_COUNTER.get().unwrap().lock().unwrap();
        *call_counter += 1;
        
        // Enhanced tracking for stock queries
        let priority = Self::classify_query_priority(query);
        if matches!(priority, QueryPriority::Critical | QueryPriority::High) {
            let mut priorities = PRIORITY_QUERIES.get().unwrap().lock().unwrap();
            *priorities.entry(query.to_string()).or_insert(0) += tokens;
        }
        
        // Track success rates by query type
        let query_type = Self::get_enhanced_query_type(query);
        let mut success_tracker = EXTRACTION_SUCCESS.get().unwrap().lock().unwrap();
        let current_rate = success_tracker.get(&query_type).copied().unwrap_or(0.5);
        let new_rate = if success {
            (current_rate * 0.85) + 0.15 // Faster learning for stock data
        } else {
            current_rate * 0.85
        };
        success_tracker.insert(query_type, new_rate);
    }
    
    /// Enhanced query type classification for better tracking
    fn get_enhanced_query_type(query: &str) -> String {
        let query_lower = query.to_lowercase();
        
        if query_lower.contains("price") || query_lower.contains("current") || query_lower.contains("ltp") {
            "price".to_string()
        } else if query_lower.contains("market cap") || query_lower.contains("mcap") {
            "market_cap".to_string()
        } else if query_lower.contains("pe") || query_lower.contains("p/e") {
            "pe_ratio".to_string()
        } else if query_lower.contains("volume") {
            "volume".to_string()
        } else if query_lower.contains("dividend") {
            "dividend".to_string()
        } else if Self::is_likely_stock_symbol(&query_lower) {
            "stock_symbol".to_string()
        } else {
            "general".to_string()
        }
    }
    
    /// Create enhanced responses with stock-specific formatting
    pub fn create_response(
        content: &str, 
        query: &str, 
        market: &str, 
        source: &str, 
        _raw_data: Value,
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
                Self::create_emergency_stock_response(content, query)
            }
            
            ResponseType::KeyMetrics => {
                Self::extract_key_stock_metrics(content, query, market)
            }
            
            ResponseType::Summary => {
                Self::create_stock_summary(content, query, market)
            }
            
            ResponseType::Minimal => {
                Self::create_minimal_stock_summary(content, query, market)
            }
            
            ResponseType::Filtered => {
                Self::create_token_efficient_stock_content(content, query, market, source)
            }
            
            ResponseType::StockFormatted => {
                Self::create_formatted_stock_response(content, query, market, source)
            }
        };
        
        // Record usage with success tracking
        if Self::is_truncate_filter_enabled() {
            let estimated_tokens = ResponseFilter::estimate_tokens(&response_text);
            let success = !response_text.contains("No data") && 
                         !response_text.contains("N/A") && 
                         ResponseFilter::contains_valid_stock_data(&response_text);
            Self::record_token_usage(estimated_tokens, query, success);
        }
        
        if response_text.is_empty() {
            ToolResult::success_with_text("".to_string())
        } else {
            let mcp_content = vec![McpContent::text(response_text)];
            ToolResult::success_with_raw(mcp_content, json!({"stock_data": true}))
        }
    }
    
    /// NEW: Create formatted stock response with markdown
    fn create_formatted_stock_response(content: &str, query: &str, market: &str, source: &str) -> String {
        let filtered_data = ResponseFilter::extract_high_value_financial_data(
            content, 
            Self::get_recommended_token_allocation(query)
        );
        
        if filtered_data == "No data" || filtered_data.is_empty() {
            return format!(
                "📈 **{}** | {} Market\n\n❌ **No financial data available**\n\n*Source: {}*",
                query.to_uppercase(), 
                market.to_uppercase(), 
                source
            );
        }

        // Parse and format the filtered data
        let metrics: Vec<&str> = filtered_data.split('|').map(|s| s.trim()).collect();
        let mut formatted_metrics = String::new();
        
        for metric in metrics {
            if metric.is_empty() { continue; }
            
            let formatted_metric = if metric.starts_with("₹") {
                format!("• **Price**: {}", metric)
            } else if metric.starts_with("MC₹") {
                format!("• **Market Cap**: {}", &metric[2..])
            } else if metric.starts_with("PE") {
                format!("• **P/E Ratio**: {}", &metric[2..])
            } else if metric.starts_with("V") {
                format!("• **Volume**: {}", &metric[1..])
            } else if metric.starts_with("DY") {
                format!("• **Dividend Yield**: {}", &metric[2..])
            } else {
                format!("• {}", metric)
            };
            
            formatted_metrics.push_str(&formatted_metric);
            formatted_metrics.push('\n');
        }
        
        if formatted_metrics.is_empty() {
            format!(
                "📈 **{}** | {} Market\n\n❌ **Unable to parse financial data**\n\n*Source: {}*",
                query.to_uppercase(), 
                market.to_uppercase(), 
                source
            )
        } else {
            format!(
                "📈 **{}** | {} Market\n\n## Key Metrics\n{}\n*Source: {}*",
                query.to_uppercase(), 
                market.to_uppercase(), 
                formatted_metrics.trim(), 
                source
            )
        }
    }
    
    /// Enhanced emergency response for stock data
    fn create_emergency_stock_response(content: &str, query: &str) -> String {
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let priority = Self::classify_query_priority(query);
        
        // Initialize regex patterns
        let price_regex = PRICE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current|ltp)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
            Regex::new(r"(?i)market\s*cap[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|b|crore|billion)?").unwrap()
        });
        
        // Priority-based extraction with enhanced patterns
        match priority {
            QueryPriority::Critical => {
                // Try multiple price patterns
                let price_patterns = [
                    r"(?i)(?:current\s*price|price)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)",
                    r"(?i)(?:ltp|last\s*traded)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)",
                    r"[₹$]\s*([0-9,]+\.?[0-9]*)"
                ];
                
                for pattern in &price_patterns {
                    if let Ok(regex) = Regex::new(pattern) {
                        if let Some(cap) = regex.captures(content) {
                            return format!("{}:₹{}", abbrev_query, &cap[1]);
                        }
                    }
                }
            },
            
            QueryPriority::High => {
                // Try market cap first for high priority
                if let Some(caps) = market_cap_regex.captures(content) {
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    return format!("{}:MC₹{}{}", abbrev_query, &caps[1], unit);
                }
                
                // Fallback to price
                if let Some(cap) = price_regex.captures(content) {
                    return format!("{}:₹{}", abbrev_query, &cap[1]);
                }
            },
            
            _ => {
                // For others, any financial number with better patterns
                let financial_patterns = [
                    r"[₹$]\s*([0-9,]+\.?[0-9]*)\s*(cr|crore|k|lakh|l|billion|b)?",
                    r"(?i)(?:price|value|worth)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)"
                ];
                
                for pattern in &financial_patterns {
                    if let Ok(regex) = Regex::new(pattern) {
                        if let Some(caps) = regex.captures(content) {
                            let value = caps.get(1).map_or("", |m| m.as_str());
                            let unit = caps.get(2).map_or("", |m| m.as_str());
                            return format!("{}:₹{}{}", abbrev_query, value, unit);
                        }
                    }
                }
            }
        }
        
        format!("{}:N/A", abbrev_query)
    }
    
    /// Extract key stock metrics with enhanced formatting
    fn extract_key_stock_metrics(content: &str, query: &str, market: &str) -> String {
        let recommended_allocation = Self::get_recommended_token_allocation(query);
        let stock_data = ResponseFilter::extract_high_value_financial_data(content, recommended_allocation);
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        let market_abbrev = match market {
            "indian" | "india" => "IN",
            "us" | "usa" => "US", 
            "global" => "GL",
            _ => "",
        };
        
        if stock_data == "No data" || stock_data.is_empty() {
            return if market_abbrev.is_empty() {
                format!("{}:No data", abbrev_query)
            } else {
                format!("{}({}):No data", abbrev_query, market_abbrev)
            };
        }
        
        let formatted_data = stock_data.replace("|", " | ");
        
        if market_abbrev.is_empty() {
            format!("{}: {}", abbrev_query, formatted_data)
        } else {
            format!("{}({}): {}", abbrev_query, market_abbrev, formatted_data)
        }
    }
    
    /// Create stock summary with better structure
    fn create_stock_summary(content: &str, query: &str, market: &str) -> String {
        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        if remaining_tokens < 150 {
            return Self::extract_key_stock_metrics(content, query, market);
        }
        
        let stock_data = ResponseFilter::extract_high_value_financial_data(content, remaining_tokens / 4);
        
        if stock_data == "No data" {
            return format!("📈 {}: No financial data found", query.to_uppercase());
        }
        
        // Create a brief summary format
        let metrics: Vec<&str> = stock_data.split('|').collect();
        let mut summary_parts = Vec::new();
        
        for metric in metrics.iter().take(3) { // Limit to 3 key metrics
            if !metric.trim().is_empty() {
                summary_parts.push(metric.trim());
            }
        }
        
        if summary_parts.is_empty() {
            format!("📈 {}: No data", query.to_uppercase())
        } else {
            format!("📈 {} ({}): {}", 
                   query.to_uppercase(), 
                   market.to_uppercase(), 
                   summary_parts.join(" • "))
        }
    }
    
    /// Create minimal stock summary
    fn create_minimal_stock_summary(content: &str, query: &str, market: &str) -> String {
        Self::extract_key_stock_metrics(content, query, market)
    }
    
    /// Create token-efficient stock content
    fn create_token_efficient_stock_content(content: &str, query: &str, market: &str, _source: &str) -> String {
        let recommended_allocation = Self::get_recommended_token_allocation(query);
        let filtered = ResponseFilter::extract_high_value_financial_data(content, recommended_allocation);
        
        if filtered == "No data" {
            return Self::create_stock_summary(content, query, market);
        }
        
        let abbrev_query = Self::ultra_abbreviate_query(query);
        format!("{} ({}): {}", abbrev_query, market.to_uppercase(), filtered)
    }
    
    /// Enhanced abbreviation with more stock symbols
    pub fn ultra_abbreviate_query(query: &str) -> String {
        let q = query.to_uppercase().replace("LIMITED", "").replace("LTD", "").replace(".NS", "").replace(".BO", "");
        
        // Expanded abbreviation map
        let abbreviations = [
            ("TATAMOTORS", "TM"), ("RELIANCEINDUSTRIES", "RIL"), ("RELIANCE", "RIL"),
            ("TATACONSULTANCYSERVICES", "TCS"), ("TCS", "TCS"), ("HDFCBANK", "HDFC"),
            ("BHARTIAIRTEL", "BHARTI"), ("INFOSYS", "INFY"), ("WIPRO", "WIP"),
            ("MARUTISUZUKI", "MARUTI"), ("MARUTI", "MARUTI"), ("STATEBANKOFINDIA", "SBI"),
            ("ITCLTD", "ITC"), ("ITC", "ITC"), ("ADANIENTERPRISES", "ADANI"),
            ("KOTAKMAHINDRABANK", "KOTAK"), ("BAJAJFINANCE", "BAJAJFIN"),
            ("ASIANPAINTS", "ASIAN"), ("NESTLEINDIA", "NESTLE"), ("HINDUNILVR", "HUL"),
            ("ULTRACEMCO", "ULTRA"), ("SUNPHARMA", "SUNPH"), ("AXISBANK", "AXIS"),
            ("ICICIBANK", "ICICI"), ("JSWSTEEL", "JSW"), ("TATASTEEL", "TSTEEL"),
            ("APOLLOHOSP", "APOLLO"), ("TECHM", "TECHM"), ("HCLTECH", "HCL"),
            // US stocks
            ("APPLE", "AAPL"), ("MICROSOFT", "MSFT"), ("GOOGLE", "GOOGL"),
            ("AMAZON", "AMZN"), ("TESLA", "TSLA"), ("META", "META"), ("NVIDIA", "NVDA"),
        ];
        
        for (full, abbrev) in &abbreviations {
            if q.contains(full) {
                return abbrev.to_string();
            }
        }
        
        // Smart truncation for unknown queries
        if q.len() <= 5 {
            q
        } else if q.len() <= 8 {
            q[..5].to_string()
        } else {
            q[..4].to_string()
        }
    }
    
    /// Apply size limits with DATA_REDUCTION control
    pub fn apply_size_limits(mut result: ToolResult) -> ToolResult {
        // If data reduction is disabled, don't apply size limits
        if !Self::is_data_reduction_enabled() {
            return result;
        }

        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // Dynamic limits based on remaining budget
        let max_response_chars = if remaining_tokens < 150 {
            EMERGENCY_MAX_LENGTH
        } else if remaining_tokens < 400 {
            SUMMARY_MAX_LENGTH
        } else {
            std::cmp::min(MAX_RESPONSE_SIZE, remaining_tokens * 5) // 5 chars per token for formatted content
        };
        
        let total_size: usize = result.content.iter()
            .map(|c| c.text.len())
            .sum();
            
        if total_size > max_response_chars {
            result.content = result.content.into_iter()
                .map(|mut content| {
                    if content.text.len() > max_response_chars {
                        // Smart truncation preserving stock data
                        content.text = ResponseFilter::smart_truncate_preserving_financial_data(&content.text, max_response_chars);
                    }
                    content
                })
                .take(1)
                .collect();
        }
        
        result
    }
    
    /// Enhanced check for trying next source with DATA_REDUCTION control
    pub fn should_try_next_source(content: &str) -> bool {
        // If data reduction is disabled, be less aggressive about trying next sources
        if !Self::is_data_reduction_enabled() {
            return false;
        }

        let (_, remaining_tokens) = Self::get_token_budget_status();
        
        // More conservative with remaining budget
        if remaining_tokens < 300 {
            return false;
        }
        
        // Don't try next source if we have valid stock data
        if ResponseFilter::contains_valid_stock_data(content) && content.len() >= MIN_CONTENT_LENGTH {
            return false;
        }
        
        // Try next source for error pages or insufficient content
        if ResponseFilter::is_error_page(content) {
            return remaining_tokens > 400;
        }
        
        // Try next if no financial data and we have budget
        let has_financial = ResponseFilter::contains_financial_data(content);
        !has_financial && remaining_tokens > 600
    }
    
    /// Enhanced financial response creation
    pub fn create_financial_response(
        _data_type: &str,
        query: &str, 
        market: &str, 
        source: &str, 
        content: &str,
        raw_data: Value
    ) -> ToolResult {
        let response_type = Self::determine_response_type(content, query);
        
        let final_result = Self::create_response(
            content, query, market, source, raw_data, response_type
        );
        
        Self::apply_size_limits(final_result)
    }
    
    /// Error response for stock queries
    pub fn create_error_response(query: &str, _error_msg: &str) -> ToolResult {
        let abbrev_query = Self::ultra_abbreviate_query(query);
        ToolResult::success_with_text(format!("{}:Error", abbrev_query))
    }
    
    /// Reset token counters
    pub fn reset_token_budget() {
        Self::init_counters();
        
        let mut counter = GLOBAL_TOKEN_COUNTER.get().unwrap().lock().unwrap();
        *counter = 0;
        
        let mut call_counter = CALL_COUNTER.get().unwrap().lock().unwrap();
        *call_counter = 0;
    }
    
    /// Get budget status string
    pub fn get_budget_status_string() -> String {
        let (used, remaining) = Self::get_token_budget_status();
        let efficiency = if TOTAL_TOKEN_BUDGET > 0 {
            (used as f32 / TOTAL_TOKEN_BUDGET as f32) * 100.0
        } else { 0.0 };
        
        format!("Tokens: {}/{} ({:.1}% used, {} remaining)", 
                used, TOTAL_TOKEN_BUDGET, efficiency, remaining)
    }
    
    /// Check if in emergency mode
    pub fn force_emergency_mode() -> bool {
        let (_, remaining) = Self::get_token_budget_status();
        remaining < 100
    }
}