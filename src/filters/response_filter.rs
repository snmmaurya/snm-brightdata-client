// src/filters/response_filter.rs
use regex::Regex;
use std::sync::OnceLock;

static NAV_REGEX: OnceLock<Regex> = OnceLock::new();
static HEADER_REGEX: OnceLock<Regex> = OnceLock::new();
static FOOTER_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct ResponseFilter;

impl ResponseFilter {
    /// Filter out navigation and boilerplate content from financial data
    pub fn filter_financial_content(content: &str) -> String {
        let mut filtered = content.to_string();
        
        // Remove common navigation patterns
        filtered = Self::remove_navigation_elements(&filtered);
        
        // Extract only financial sections
        Self::extract_financial_sections(&filtered)
    }
    
    /// Remove navigation, header, footer elements
    fn remove_navigation_elements(content: &str) -> String {
        let mut filtered = content.to_string();
        
        // Initialize regexes once
        let nav_regex = NAV_REGEX.get_or_init(|| {
            Regex::new(r"(?s)<nav[^>]*>.*?</nav>|Skip to main content|Skip to navigation|Accessibility help").unwrap()
        });
        
        let header_regex = HEADER_REGEX.get_or_init(|| {
            Regex::new(r"(?s)<header[^>]*>.*?</header>|Sign in.*?Sign up").unwrap()
        });
        
        let footer_regex = FOOTER_REGEX.get_or_init(|| {
            Regex::new(r"(?s)<footer[^>]*>.*?</footer>|Privacy.*?Terms|Copyright.*?All rights reserved").unwrap()
        });
        
        // Apply filters
        filtered = nav_regex.replace_all(&filtered, "").to_string();
        filtered = header_regex.replace_all(&filtered, "").to_string();
        filtered = footer_regex.replace_all(&filtered, "").to_string();
        
        // Remove excessive whitespace
        let whitespace_regex = Regex::new(r"\s{3,}").unwrap();
        whitespace_regex.replace_all(&filtered, "\n\n").to_string()
    }
    
    /// Extract sections containing financial data
    fn extract_financial_sections(content: &str) -> String {
        let financial_keywords = [
            "stock price", "market cap", "volume", "pe ratio", "eps", 
            "dividend", "revenue", "profit", "sales", "ebitda",
            "current price", "52 week", "book value", "roce", "roe",
            "₹", "$", "price", "trading", "shares", "fundamentals"
        ];
        
        let lines: Vec<&str> = content.lines().collect();
        let _relevant_sections: Vec<&str> = Vec::new();
        let mut context_buffer: Vec<&str> = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let is_financial = financial_keywords.iter()
                .any(|keyword| line_lower.contains(keyword));
            
            if is_financial {
                // Add previous context lines
                let start = i.saturating_sub(2);
                for j in start..i {
                    if !context_buffer.contains(&lines[j]) {
                        context_buffer.push(lines[j]);
                    }
                }
                
                // Add current line
                context_buffer.push(line);
                
                // Add next context lines
                let end = std::cmp::min(i + 3, lines.len());
                for j in (i + 1)..end {
                    context_buffer.push(lines[j]);
                }
            }
        }
        
        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        let unique_lines: Vec<&str> = context_buffer.into_iter()
            .filter(|line| seen.insert(*line))
            .collect();
        
        if !unique_lines.is_empty() {
            unique_lines.join("\n")
        } else {
            // No financial data found, return truncated original
            Self::truncate_content(content, 1500)
        }
    }
    
    /// Check if response contains actual financial data
    pub fn contains_financial_data(content: &str) -> bool {
        let content_lower = content.to_lowercase();
        let financial_indicators = [
            "₹", "$", "price", "market cap", "volume", "eps", "pe ratio",
            "revenue", "profit", "dividend", "stock", "share", "trading",
            "fundamentals", "roce", "roe", "book value"
        ];
        
        let financial_count = financial_indicators.iter()
            .map(|indicator| content_lower.matches(indicator).count())
            .sum::<usize>();
        
        // Must have at least 3 financial terms
        financial_count >= 3
    }
    
    /// Check if this is an error page
    pub fn is_error_page(content: &str) -> bool {
        let error_indicators = [
            "502 bad gateway", "404 not found", "503 service unavailable",
            "500 internal server error", "oops, something went wrong", 
            "no results for", "error occurred", "temporarily unavailable",
            "service temporarily unavailable"
        ];
        
        let content_lower = content.to_lowercase();
        error_indicators.iter().any(|error| content_lower.contains(error))
    }
    
    /// Check if content is mostly navigation/boilerplate
    pub fn is_mostly_navigation(content: &str) -> bool {
        let nav_indicators = [
            "skip to main content", "skip to navigation", "sign in", "sign up",
            "privacy policy", "terms of service", "cookie policy", "about us",
            "help", "support", "contact", "careers"
        ];
        
        let content_lower = content.to_lowercase();
        let nav_count = nav_indicators.iter()
            .map(|indicator| content_lower.matches(indicator).count())
            .sum::<usize>();
        
        let total_words = content.split_whitespace().count();
        
        // If more than 20% of content is navigation terms, consider it mostly navigation
        if total_words > 0 {
            (nav_count as f32 / total_words as f32) > 0.2
        } else {
            false
        }
    }
    
    /// Truncate content to specified length with word boundary
    pub fn truncate_content(content: &str, max_chars: usize) -> String {
        if content.len() <= max_chars {
            return content.to_string();
        }
        
        // Try to truncate at word boundary
        if let Some(pos) = content[..max_chars].rfind(' ') {
            format!("{}...[truncated {} chars]", &content[..pos], content.len() - pos)
        } else {
            format!("{}...[truncated]", &content[..max_chars])
        }
    }
    
    /// Get quality score for content (0-100)
    pub fn get_content_quality_score(content: &str) -> u8 {
        let mut score = 0u8;
        
        // Check for financial data (40 points max)
        if Self::contains_financial_data(content) {
            score += 40;
        }
        
        // Check size (20 points max)
        let size_score = match content.len() {
            0..=100 => 0,
            101..=1000 => 10,
            1001..=5000 => 20,
            5001..=20000 => 15,
            _ => 5, // Too large
        };
        score += size_score;
        
        // Check if it's not an error page (20 points)
        if !Self::is_error_page(content) {
            score += 20;
        }
        
        // Check if it's not mostly navigation (20 points)
        if !Self::is_mostly_navigation(content) {
            score += 20;
        }
        
        score
    }
}