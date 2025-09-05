// /snm-brightdata-client/src/symbols/mutual_fund_symbol.rs

use std::collections::HashMap;

fn known_name_to_symbol_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
    ])
}

pub fn match_symbol_from_query(query: &str) -> String {
    let lower_query = query.to_lowercase();
    for (name, symbol) in known_name_to_symbol_map() {
        if lower_query.contains(name) {
            return symbol.to_string();
        }
    }
    query.to_string()
}
