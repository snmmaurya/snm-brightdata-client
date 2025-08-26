// /snm-brightdata-client/src/symbols/forex_symbol.rs

use std::collections::HashMap;

/// Returns a static map of known stock-related names to Yahoo Finance-compatible symbols.
fn known_name_to_symbol_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("USDINR=X", "USDINR")
    ])
}

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
