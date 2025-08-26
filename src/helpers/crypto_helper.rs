// // snm-brightdata-client/src/helpers/crypto_helper.rs

// use std::collections::HashMap;
// use log::{info, debug};

// /// Match crypto symbol from query - maps common names to symbols
// pub fn match_crypto_from_query(query: &str) -> String {
//     let clean_query = query.trim().to_uppercase();
    
//     // Common crypto name to symbol mappings
//     let crypto_mappings = get_crypto_name_mappings();
    
//     // Check for exact name matches first
//     for (name, symbol) in &crypto_mappings {
//         if clean_query == *name {
//             debug!("Exact crypto name match: '{}' -> '{}'", query, symbol);
//             return symbol.to_string();
//         }
//     }
    
//     // Check for partial matches
//     for (name, symbol) in &crypto_mappings {
//         if clean_query.contains(name) || name.contains(&clean_query) {
//             debug!("Partial crypto name match: '{}' -> '{}'", query, symbol);
//             return symbol.to_string();
//         }
//     }
    
//     // If no match found, return original query (assume it's already a symbol)
//     debug!("No crypto name match found, using as symbol: '{}'", clean_query);
//     clean_query
// }

// /// Get comprehensive crypto name to symbol mappings
// pub fn get_crypto_name_mappings() -> HashMap<&'static str, &'static str> {
//     let mut mappings = HashMap::new();
    
//     // Major cryptocurrencies
//     mappings.insert("BITCOIN", "BTC");
//     mappings.insert("ETHEREUM", "ETH");
//     mappings.insert("CARDANO", "ADA");
//     mappings.insert("POLKADOT", "DOT");
//     mappings.insert("SOLANA", "SOL");
//     mappings.insert("POLYGON", "MATIC");
//     mappings.insert("AVALANCHE", "AVAX");
//     mappings.insert("CHAINLINK", "LINK");
//     mappings.insert("UNISWAP", "UNI");
//     mappings.insert("AAVE", "AAVE");
//     mappings.insert("RIPPLE", "XRP");
//     mappings.insert("LITECOIN", "LTC");
//     mappings.insert("BITCOIN CASH", "BCH");
//     mappings.insert("ETHEREUM CLASSIC", "ETC");
//     mappings.insert("DOGECOIN", "DOGE");
//     mappings.insert("SHIBA INU", "SHIB");
//     mappings.insert("COSMOS", "ATOM");
//     mappings.insert("TERRA LUNA", "LUNA");
//     mappings.insert("NEAR PROTOCOL", "NEAR");
//     mappings.insert("FANTOM", "FTM");
//     mappings.insert("ALGORAND", "ALGO");
//     mappings.insert("TEZOS", "XTZ");
//     mappings.insert("ELROND", "EGLD");
//     mappings.insert("FLOW", "FLOW");
//     mappings.insert("INTERNET COMPUTER", "ICP");
//     mappings.insert("VECHAIN", "VET");
//     mappings.insert("THETA", "THETA");
//     mappings.insert("FILECOIN", "FIL");
//     mappings.insert("TRON", "TRX");
//     mappings.insert("EOS", "EOS");
    
//     // Additional popular cryptos
//     mappings.insert("BINANCE COIN", "BNB");
//     mappings.insert("STELLAR", "XLM");
//     mappings.insert("MONERO", "XMR");
//     mappings.insert("DASH", "DASH");
//     mappings.insert("ZCASH", "ZEC");
//     mappings.insert("COMPOUND", "COMP");
//     mappings.insert("MAKER", "MKR");
//     mappings.insert("YEARN FINANCE", "YFI");
//     mappings.insert("SYNTHETIX", "SNX");
//     mappings.insert("CURVE DAO TOKEN", "CRV");
//     mappings.insert("SUSHISWAP", "SUSHI");
//     mappings.insert("1INCH", "1INCH");
//     mappings.insert("PANCAKESWAP", "CAKE");
//     mappings.insert("DECENTRALAND", "MANA");
//     mappings.insert("SANDBOX", "SAND");
//     mappings.insert("AXIE INFINITY", "AXS");
//     mappings.insert("ENJIN COIN", "ENJ");
//     mappings.insert("CHILIZ", "CHZ");
//     mappings.insert("BASIC ATTENTION TOKEN", "BAT");
//     mappings.insert("LOOPRING", "LRC");
    
//     // DeFi tokens
//     mappings.insert("WRAPPED BITCOIN", "WBTC");
//     mappings.insert("USD COIN", "USDC");
//     mappings.insert("TETHER", "USDT");
//     mappings.insert("BINANCE USD", "BUSD");
//     mappings.insert("DAI", "DAI");
//     mappings.insert("FRAX", "FRAX");
//     mappings.insert("TERRA USD", "UST");
    
