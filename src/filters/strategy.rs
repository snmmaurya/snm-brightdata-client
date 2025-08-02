// src/filters/strategy.rs
use crate::tool::{ToolResult, McpContent};
use crate::filters::response_filter::ResponseFilter;
use serde_json::Value;
use regex::Regex;
use std::sync::OnceLock;

// Response size limits
pub const MAX_RESPONSE_SIZE: usize = 10_000;  // 10KB total response limit
pub const MAX_CONTENT_LENGTH: usize = 5_000;  // 5KB per content block
pub const MIN_CONTENT_LENGTH: usize = 50;     // Minimum useful content

static PRICE_REGEX: OnceLock<Regex> = OnceLock::new();
static MARKET_CAP_REGEX: OnceLock<Regex> = OnceLock::new();
static PE_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseStrategy;

#[derive(Debug, Clone)]
pub enum ResponseType {
    Empty,           // Return minimal/empty response
    Error,           // Return error message
    Minimal,         // Return basic summary only
    Filtered,        // Return filtered content
    Full,           // Return complete filtered content
}

impl ResponseStrategy {
    /// Determine the appropriate response type based on content analysis
    pub fn determine_response_type(content: &str, query: &str) -> ResponseType {
        // 1. Empty query or content
        if query.trim().is_empty() || content.trim().is_empty() {
            return ResponseType::Empty;
        }
        
        // 2. Error pages
        if ResponseFilter::is_error_page(content) {
            return ResponseType::Error;
        }
        
        // 3. Get content quality score
        let quality_score = ResponseFilter::get_content_quality_score(content);
        
        match quality_score {
            0..=20 => ResponseType::Error,      // Very poor quality
            21..=40 => ResponseType::Minimal,   // Poor quality, provide summary
            41..=60 => ResponseType::Filtered,  // Good quality, filter and truncate
            61..=100 => {
                // High quality - check size
                if content.len() > MAX_RESPONSE_SIZE {
                    ResponseType::Filtered
                } else {
                    ResponseType::Full
                }
            }
            _ => ResponseType::Error, // Handle any other values (shouldn't happen with quality score 0-100)
        }
    }
    
    /// Create appropriate response based on strategy
    pub fn create_response(
        content: &str, 
        query: &str, 
        market: &str, 
        source: &str, 
        raw_data: Value,
        response_type: ResponseType
    ) -> ToolResult {
        match response_type {
            ResponseType::Empty => {
                ToolResult::success_with_text("Query parameter is required".to_string())
            }
            
            ResponseType::Error => {
                ToolResult::success_with_text(format!(
                    "❌ Unable to fetch reliable data for {}. Data source may be temporarily unavailable. Please try again later.", 
                    query
                ))
            }
            
            ResponseType::Minimal => {
                let summary = Self::create_minimal_summary(content, query, market);
                ToolResult::success_with_text(summary)
            }
            
            ResponseType::Filtered => {
                let filtered_content = ResponseFilter::filter_financial_content(content);
                let truncated_content = ResponseFilter::truncate_content(&filtered_content, MAX_CONTENT_LENGTH);
                
                let mcp_content = vec![McpContent::text(format!(
                    "📊 **Financial Data for {}**\n\nMarket: {} | Source: {}\n\n{}",
                    query, market.to_uppercase(), source, truncated_content
                ))];
                
                ToolResult::success_with_raw(mcp_content, raw_data)
            }
            
            ResponseType::Full => {
                let filtered_content = ResponseFilter::filter_financial_content(content);
                
                let mcp_content = vec![McpContent::text(format!(
                    "📊 **Financial Data for {}**\n\nMarket: {} | Source: {}\n\n{}",
                    query, market.to_uppercase(), source, filtered_content
                ))];
                
                ToolResult::success_with_raw(mcp_content, raw_data)
            }
        }
    }
    
