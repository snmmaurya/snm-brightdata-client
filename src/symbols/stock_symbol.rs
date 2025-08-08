// SNM/crates/snm-brightdata-client/src/symbols/stock_symbol.rs

use std::collections::HashMap;

/// Returns a static map of known stock-related names to Yahoo Finance-compatible symbols.
fn known_name_to_symbol_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("ather", "ATHERENERG"),
        ("tata motors", "TATAMOTORS"),
        ("reliance", "RELIANCE"),
        ("infosys", "INFY"),
        ("hdfc bank", "HDFCBANK"),
        ("icici bank", "ICICIBANK"),
        ("sbi", "SBIN"),
        ("hcl", "HCLTECH"),
        ("maruti", "MARUTI"),
        ("l&t", "LT"),
        ("zomato", "ZOMATO"),
        ("paytm", "PAYTM"),
        ("flipkart", "WMT"), // Walmart owns Flipkart
        ("nykaa", "NYKAA"),
        ("ola", "OLACABS"), // not public yet
        ("mahindra", "M&M"),
        ("zomato", "ETERNAL"),
        ("infy", "INFOSYS"),
        ("suven", "COHANCE")
    ])
}

/// Attempts to match a known name in the query string to a known Yahoo Finance symbol.
/// If no match is found, returns the original query string as a fallback.
///
/// # Arguments
///
/// * `query` - A string slice that may contain a company or stock name.
///
/// # Returns
///
/// * A `String` containing the matched symbol or the original query if no match is found.
pub fn match_symbol_from_query(query: &str) -> String {
    let lower_query = query.to_lowercase();
    for (name, symbol) in known_name_to_symbol_map() {
        if lower_query.contains(name) {
            return symbol.to_string();
        }
    }
    // fallback: return the original query if no match
    query.to_string()
}