//     mappings
// }

// /// Check if a query looks like a valid crypto symbol
// pub fn is_likely_crypto_symbol(query: &str) -> bool {
//     let clean = query.trim().to_uppercase();
    
//     if clean.len() < 1 || clean.len() > 10 {
//         return false;
//     }

//     // Known crypto symbols (most popular ones)
//     let known_cryptos = get_known_crypto_symbols();
    
//     // Check if it's a known crypto symbol
//     if known_cryptos.contains(&clean.as_str()) {
//         return true;
//     }
    
//     // Check pattern: alphanumeric, mostly uppercase, reasonable length
//     let valid_chars = clean.chars().all(|c| c.is_alphanumeric());
//     let has_letters = clean.chars().any(|c| c.is_alphabetic());
//     let reasonable_length = clean.len() >= 2 && clean.len() <= 6;
    
//     valid_chars && has_letters && reasonable_length
// }

// /// Get list of known crypto symbols
// pub fn get_known_crypto_symbols() -> Vec<&'static str> {
//     vec![
//         "BTC", "ETH", "ADA", "DOT", "SOL", "MATIC", "AVAX", "LINK", "UNI", "AAVE",
//         "XRP", "LTC", "BCH", "ETC", "DOGE", "SHIB", "ATOM", "LUNA", "NEAR", "FTM",
//         "ALGO", "XTZ", "EGLD", "FLOW", "ICP", "VET", "THETA", "FIL", "TRX", "EOS",
//         "BNB", "XLM", "XMR", "DASH", "ZEC", "COMP", "MKR", "YFI", "SNX", "CRV",
//         "SUSHI", "1INCH", "CAKE", "MANA", "SAND", "AXS", "ENJ", "CHZ", "BAT", "LRC",
//         "WBTC", "USDC", "USDT", "BUSD", "DAI", "FRAX", "UST"
//     ]
// }

// /// Generate crypto URL for different markets
// pub fn generate_crypto_url(symbol: &str, market: &str) -> String {
//     let clean_symbol = symbol.trim().to_uppercase();
    
//     let symbol_with_pair = match market.to_lowercase().as_str() {
//         "usd" => format!("{}-USD", clean_symbol),
//         "btc" => format!("{}-BTC", clean_symbol),
//         "eth" => format!("{}-ETH", clean_symbol),
//         _ => format!("{}-USD", clean_symbol), // Default to USD
//     };
    
//     format!("https://finance.yahoo.com/quote/{}/", symbol_with_pair)
// }

// /// Get alternative crypto URL patterns for popular cryptos
// pub fn get_crypto_url_alternatives(symbol: &str, market: &str) -> Vec<(String, String)> {
//     let clean_symbol = symbol.trim().to_uppercase();
//     let mut alternatives = Vec::new();
    
//     // Primary URL
//     let primary_url = generate_crypto_url(&clean_symbol, market);
//     let primary_desc = format!("Yahoo Finance ({})", 
//         if market == "usd" { format!("{}-USD", clean_symbol) } else { format!("{}-{}", clean_symbol, market.to_uppercase()) }
//     );
//     alternatives.push((primary_url, primary_desc));
    
//     // Add alternative patterns for popular cryptos
//     match clean_symbol.as_str() {
//         "BTC" | "BITCOIN" => {
//             if !alternatives.iter().any(|(url, _)| url.contains("BTC-USD")) {
//                 alternatives.push((
//                     "https://finance.yahoo.com/quote/BTC-USD/".to_string(),
//                     "Yahoo Finance (BTC-USD)".to_string()
//                 ));
//             }
//         }
//         "ETH" | "ETHEREUM" => {
//             if !alternatives.iter().any(|(url, _)| url.contains("ETH-USD")) {
//                 alternatives.push((
//                     "https://finance.yahoo.com/quote/ETH-USD/".to_string(),
//                     "Yahoo Finance (ETH-USD)".to_string()
//                 ));
//             }
//         }
//         "ADA" | "CARDANO" => {
//             alternatives.push((
//                 "https://finance.yahoo.com/quote/ADA-USD/".to_string(),
//                 "Yahoo Finance (ADA-USD)".to_string()
//             ));
//         }
//         "DOT" | "POLKADOT" => {
//             alternatives.push((
//                 "https://finance.yahoo.com/quote/DOT-USD/".to_string(),
//                 "Yahoo Finance (DOT-USD)".to_string()
//             ));
//         }
//         "SOL" | "SOLANA" => {
//             alternatives.push((
//                 "https://finance.yahoo.com/quote/SOL-USD/".to_string(),
//                 "Yahoo Finance (SOL-USD)".to_string()
//             ));
//         }
//         _ => {}
//     }
    
//     // Limit alternatives to avoid too many requests
//     alternatives.truncate(3);
    
