// src/filters/strategy.rs - Enhanced with DEDUCT_DATA environment variable only
use crate::tool::{ToolResult, McpContent};
use crate::filters::response_filter::ResponseFilter;
use serde_json::Value;
use regex::Regex;
use std::sync::OnceLock;
use serde_json::json;
use std::sync::Mutex;
use std::collections::HashMap;

// Size limits (not used when DEDUCT_DATA=false)
pub const MAX_RESPONSE_SIZE: usize = 200000;
pub const MAX_CONTENT_LENGTH: usize = 200000;
pub const MIN_CONTENT_LENGTH: usize = 200000;
pub const SUMMARY_MAX_LENGTH: usize = 200000;
pub const EMERGENCY_MAX_LENGTH: usize = 200000;

// Token budget management (not used when DEDUCT_DATA=false)
pub const TOTAL_TOKEN_BUDGET: usize = 2_00000;    
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
static STOCK_CHANGE_REGEX: OnceLock<Regex> = OnceLock::new();
static STOCK_SYMBOL_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseStrategy;

#[derive(Debug, Clone)]
pub enum ResponseType {
    Empty,           
    Error,           
    Skip,            
    Emergency,       
    KeyMetrics,      
    Summary,         
    Minimal,         
    Filtered,        
    StockFormatted,  // Default when DEDUCT_DATA=false
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryPriority {
    Critical,    
    High,        
    Medium,      
    Low,         
}

impl ResponseStrategy {
    /// ENHANCED: Check if data reduction is enabled via DEDUCT_DATA environment variable only
    fn is_data_reduction_enabled() -> bool {
        std::env::var("DEDUCT_DATA")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// ENHANCED: Check if truncate filtering is enabled (same as data reduction)
    fn is_truncate_filter_enabled() -> bool {
        Self::is_data_reduction_enabled()
    }

    /// TODO: Initialize global counters with enhanced tracking
    fn init_counters() {
        // TODO: Add counter initialization logic
        GLOBAL_TOKEN_COUNTER.get_or_init(|| Mutex::new(0));
        CALL_COUNTER.get_or_init(|| Mutex::new(0));
        PRIORITY_QUERIES.get_or_init(|| Mutex::new(HashMap::new()));
        EXTRACTION_SUCCESS.get_or_init(|| Mutex::new(HashMap::new()));
    }
    
    /// TODO: Stock-specific query priority classification
    pub fn classify_query_priority(query: &str) -> QueryPriority {
        // TODO: Add query priority classification logic
        // For now, return High priority for all queries
        QueryPriority::High
    }
    
    /// TODO: Check if query looks like a stock symbol
    fn is_likely_stock_symbol(query: &str) -> bool {
        // TODO: Add stock symbol detection logic
        // For now, return false
        false
    }
    
    /// ENHANCED: Stock-aware token allocation with DEDUCT_DATA control
    pub fn get_recommended_token_allocation(query: &str) -> usize {
        // If data reduction is disabled, return full allocation
        if !Self::is_data_reduction_enabled() {
            return MAX_RESPONSE_SIZE;
        }

        // TODO: Add token allocation logic when DEDUCT_DATA=true
        // For now, return full allocation
        MAX_RESPONSE_SIZE
    }
    
    /// ENHANCED: Get current token usage and remaining budget with DEDUCT_DATA control
    pub fn get_token_budget_status() -> (usize, usize) {
        // If data reduction is disabled, return unlimited budget
        if !Self::is_data_reduction_enabled() {
            return (0, TOTAL_TOKEN_BUDGET);
        }

        // TODO: Add token budget tracking logic when DEDUCT_DATA=true
        // For now, return full budget available
        (0, TOTAL_TOKEN_BUDGET)
    }
    
    /// ENHANCED: Response type determination with DEDUCT_DATA control
    pub fn determine_response_type(content: &str, query: &str) -> ResponseType {
        if query.trim().is_empty() || content.trim().is_empty() {
            return ResponseType::Empty;
        }

        // If data reduction is disabled, use formatted response
        if !Self::is_data_reduction_enabled() {
            return ResponseType::StockFormatted;
        }
        
        // TODO: Add response type determination logic when DEDUCT_DATA=true
        // For now, return StockFormatted
        ResponseType::StockFormatted
    }
    
    /// ENHANCED: Record token usage with DEDUCT_DATA control
    fn record_token_usage(tokens: usize, query: &str, success: bool) {
        // Only record usage if data reduction is enabled
        if !Self::is_data_reduction_enabled() {
            return;
        }

        // TODO: Add token usage recording logic when DEDUCT_DATA=true
        // For now, do nothing
    }
    
    /// TODO: Enhanced query type classification for better tracking
    fn get_enhanced_query_type(query: &str) -> String {
        // TODO: Add query type classification logic
        // For now, return "general"
        "general".to_string()
    }
    
    /// ENHANCED: Create responses with stock-specific formatting
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
        
        // Record usage with success tracking (only if DEDUCT_DATA=true)
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
    
    /// ENHANCED: Create formatted stock response with markdown (default when DEDUCT_DATA=false)
    fn create_formatted_stock_response(content: &str, query: &str, market: &str, source: &str) -> String {
        // When DEDUCT_DATA=false, return full formatted content
        if !Self::is_data_reduction_enabled() {
            return format!(
                "📈 **{}** | {} Market\n\n## Full Content\n{}\n\n*Source: {}*",
                query.to_uppercase(), 
                market.to_uppercase(), 
                content,
                source
            );
        }

        // TODO: Add filtered data extraction logic when DEDUCT_DATA=true
        // For now, return full content formatted
        format!(
            "📈 **{}** | {} Market\n\n## Content\n{}\n\n*Source: {}*",
            query.to_uppercase(), 
            market.to_uppercase(), 
            content,
            source
        )
    }
    
    /// TODO: Enhanced emergency response for stock data
    fn create_emergency_stock_response(content: &str, query: &str) -> String {
        // TODO: Add emergency response logic
        // For now, return basic response
        format!("{}:Emergency", Self::ultra_abbreviate_query(query))
    }
    
    /// TODO: Extract key stock metrics with enhanced formatting
    fn extract_key_stock_metrics(content: &str, query: &str, market: &str) -> String {
        // TODO: Add key metrics extraction logic
        // For now, return basic response
        format!("{}({}):KeyMetrics", Self::ultra_abbreviate_query(query), market.to_uppercase())
    }
    
    /// TODO: Create stock summary with better structure
    fn create_stock_summary(content: &str, query: &str, market: &str) -> String {
        // TODO: Add stock summary creation logic
        // For now, return basic response
        format!("📈 {} ({}): Summary", query.to_uppercase(), market.to_uppercase())
    }
    
    /// TODO: Create minimal stock summary
    fn create_minimal_stock_summary(content: &str, query: &str, market: &str) -> String {
        // TODO: Add minimal summary logic
        // For now, delegate to key metrics
        Self::extract_key_stock_metrics(content, query, market)
    }
    
    /// TODO: Create token-efficient stock content
    fn create_token_efficient_stock_content(content: &str, query: &str, market: &str, _source: &str) -> String {
        // TODO: Add token-efficient content creation logic
        // For now, return basic response
        format!("{} ({}): Filtered", Self::ultra_abbreviate_query(query), market.to_uppercase())
    }
    
    /// TODO: Enhanced abbreviation with more stock symbols
    pub fn ultra_abbreviate_query(query: &str) -> String {
        // TODO: Add query abbreviation logic
        // For now, return first 4 characters in uppercase
        query.chars().take(4).collect::<String>().to_uppercase()
    }
    
    /// ENHANCED: Apply size limits with DEDUCT_DATA control
    pub fn apply_size_limits(mut result: ToolResult) -> ToolResult {
        // If data reduction is disabled, don't apply size limits
        if !Self::is_data_reduction_enabled() {
            return result;
        }

        // TODO: Add size limit application logic when DEDUCT_DATA=true
        // For now, return result unchanged
        result
    }
    
    /// ENHANCED: Check for trying next source with DEDUCT_DATA control
    pub fn should_try_next_source(content: &str) -> bool {
        // If data reduction is disabled, be less aggressive about trying next sources
        if !Self::is_data_reduction_enabled() {
            return false;
        }

        // TODO: Add next source determination logic when DEDUCT_DATA=true
        // For now, return false
        false
    }
    
    /// TODO: Enhanced financial response creation
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
    
    /// TODO: Error response for stock queries
    pub fn create_error_response(query: &str, _error_msg: &str) -> ToolResult {
        let abbrev_query = Self::ultra_abbreviate_query(query);
        ToolResult::success_with_text(format!("{}:Error", abbrev_query))
    }
    
    /// TODO: Reset token counters
    pub fn reset_token_budget() {
        // TODO: Add token reset logic
        // For now, do nothing
    }
    
    /// TODO: Get budget status string
    pub fn get_budget_status_string() -> String {
        // TODO: Add budget status logic
        // For now, return basic status
        format!("Budget: {} (DEDUCT_DATA={})", 
               if Self::is_data_reduction_enabled() { "Limited" } else { "Unlimited" },
               Self::is_data_reduction_enabled())
    }
    
    /// TODO: Check if in emergency mode
    pub fn force_emergency_mode() -> bool {
        // TODO: Add emergency mode check logic
        // For now, return false
        false
    }
}