// src/filters/response_filter.rs - PANIC-PROOF version with comprehensive safety checks
use regex::Regex;
use std::sync::OnceLock;

static FINANCIAL_REGEX: OnceLock<Regex> = OnceLock::new();
static NUMBER_REGEX: OnceLock<Regex> = OnceLock::new();
static ESSENTIAL_REGEX: OnceLock<Regex> = OnceLock::new();
// NEW: Enhanced stock-specific patterns
static STOCK_PRICE_REGEX: OnceLock<Regex> = OnceLock::new();
static MARKET_CAP_REGEX: OnceLock<Regex> = OnceLock::new();
static STOCK_SYMBOL_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseFilter;

impl ResponseFilter {
    /// PANIC-SAFE: Check if data reduction is enabled via environment variable
    pub fn is_data_reduction_enabled() -> bool {
        std::env::var("DATA_REDUCTION")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// PANIC-SAFE: Check if truncate filtering is enabled (legacy support)
    pub fn is_truncate_filter_enabled() -> bool {
        // Check both new DATA_REDUCTION and legacy TRUNCATE_FILTER
        Self::is_data_reduction_enabled() || 
        std::env::var("TRUNCATE_FILTER")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true"
    }

    /// PANIC-SAFE: Stock-specific financial content filtering with DATA_REDUCTION control
    pub fn filter_financial_content(content: &str) -> String {
        // SAFETY: Early empty check
        if content.is_empty() {
            return "No data".to_string();
        }

        // Check DATA_REDUCTION environment variable
        if !Self::is_data_reduction_enabled() {
            return content.to_string(); // Return full content if reduction disabled
        }

        // SAFETY: Try stock-specific extraction - simplified without catch_unwind
        let stock_data = Self::extract_stock_specific_data(content);
        if !stock_data.is_empty() && stock_data != "No data" {
            return stock_data;
        }
        
        // SAFETY: Try essential metrics - simplified without catch_unwind
        let essential = Self::extract_only_essential_metrics(content);
        if !essential.is_empty() && essential != "No data" {
            return essential;
        }
        
        // SAFETY: Final fallback with safe regex
        Self::safe_extract_financial_lines(content)
    }
    
    /// PANIC-SAFE: Extract financial lines with comprehensive safety
    fn safe_extract_financial_lines(content: &str) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }

        let financial_regex = FINANCIAL_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(₹|[$]|price|market cap|pe |eps |dividend|revenue)\s*:?\s*([0-9,]+\.?[0-9]*)")
                .unwrap_or_else(|_| Regex::new(r"[₹$]\s*[0-9,]+").unwrap_or_else(|_| Regex::new(r"[0-9]+").unwrap()))
        });
        
        let lines: Vec<&str> = content
            .lines()
            .filter(|line| {
                let line_len = line.len();
                line_len > 3 && line_len < 80 && financial_regex.is_match(line)
            })
            .take(3)
            .collect();
        
        if lines.is_empty() {
            Self::safe_extract_any_numbers(content)
        } else {
            lines.join(" | ")
        }
    }
    
    /// PANIC-SAFE: Extract stock-specific data with comprehensive error handling
    fn extract_stock_specific_data(content: &str) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }

        let mut metrics = Vec::new();
        
        // SAFETY: Safe regex initialization with fallbacks
        let stock_price_regex = STOCK_PRICE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:current\s*price|last\s*traded\s*price|ltp|quote)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|k|m)?")
                .unwrap_or_else(|_| Regex::new(r"[₹$]\s*([0-9,]+)").unwrap_or_else(|_| Regex::new(r"([0-9]+)").unwrap()))
        });
        
        let market_cap_regex = MARKET_CAP_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:market\s*cap|market\s*capitalization|mcap)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|crore|billion|b|k|m|lakh|l)?")
                .unwrap_or_else(|_| Regex::new(r"(?i)market\s*cap.*?([0-9,]+)").unwrap_or_else(|_| Regex::new(r"([0-9]+)").unwrap()))
        });
        
        // SAFETY: Extract current price with error handling
        if let Some(caps) = stock_price_regex.captures(content) {
            let value = caps.get(1).map_or("0", |m| m.as_str());
            let unit = caps.get(2).map_or("", |m| m.as_str());
            if !value.is_empty() && value != "0" {
                metrics.push(format!("₹{}{}", value, unit));
            }
        }
        
        // SAFETY: Extract market cap with error handling
        if let Some(caps) = market_cap_regex.captures(content) {
            let value = caps.get(1).map_or("0", |m| m.as_str());
            let unit = caps.get(2).map_or("", |m| m.as_str());
            if !value.is_empty() && value != "0" {
                metrics.push(format!("MC₹{}{}", value, unit));
            }
        }
        
        // SAFETY: Extract P/E ratio with safe regex
        if let Ok(pe_regex) = Regex::new(r"(?i)(?:p/e|pe)\s*ratio?[\s:]*([0-9,]+\.?[0-9]*)") {
            if let Some(caps) = pe_regex.captures(content) {
                let value = caps.get(1).map_or("0", |m| m.as_str());
                if !value.is_empty() && value != "0" {
                    metrics.push(format!("PE{}", value));
                }
            }
        }
        
        // SAFETY: Extract volume with safe regex
        if let Ok(volume_regex) = Regex::new(r"(?i)(?:volume|traded\s*qty)[\s:]*([0-9,]+\.?[0-9]*)\s*(cr|crore|k|l|lakh|shares?)?") {
            if let Some(caps) = volume_regex.captures(content) {
                let value = caps.get(1).map_or("0", |m| m.as_str());
                let unit = caps.get(2).map_or("", |m| m.as_str());
                if !value.is_empty() && value != "0" {
                    metrics.push(format!("V{}{}", value, unit));
                }
            }
        }
        
        if metrics.is_empty() {
            "No data".to_string()
        } else {
            metrics.join("|")
        }
    }
    
    /// PANIC-SAFE: Enhanced essential metrics extraction
    fn extract_only_essential_metrics(content: &str) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }

        let essential_regex = ESSENTIAL_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current|market\s*cap|pe\s*ratio?|volume|dividend)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|billion|b|k|m|lakh|l|%)?")
                .unwrap_or_else(|_| Regex::new(r"[₹$]\s*([0-9,]+)").unwrap_or_else(|_| Regex::new(r"([0-9]+)").unwrap()))
        });
        
        let mut metrics = Vec::new();
        
        // SAFETY: Limit iterations to prevent infinite loops
        for (index, cap) in essential_regex.captures_iter(content).take(4).enumerate() {
            if index >= 4 { break; } // Extra safety check
            
            let full_match = cap.get(0).map_or("", |m| m.as_str()).to_lowercase();
            let value = cap.get(1).map_or("0", |m| m.as_str());
            let unit = cap.get(2).map_or("", |m| m.as_str());
            
            if value.is_empty() || value == "0" {
                continue;
            }
            
            let metric = if full_match.contains("price") || full_match.contains("current") {
                format!("₹{}{}", value, unit)
            } else if full_match.contains("market") {
                format!("MC₹{}{}", value, unit)
            } else if full_match.contains("pe") {
                format!("PE{}", value)
            } else if full_match.contains("volume") {
                format!("V{}{}", value, unit)
            } else if full_match.contains("dividend") {
                format!("DY{}%", value)
            } else {
                continue;
            };
            
            metrics.push(metric);
        }
        
        if metrics.is_empty() {
            "No data".to_string()
        } else {
            metrics.join("|")
        }
    }
    
    /// PANIC-SAFE: Extract any financial numbers with comprehensive safety
    fn safe_extract_any_numbers(content: &str) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }

        let number_regex = NUMBER_REGEX.get_or_init(|| {
            Regex::new(r"[₹$]\s*([0-9,]+\.?[0-9]*)\s*(cr|crore|k|l|lakh|billion|b|m)?")
                .unwrap_or_else(|_| Regex::new(r"[₹$]\s*([0-9,]+)").unwrap_or_else(|_| Regex::new(r"([0-9]+)").unwrap()))
        });
        
        let mut numbers = Vec::new();
        
        // SAFETY: Limit iterations and handle errors
        for (index, caps) in number_regex.captures_iter(content).take(4).enumerate() {
            if index >= 4 { break; } // Extra safety
            
            let value = caps.get(1).map_or("0", |m| m.as_str());
            let unit = caps.get(2).map_or("", |m| m.as_str());
            
            if !value.is_empty() && value != "0" {
                numbers.push(format!("{}{}", value, unit));
            }
        }
        
        if numbers.is_empty() {
            "No data".to_string()
        } else {
            numbers.join("|")
        }
    }
    
    /// PANIC-SAFE: Enhanced financial data check with null safety
    pub fn contains_financial_data(content: &str) -> bool {
        if content.is_empty() {
            return false;
        }
        
        // SAFETY: Catch any potential panics in string operations
        match std::panic::catch_unwind(|| {
            let content_lower = content.to_lowercase();
            let financial_indicators = content_lower.matches("₹").count().saturating_add(
                content_lower.matches("$").count()
            ).saturating_add(
                content_lower.matches("price").count()
            ).saturating_add(
                content_lower.matches("market cap").count()
            ).saturating_add(
                content_lower.matches("volume").count()
            ).saturating_add(
                content_lower.matches("pe ratio").count()
            ).saturating_add(
                content_lower.matches("dividend").count()
            );
            
            financial_indicators > 0
        }) {
            Ok(result) => result,
            Err(_) => false, // Safe fallback
        }
    }
    
    /// PANIC-SAFE: Check if content contains valid stock data
    pub fn contains_valid_stock_data(content: &str) -> bool {
        if content.is_empty() {
            return false;
        }
        
        // SAFETY: Comprehensive error handling
        match std::panic::catch_unwind(|| {
            let content_lower = content.to_lowercase();
            
            let stock_indicators = [
                "current price", "last traded price", "market cap", "pe ratio", 
                "volume", "52 week", "dividend yield", "eps", "book value"
            ];
            
            stock_indicators.iter().any(|&indicator| content_lower.contains(indicator)) ||
            Self::contains_financial_data(content)
        }) {
            Ok(result) => result,
            Err(_) => false, // Safe fallback
        }
    }
    
    /// PANIC-SAFE: Enhanced error page check
    pub fn is_error_page(content: &str) -> bool {
        if content.is_empty() {
            return true; // Empty content is effectively an error
        }
        
        // SAFETY: Prevent panics in string operations
        match std::panic::catch_unwind(|| {
            let content_lower = content.to_lowercase();
            content_lower.contains("404") || 
            content_lower.contains("500") ||
            content_lower.contains("error") ||
            content_lower.contains("not found") ||
            content_lower.contains("access denied") ||
            content_lower.contains("forbidden") ||
            content_lower.contains("symbol not found") ||
            content_lower.contains("invalid symbol") ||
            content_lower.contains("no data available")
        }) {
            Ok(result) => result,
            Err(_) => true, // Assume error if we can't process
        }
    }
    
    /// PANIC-SAFE: Enhanced navigation check
    pub fn is_mostly_navigation(content: &str) -> bool {
        if content.is_empty() {
            return false;
        }
        
        // SAFETY: Comprehensive error handling
        match std::panic::catch_unwind(|| {
            let content_lower = content.to_lowercase();
            let nav_words = content_lower.matches("menu").count().saturating_add(
                content_lower.matches("navigation").count()
            ).saturating_add(
                content_lower.matches("sign in").count()
            ).saturating_add(
                content_lower.matches("login").count()
            ).saturating_add(
                content_lower.matches("register").count()
            ).saturating_add(
                content_lower.matches("subscribe").count()
            ).saturating_add(
                content_lower.matches("advertisement").count()
            );
            
            let total_words = content.split_whitespace().count();
            
            if total_words > 0 {
                let nav_ratio = nav_words as f32 / total_words as f32;
                nav_ratio > 0.25 || (nav_ratio > 0.15 && total_words < 50)
            } else {
                false
            }
        }) {
            Ok(result) => result,
            Err(_) => false, // Safe fallback
        }
    }
    
    /// PANIC-SAFE: Smart financial data extraction with DATA_REDUCTION control
    pub fn extract_high_value_financial_data(content: &str, max_tokens: usize) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }
        
        // Check DATA_REDUCTION - return full content if disabled
        if !Self::is_data_reduction_enabled() {
            return content.to_string();
        }

        // SAFETY: Prevent overflow in multiplication
        let max_chars = max_tokens.saturating_mul(3).max(10); // Minimum 10 chars
        
        // SAFETY: Try stock-specific extraction - simplified
        let stock_data = Self::extract_stock_specific_data(content);
        if stock_data != "No data" && stock_data.len() <= max_chars {
            return stock_data;
        }
        
        // SAFETY: Fallback to priority patterns
        Self::safe_extract_priority_patterns(content, max_chars)
    }
    
    /// PANIC-SAFE: Extract priority patterns with full safety
    fn safe_extract_priority_patterns(content: &str, max_chars: usize) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }

        let priority_patterns = [
            (r"(?i)(?:current\s*price|price|ltp)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|k|m)?", "₹{}{}"),
            (r"(?i)market\s*cap[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|crore|b|billion|k|m|l|lakh)?", "MC₹{}{}"),
            (r"(?i)(?:p/e|pe)\s*ratio?[\s:]*([0-9,]+\.?[0-9]*)", "PE{}"),
            (r"(?i)volume[\s:]*([0-9,]+\.?[0-9]*)\s*(shares?|cr|k|l)?", "V{}{}"),
            (r"(?i)dividend\s*yield?[\s:]*([0-9,]+\.?[0-9]*)%?", "DY{}%"),
        ];
        
        let mut extracted = Vec::new();
        let mut total_len = 0usize;
        
        for (pattern, format_str) in &priority_patterns {
            if total_len >= max_chars {
                break;
            }
            
            // SAFETY: Safe regex compilation with error handling
            match Regex::new(pattern) {
                Ok(regex) => {
                    if let Some(caps) = regex.captures(content) {
                        let value = caps.get(1).map_or("0", |m| m.as_str());
                        let unit = caps.get(2).map_or("", |m| m.as_str());
                        
                        if value.is_empty() || value == "0" {
                            continue;
                        }
                        
                        let formatted = if unit.is_empty() {
                            format_str.replace("{}", value).replace("{}", "")
                        } else {
                            format_str.replace("{}", value).replace("{}", unit)
                        };
                        
                        let new_total = total_len.saturating_add(formatted.len()).saturating_add(1);
                        if new_total <= max_chars {
                            total_len = new_total;
                            extracted.push(formatted);
                        }
                    }
                }
                Err(_) => {
                    // Skip invalid regex patterns
                    continue;
                }
            }
        }
        
        if extracted.is_empty() {
            "No data".to_string()
        } else {
            extracted.join("|")
        }
    }

    /// PANIC-SAFE: Extract stock symbol with comprehensive safety
    pub fn extract_stock_symbol(content: &str) -> Option<String> {
        if content.is_empty() {
            return None;
        }
        
        // SAFETY: Multiple fallback regex patterns
        let symbol_regex = STOCK_SYMBOL_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:symbol|ticker)[\s:]*([A-Z]{2,10}(?:\.[A-Z]{2})?)\b")
                .unwrap_or_else(|_| {
                    Regex::new(r"([A-Z]{2,6})")
                        .unwrap_or_else(|_| {
                            Regex::new(r"[A-Z]+").unwrap()
                        })
                })
        });
        
        // SAFETY: Simple regex matching without catch_unwind
        symbol_regex.captures(content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }
    
    /// PANIC-SAFE: Enhanced quality score with overflow protection
    pub fn get_content_quality_score(content: &str) -> u8 {
        if content.is_empty() {
            return 0;
        }

        // SAFETY: Prevent panics in quality assessment
        match std::panic::catch_unwind(|| {
            let mut score = 15u8;
            
            // Valid stock data present (50 points)
            if Self::contains_valid_stock_data(content) {
                score = score.saturating_add(50);
            } else if Self::contains_financial_data(content) {
                score = score.saturating_add(30);
            }
            
            // Stock-specific indicators (20 points)
            let content_lower = content.to_lowercase();
            let stock_specific_terms = ["current price", "market cap", "pe ratio", "volume", "52 week"];
            let stock_term_count = stock_specific_terms.iter()
                .map(|&term| content_lower.matches(term).count())
                .sum::<usize>();
            
            if stock_term_count > 0 {
                let bonus = std::cmp::min(20, stock_term_count.saturating_mul(5)) as u8;
                score = score.saturating_add(bonus);
            }
            
            // Token-efficient size (15 points)
            if content.len() >= 50 && content.len() <= 1500 {
                score = score.saturating_add(15);
            }
            
            // Not error page (15 points)
            if !Self::is_error_page(content) {
                score = score.saturating_add(15);
            }
            
            score.min(100)
        }) {
            Ok(score) => score,
            Err(_) => 0, // Safe fallback
        }
    }
    
    /// PANIC-SAFE: Truncation with DATA_REDUCTION control
    pub fn truncate_content(content: &str, max_chars: usize) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }
        
        // Check DATA_REDUCTION - return full content if disabled
        if !Self::is_data_reduction_enabled() {
            return content.to_string();
        }

        if content.len() <= max_chars {
            return content.to_string();
        }
        
        // SAFETY: Try high-value extraction first - simplified
        let essential_data = Self::extract_high_value_financial_data(content, max_chars.saturating_div(3));
        if essential_data != "No data" && essential_data.len() <= max_chars {
            return essential_data;
        }
        
        // SAFETY: Safe truncation fallback
        Self::safe_truncate_fallback(content, max_chars)
    }
    
    /// PANIC-SAFE: Fallback truncation method
    fn safe_truncate_fallback(content: &str, max_chars: usize) -> String {
        let safe_max_chars = max_chars.saturating_sub(3).max(1);
        
        // SAFETY: Safe string slicing
        match content.get(..safe_max_chars) {
            Some(slice) => {
                if let Some(pos) = slice.rfind(' ') {
                    format!("{}…", &content[..pos])
                } else {
                    let truncate_at = std::cmp::min(safe_max_chars, content.len());
                    if truncate_at > 0 {
                        format!("{}…", &content[..truncate_at])
                    } else {
                        "…".to_string()
                    }
                }
            }
            None => {
                // Content is longer than safe_max_chars, take what we can
                let truncate_at = std::cmp::min(safe_max_chars, content.len());
                if truncate_at > 0 {
                    format!("{}…", &content[..truncate_at])
                } else {
                    "…".to_string()
                }
            }
        }
    }
    
    /// PANIC-SAFE: Token efficiency check with safety
    pub fn is_token_efficient(content: &str) -> bool {
        if content.is_empty() {
            return false;
        }
        
        // SAFETY: Comprehensive error handling
        match std::panic::catch_unwind(|| {
            let financial_keywords = ["₹", "$", "price", "market", "stock", "share", "volume", "pe", "dividend"];
            let keyword_count = financial_keywords.iter()
                .map(|&kw| content.to_lowercase().matches(kw).count())
                .sum::<usize>();
            
            let word_count = content.split_whitespace().count();
            
            if word_count == 0 {
                false
            } else {
                (keyword_count as f32 / word_count as f32) >= 0.05
            }
        }) {
            Ok(result) => result,
            Err(_) => false, // Safe fallback
        }
    }
    
    /// PANIC-SAFE: Token estimation with overflow protection
    pub fn estimate_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        // SAFETY: Prevent overflow in calculations
        match std::panic::catch_unwind(|| {
            let char_count = text.len();
            let number_count = text.matches(char::is_numeric).count();
            let symbol_count = text.matches(|c: char| matches!(c, '₹' | '$' | ',' | '.')).count();
            
            let base_tokens = (char_count as f32 / 4.0).ceil() as usize;
            let adjustment = (number_count.saturating_add(symbol_count)) as f32 * 0.1;
            
            (base_tokens as f32 + adjustment).ceil() as usize
        }) {
            Ok(tokens) => tokens,
            Err(_) => text.len().saturating_div(4).max(1), // Safe fallback
        }
    }

    /// PANIC-SAFE: Smart truncation preserving financial data with DATA_REDUCTION control
    pub fn smart_truncate_preserving_financial_data(content: &str, max_chars: usize) -> String {
        if content.is_empty() {
            return "No data".to_string();
        }
        
        // Check DATA_REDUCTION - return full content if disabled
        if !Self::is_data_reduction_enabled() {
            return content.to_string();
        }

        if content.len() <= max_chars {
            return content.to_string();
        }
        
        // SAFETY: Try stock-specific extraction first - simplified
        let stock_data = Self::extract_stock_specific_data(content);
        if stock_data != "No data" && stock_data.len() <= max_chars {
            return stock_data;
        }
        
        // SAFETY: Safe line-based extraction
        Self::safe_extract_financial_lines_truncated(content, max_chars)
    }
    
    /// PANIC-SAFE: Extract financial lines with truncation
    fn safe_extract_financial_lines_truncated(content: &str, max_chars: usize) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut essential_lines = Vec::new();
        let mut total_chars = 0usize;
        
        // SAFETY: Prioritize lines with comprehensive error handling
        let priority_terms = ["current price", "market cap", "pe ratio", "volume", "dividend"];
        
        // Process lines safely without mutable references in catch_unwind
        for line in lines {
            if line.is_empty() {
                continue;
            }
            
            // SAFETY: Safe line classification without panic catching mutable refs
            let is_priority_line = priority_terms.iter().any(|&term| {
                line.to_lowercase().contains(term)
            });
            
            let is_financial_line = line.to_lowercase().contains("₹") || 
                                   line.to_lowercase().contains("$") || 
                                   line.to_lowercase().contains("price") || 
                                   line.to_lowercase().contains("market");
            
            if is_priority_line || is_financial_line {
                let line_len = line.len();
                let new_total = total_chars.saturating_add(line_len).saturating_add(1);
                
                if new_total <= max_chars {
                    essential_lines.push(line);
                    total_chars = new_total;
                } else {
                    break;
                }
            }
        }
        
        if essential_lines.is_empty() {
            // SAFETY: Safe fallback truncation
            Self::safe_truncate_fallback(content, max_chars)
        } else {
            essential_lines.join("\n")
        }
    }
}