//     alternatives
// }

// /// Validate crypto market pair
// pub fn validate_crypto_market(market: &str) -> String {
//     match market.to_lowercase().as_str() {
//         "usd" | "btc" | "eth" | "global" => market.to_lowercase(),
//         _ => "usd".to_string(), // Default fallback
//     }
// }

// /// Get crypto market description
// pub fn get_crypto_market_description(market: &str) -> &str {
//     match market.to_lowercase().as_str() {
//         "usd" => "USD pairs",
//         "btc" => "BTC pairs", 
//         "eth" => "ETH pairs",
//         "global" => "Global overview",
//         _ => "USD pairs",
//     }
// }

// /// Extract crypto data quality indicators from content
// pub fn assess_crypto_content_quality(content: &str, symbol: &str) -> CryptoContentQuality {
//     let content_lower = content.to_lowercase();
//     let symbol_lower = symbol.to_lowercase();
    
//     let mut quality = CryptoContentQuality::default();
    
//     // Check for symbol presence
//     quality.has_symbol = content_lower.contains(&symbol_lower);
    
//     // Check for price information
//     quality.has_price = content_lower.contains("price") || 
//                        content_lower.contains("$") || 
//                        content_lower.contains("usd");
    
//     // Check for market cap
//     quality.has_market_cap = content_lower.contains("market cap") || 
//                             content_lower.contains("market capitalization");
    
//     // Check for volume
//     quality.has_volume = content_lower.contains("volume") || 
//                         content_lower.contains("trading volume");
    
//     // Check for change/movement data
//     quality.has_change_data = content_lower.contains("%") || 
//                              content_lower.contains("change") || 
//                              content_lower.contains("up") || 
//                              content_lower.contains("down");
    
//     // Calculate overall quality score
//     let indicators = [
//         quality.has_symbol,
//         quality.has_price,
//         quality.has_market_cap,
//         quality.has_volume,
//         quality.has_change_data,
//     ];
    
//     quality.quality_score = indicators.iter().filter(|&&x| x).count() as f32 / indicators.len() as f32;
//     quality.content_length = content.len();
    
//     debug!("Crypto content quality for {}: score={:.2}, length={}, indicators={:?}", 
//            symbol, quality.quality_score, quality.content_length, indicators);
    
//     quality
// }

// /// Crypto content quality assessment
// #[derive(Debug, Default, Clone)]
// pub struct CryptoContentQuality {
//     pub has_symbol: bool,
//     pub has_price: bool,
//     pub has_market_cap: bool,
//     pub has_volume: bool,
//     pub has_change_data: bool,
//     pub quality_score: f32, // 0.0 to 1.0
//     pub content_length: usize,
// }

// impl CryptoContentQuality {
//     /// Check if content quality is sufficient for crypto data
//     pub fn is_sufficient(&self) -> bool {
//         self.quality_score >= 0.4 && self.content_length > 100
//     }
    
//     /// Check if content is high quality
//     pub fn is_high_quality(&self) -> bool {
//         self.quality_score >= 0.8 && self.content_length > 500
//     }
// }

// /// Generate search fallback URL for crypto
// pub fn generate_crypto_search_url(query: &str) -> (String, String) {
//     let encoded_query = urlencoding::encode(query);
//     let url = format!("https://finance.yahoo.com/quote/{}-USD/", encoded_query);
//     let description = "Yahoo Finance Crypto Search".to_string();
    
//     (url, description)
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_crypto_symbol_matching() {
//         assert_eq!(match_crypto_from_query("bitcoin"), "BTC");
//         assert_eq!(match_crypto_from_query("ETHEREUM"), "ETH");
//         assert_eq!(match_crypto_from_query("BTC"), "BTC");
//         assert_eq!(match_crypto_from_query("cardano"), "ADA");
//     }

//     #[test]
//     fn test_crypto_symbol_validation() {
//         assert!(is_likely_crypto_symbol("BTC"));
//         assert!(is_likely_crypto_symbol("ETH"));
//         assert!(!is_likely_crypto_symbol(""));
//         assert!(!is_likely_crypto_symbol("TOOLONGFORCRYPTO"));
//     }

//     #[test]
//     fn test_crypto_url_generation() {
//         assert_eq!(generate_crypto_url("BTC", "usd"), "https://finance.yahoo.com/quote/BTC-USD/");
//         assert_eq!(generate_crypto_url("eth", "btc"), "https://finance.yahoo.com/quote/ETH-BTC/");
//     }

//     #[test]
//     fn test_market_validation() {
//         assert_eq!(validate_crypto_market("USD"), "usd");
//         assert_eq!(validate_crypto_market("invalid"), "usd");
//         assert_eq!(validate_crypto_market("BTC"), "btc");
//     }
// }