    /// Create a minimal summary for low-quality content
    fn create_minimal_summary(content: &str, query: &str, market: &str) -> String {
        let mut summary = format!("📊 **Summary for {}** ({})\n\n", query, market.to_uppercase());
        
        // Try to extract key financial metrics using regex
        let price_regex = PRICE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current|last)[:\s]*[₹$]?\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
            Regex::new(r"(?i)market\s*cap[:\s]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|crore|billion|million|b|m)?").unwrap()
        });
        
        let pe_regex = PE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)p[/:]?e\s*(?:ratio)?[:\s]*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let mut found_data = false;
        
        // Extract price
        if let Some(caps) = price_regex.captures(content) {
            summary.push_str(&format!("💰 **Price**: ₹{}\n", &caps[1]));
            found_data = true;
        }
        
        // Extract market cap
        if let Some(caps) = market_cap_regex.captures(content) {
            let unit = caps.get(2).map_or("", |m| m.as_str());
            summary.push_str(&format!("📈 **Market Cap**: ₹{} {}\n", &caps[1], unit));
            found_data = true;
        }
        
        // Extract P/E ratio
        if let Some(caps) = pe_regex.captures(content) {
            summary.push_str(&format!("📊 **P/E Ratio**: {}\n", &caps[1]));
            found_data = true;
        }
        
        if !found_data {
            summary.push_str("⚠️ **Limited data available**\n");
            summary.push_str("Please check financial websites directly for current information.\n");
        } else {
            summary.push_str("\n💡 *Summary extracted from available data*\n");
        }
        
        summary
    }
    
    /// Apply size limits to existing ToolResult
    pub fn apply_size_limits(mut result: ToolResult) -> ToolResult {
        let total_size: usize = result.content.iter()
            .map(|c| c.text.len())
            .sum();
            
        if total_size > MAX_RESPONSE_SIZE {
            // Truncate content while preserving structure
            result.content = result.content.into_iter()
                .map(|mut content| {
                    if content.text.len() > MAX_CONTENT_LENGTH {
                        content.text = ResponseFilter::truncate_content(&content.text, MAX_CONTENT_LENGTH);
                    }
                    content
                })
                .collect();
        }
        
        result
    }
    
    /// Check if content should trigger fallback to next source
    pub fn should_try_next_source(content: &str) -> bool {
        // Try next source if:
        // 1. Error page
        // 2. No financial data
        // 3. Mostly navigation
        // 4. Too small or too large
        
        if ResponseFilter::is_error_page(content) {
            return true;
        }
        
        if !ResponseFilter::contains_financial_data(content) {
            return true;
        }
        
        if ResponseFilter::is_mostly_navigation(content) {
            return true;
        }
        
        if content.len() < MIN_CONTENT_LENGTH || content.len() > 100_000 {
            return true;
        }
        
        false
    }
    
    /// Create standardized error response
    pub fn create_error_response(query: &str, error_msg: &str) -> ToolResult {
        ToolResult::success_with_text(format!(
            "❌ **Unable to fetch data for {}**\n\n{}\n\n💡 Please try again later or check financial websites directly.",
            query, error_msg
        ))
    }
    
    /// Create standardized success response with emoji based on data type
    pub fn create_financial_response(
        data_type: &str,
        query: &str, 
        market: &str, 
        source: &str, 
        content: &str,
        raw_data: Value
    ) -> ToolResult {
        let emoji = match data_type.to_lowercase().as_str() {
            "stock" => "📈",
            "crypto" => "₿",
            "etf" => "📊",
            "bond" => "🏛️",
            "mutual_fund" => "💼",
            "commodity" => "🥇",
            "market" => "🌍",
            _ => "📊"
        };
        
        let response_type = Self::determine_response_type(content, query);
        
        let formatted_content = match response_type {
            ResponseType::Full | ResponseType::Filtered => {
                ResponseFilter::filter_financial_content(content)
            }
            _ => content.to_string()
        };
        
        let final_content = if formatted_content.len() > MAX_CONTENT_LENGTH {
            ResponseFilter::truncate_content(&formatted_content, MAX_CONTENT_LENGTH)
        } else {
            formatted_content
        };
        
        let mcp_content = vec![McpContent::text(format!(
            "{} **{} Data for {}**\n\nMarket: {} | Source: {}\n\n{}",
            emoji, 
            data_type.to_uppercase(), 
            query, 
            market.to_uppercase(), 
            source, 
            final_content
        ))];
        
        ToolResult::success_with_raw(mcp_content, raw_data)
    }
}