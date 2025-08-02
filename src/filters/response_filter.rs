// src/filters/response_filter.rs - Ultra-compact version with optional filtering
use regex::Regex;
use std::sync::OnceLock;

static FINANCIAL_REGEX: OnceLock<Regex> = OnceLock::new();
static NUMBER_REGEX: OnceLock<Regex> = OnceLock::new();
static ESSENTIAL_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseFilter;

impl ResponseFilter {
    /// Check if truncate filtering is enabled via environment variable
    pub fn is_truncate_filter_enabled() -> bool {
        std::env::var("TRUNCATE_FILTER")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false)
    }

    /// HYPER-aggressive financial content filtering - extract only essential numbers
    pub fn filter_financial_content(content: &str) -> String {
        if !Self::is_truncate_filter_enabled() {
            return content.to_string();
        }

        // First try to extract structured essential data
        let essential = Self::extract_only_essential_metrics(content);
        if !essential.is_empty() && essential != "No data" {
            return essential;
        }
        
        // Fallback to original method but more aggressive
        let financial_regex = FINANCIAL_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(₹|[$]|price|market cap|pe |eps |dividend|revenue)\s*:?\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let lines: Vec<&str> = content
            .lines()
            .filter(|line| {
                line.len() > 3 && line.len() < 50 && // Even shorter lines
                financial_regex.is_match(line)
            })
            .take(2) // Max 2 lines for hyper-compact
            .collect();
        
        if lines.is_empty() {
            Self::extract_any_numbers(content)
        } else {
            // Ultra-compact join
            lines.join("|")
        }
    }
    
    /// NEW: Extract only essential metrics (price, market cap, PE) in minimal format
    fn extract_only_essential_metrics(content: &str) -> String {
        let essential_regex = ESSENTIAL_REGEX.get_or_init(|| {
            Regex::new(r"(?i)(?:price|current|market\s*cap|pe\s*ratio?|volume)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|billion|b|k|m)?").unwrap()
        });
        
        let mut metrics = Vec::new();
        
        for cap in essential_regex.captures_iter(content).take(3) {
            let full_match = cap.get(0).unwrap().as_str().to_lowercase();
            let value = cap.get(1).map_or("", |m| m.as_str());
            let unit = cap.get(2).map_or("", |m| m.as_str());
            
            let metric = if full_match.contains("price") || full_match.contains("current") {
                format!("₹{}{}", value, unit)
            } else if full_match.contains("market") {
                format!("MC₹{}{}", value, unit)
            } else if full_match.contains("pe") {
                format!("PE{}", value)
            } else if full_match.contains("volume") {
                format!("V{}{}", value, unit)
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
    
    /// Extract any financial numbers - more aggressive
    fn extract_any_numbers(content: &str) -> String {
        let number_regex = NUMBER_REGEX.get_or_init(|| {
            Regex::new(r"[₹$]\s*([0-9,]+\.?[0-9]*)").unwrap()
        });
        
        let numbers: Vec<&str> = number_regex
            .find_iter(content)
            .map(|m| m.as_str())
            .take(3) // Reduced from 5 to 3
            .collect();
        
        if numbers.is_empty() {
            "No data".to_string()
        } else {
            numbers.join("|")
        }
    }
    
    /// Enhanced financial data check - more precise
    pub fn contains_financial_data(content: &str) -> bool {
        let content_lower = content.to_lowercase();
        let financial_indicators = content_lower.matches('₹').count() + 
                                 content_lower.matches('$').count() +
                                 content_lower.matches("price").count() +
                                 content_lower.matches("market cap").count();
        
        financial_indicators > 0
    }
    
    /// Quick error page check - more patterns
    pub fn is_error_page(content: &str) -> bool {
        let content_lower = content.to_lowercase();
        content_lower.contains("404") || 
        content_lower.contains("500") ||
        content_lower.contains("error") ||
        content_lower.contains("not found") ||
        content_lower.contains("access denied") ||
        content_lower.contains("forbidden")
    }
    
    /// Enhanced navigation check
    pub fn is_mostly_navigation(content: &str) -> bool {
        let content_lower = content.to_lowercase();
        let nav_words = content_lower.matches("menu").count() + 
                       content_lower.matches("navigation").count() +
                       content_lower.matches("sign in").count() +
                       content_lower.matches("login").count() +
                       content_lower.matches("register").count();
        
        let total_words = content.split_whitespace().count();
        
        if total_words > 0 {
            (nav_words as f32 / total_words as f32) > 0.25 // Slightly more lenient
        } else {
            false
        }
    }
    
    /// HYPER-aggressive truncation for token efficiency
    pub fn truncate_content(content: &str, max_chars: usize) -> String {
        if !Self::is_truncate_filter_enabled() {
            return content.to_string();
        }

        if content.len() <= max_chars {
            return content.to_string();
        }
        
        // For very small limits, be more aggressive
        let truncate_at = if max_chars < 100 {
            max_chars.saturating_sub(3)
        } else if let Some(pos) = content[..max_chars].rfind(' ') {
            pos
        } else {
            max_chars.saturating_sub(3)
        };
        
        format!("{}…", &content[..truncate_at])
    }
    
    /// Enhanced quality score with token efficiency bias
    pub fn get_content_quality_score(content: &str) -> u8 {
        let mut score = 10u8; // Lower base score
        
        // Financial data present (60 points - more weight)
        if Self::contains_financial_data(content) {
            score += 60;
        }
        
        // Token-efficient size (20 points)
        if content.len() >= 20 && content.len() <= 1000 { // Smaller range
            score += 20;
        }
        
        // Not error page (10 points)
        if !Self::is_error_page(content) {
            score += 10;
        }
        
        score.min(100)
    }
    
    /// NEW: Check if content is token-efficient
    pub fn is_token_efficient(content: &str) -> bool {
        let financial_keywords = ["₹", "$", "price", "market", "stock", "share"];
        let keyword_count = financial_keywords.iter()
            .map(|&kw| content.to_lowercase().matches(kw).count())
            .sum::<usize>();
        
        let word_count = content.split_whitespace().count();
        
        if word_count == 0 {
            return false;
        }
        
        // Must have at least 1 financial keyword per 30 words for efficiency
        (keyword_count as f32 / word_count as f32) >= 0.033
    }
    
    /// NEW: Estimate token count (rough approximation)
    pub fn estimate_tokens(text: &str) -> usize {
        // Conservative estimate: 1 token ≈ 3.5 characters for financial text
        (text.len() as f32 / 3.5).ceil() as usize
    }
    
    /// NEW: Smart financial data extraction prioritizing high-value info
    pub fn extract_high_value_financial_data(content: &str, max_tokens: usize) -> String {
        if !Self::is_truncate_filter_enabled() {
            return content.to_string();
        }

        let max_chars = max_tokens * 3; // 3 chars per token estimate
        
        // Priority order: Price > Market Cap > PE > Volume > Others
        let priority_patterns = [
            (r"(?i)(?:price|current)[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)", "₹{}"),
            (r"(?i)market\s*cap[\s:]*[₹$]?\s*([0-9,]+\.?[0-9]*)\s*(cr|b)?", "MC₹{}{}"),
            (r"(?i)pe?\s*ratio?[\s:]*([0-9,]+\.?[0-9]*)", "PE{}"),
            (r"(?i)volume[\s:]*([0-9,]+\.?[0-9]*)", "V{}"),
        ];
        
        let mut extracted = Vec::new();
        let mut total_len = 0;
        
        for (pattern, format_str) in &priority_patterns {
            if total_len >= max_chars {
                break;
            }
            
            if let Ok(regex) = Regex::new(pattern) {
                if let Some(caps) = regex.captures(content) {
                    let value = caps.get(1).map_or("", |m| m.as_str());
                    let unit = caps.get(2).map_or("", |m| m.as_str());
                    
                    let formatted = if unit.is_empty() {
                        format_str.replace("{}", value).replace("{}", "") // Remove extra {}
                    } else {
                        format_str.replace("{}", value).replace("{}", unit)
                    };
                    
                    if total_len + formatted.len() <= max_chars {
                        total_len += formatted.len() + 1; // +1 for separator
                        extracted.push(formatted);
                    }
                }
            }
        }
        
        if extracted.is_empty() {
            "No data".to_string()
        } else {
            extracted.join("|")
        }
    }

    /// NEW: Smart truncation that preserves financial data
    pub fn smart_truncate_preserving_financial_data(content: &str, max_chars: usize) -> String {
        if !Self::is_truncate_filter_enabled() {
            return content.to_string();
        }

        if content.len() <= max_chars {
            return content.to_string();
        }
        
        // Find financial data lines first
        let lines: Vec<&str> = content.lines().collect();
        let mut essential_lines = Vec::new();
        let mut total_chars = 0;
        
        for line in lines {
            let line_lower = line.to_lowercase();
            if line_lower.contains("₹") || line_lower.contains("$") || 
               line_lower.contains("price") || line_lower.contains("market") {
                if total_chars + line.len() <= max_chars {
                    essential_lines.push(line);
                    total_chars += line.len() + 1; // +1 for newline
                } else {
                    break;
                }
            }
        }
        
        if essential_lines.is_empty() {
            // Fallback: truncate normally but try to end at word boundary
            if let Some(pos) = content[..max_chars].rfind(' ') {
                format!("{}…", &content[..pos])
            } else {
                format!("{}…", &content[..max_chars.saturating_sub(1)])
            }
        } else {
            essential_lines.join("\n")
        }
    }
}