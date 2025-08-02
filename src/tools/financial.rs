// src/tools/financial.rs - Cleaned up version
use crate::tool::{Tool, ToolResult, McpContent};
use crate::error::BrightDataError;
use async_trait::async_trait;
use serde_json::{json, Value};
use reqwest::Client;
use std::time::Duration;
use log::{info, warn, error};
use chrono::Utc;

// Shared BrightData client for all financial tools
async fn scrape_via_brightdata(url: &str, query: &str) -> Result<Value, BrightDataError> {
    info!("🌐 BrightData request: {}", url);
    
    let api_token = std::env::var("BRIGHTDATA_API_TOKEN")
        .or_else(|_| std::env::var("API_TOKEN"))
        .map_err(|_| BrightDataError::ToolError("Missing BRIGHTDATA_API_TOKEN".into()))?;

    let base_url = std::env::var("BRIGHTDATA_BASE_URL")
        .unwrap_or_else(|_| "https://api.brightdata.com".to_string());

    let zone = std::env::var("WEB_UNLOCKER_ZONE")
        .unwrap_or_else(|_| "default".to_string());

    let payload = json!({
        "url": url,
        "zone": zone,
        "format": "raw",
        "data_format": "markdown"
    });

    let client = Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

    let response = client
        .post(&format!("{}/request", base_url))
        .header("Authorization", format!("Bearer {}", api_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            error!("❌ Request failed: {}", e);
            BrightDataError::ToolError(format!("Request failed: {}", e))
        })?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        error!("❌ HTTP Error {}: {}", status, error_text);
        return Err(BrightDataError::ToolError(format!(
            "HTTP error {}: {}", status, error_text
        )));
    }

    let content = response.text().await
        .map_err(|e| BrightDataError::ToolError(e.to_string()))?;

    info!("✅ Request completed - {} chars", content.len());
    
    Ok(json!({
        "content": content,
        "url": url,
        "query": query,
        "success": true,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

// Stock Data Tool
pub struct StockDataTool;

#[async_trait]
impl Tool for StockDataTool {
    fn name(&self) -> &str {
        "get_stock_data"
    }

    fn description(&self) -> &str {
        "Get comprehensive stock data including prices, performance, market cap, volumes"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Stock symbol, company name, comparison query, or market overview request"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        info!("📊 Stock query: '{}' (market: {})", query, market);

        let url = self.build_stock_url(query, market);
        let result = scrape_via_brightdata(&url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "📈 **Stock Data for: {}**\n\nMarket: {}\n\n{}",
            query,
            market,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl StockDataTool {
    fn build_stock_url(&self, query: &str, market: &str) -> String {
        let query_lower = query.to_lowercase();
        let timestamp = Utc::now().timestamp();
        
        // Market overview queries
        if query_lower.contains("market") || query_lower.contains("overview") {
            return match market {
                "us" => format!("https://finance.yahoo.com/markets/stocks?t={}", timestamp),
                "indian" => format!("https://www.moneycontrol.com/markets/indian-indices/?t={}", timestamp),
                _ => format!("https://finance.yahoo.com/world-indices?t={}", timestamp),
            };
        }
        
        // Specific company mappings
        if query_lower.contains("ashok leyland") || query_lower.contains("ashokley") {
            return format!("https://finance.yahoo.com/quote/ASHOKLEY.NS?t={}", timestamp);
        }
        if query_lower.contains("tcs") || query_lower.contains("tata consultancy") {
            return format!("https://finance.yahoo.com/quote/TCS.NS?t={}", timestamp);
        }
        if query_lower.contains("reliance") {
            return format!("https://finance.yahoo.com/quote/RELIANCE.NS?t={}", timestamp);
        }
        
        // Symbol-based lookup
        let clean_query = query.to_uppercase();
        match market {
            "us" => format!("https://finance.yahoo.com/quote/{}?t={}", clean_query, timestamp),
            "indian" => format!("https://finance.yahoo.com/quote/{}.NS?t={}", clean_query, timestamp),
            _ => format!("https://finance.yahoo.com/quote/{}?t={}", clean_query, timestamp),
        }
    }
}

// Crypto Data Tool
pub struct CryptoDataTool;

#[async_trait]
impl Tool for CryptoDataTool {
    fn name(&self) -> &str {
        "get_crypto_data"
    }

    fn description(&self) -> &str {
        "Get cryptocurrency data including prices, market cap, trading volumes"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Crypto symbol, name, comparison query, or market overview"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        info!("💰 Crypto query: '{}'", query);

        let url = self.build_crypto_url(query);
        let result = scrape_via_brightdata(&url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "💰 **Crypto Data for: {}**\n\n{}",
            query,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl CryptoDataTool {
    fn build_crypto_url(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();
        let timestamp = Utc::now().timestamp();
        
        if query_lower.contains("bitcoin") || query_lower.contains("btc") {
            format!("https://coinmarketcap.com/currencies/bitcoin/?t={}", timestamp)
        } else if query_lower.contains("ethereum") || query_lower.contains("eth") {
            format!("https://coinmarketcap.com/currencies/ethereum/?t={}", timestamp)
        } else if query_lower.contains("market") || query_lower.contains("overview") {
            format!("https://coinmarketcap.com/?t={}", timestamp)
        } else {
            format!("https://coinmarketcap.com/search/?q={}&t={}", urlencoding::encode(query), timestamp)
        }
    }
}

// ETF Data Tool
pub struct ETFDataTool;

#[async_trait]
impl Tool for ETFDataTool {
    fn name(&self) -> &str {
        "get_etf_data"
    }

    fn description(&self) -> &str {
        "Get ETF and index fund data including NAV, holdings, performance, expense ratios"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "ETF symbol, name, or ETF market analysis query"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let url = self.build_etf_url(query, market);
        let result = scrape_via_brightdata(&url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "📊 **ETF Data for: {}**\n\nMarket: {}\n\n{}",
            query,
            market,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

impl ETFDataTool {
    fn build_etf_url(&self, query: &str, market: &str) -> String {
        let timestamp = Utc::now().timestamp();
        match market {
            "us" => format!("https://finance.yahoo.com/quote/{}?t={}", query, timestamp),
            "indian" => format!("https://finance.yahoo.com/quote/{}.NS?t={}", query, timestamp),
            _ => format!("https://finance.yahoo.com/quote/{}?t={}", query, timestamp),
        }
    }
}

// Bond Data Tool
pub struct BondDataTool;

#[async_trait]
impl Tool for BondDataTool {
    fn name(&self) -> &str {
        "get_bond_data"
    }

    fn description(&self) -> &str {
        "Get bond market data including yields, government bonds, corporate bonds, and bond market trends"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Bond type, yield query, or bond market analysis"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let market = parameters
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let url = format!("https://www.investing.com/rates-bonds/?t={}", Utc::now().timestamp());
        let result = scrape_via_brightdata(&url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "🏛️ **Bond Data for: {}**\n\nMarket: {}\n\n{}",
            query,
            market,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

// Mutual Fund Data Tool
pub struct MutualFundDataTool;

#[async_trait]
impl Tool for MutualFundDataTool {
    fn name(&self) -> &str {
        "get_mutual_fund_data"
    }

    fn description(&self) -> &str {
        "Get mutual fund data including NAV, performance, portfolio composition, and fund comparisons"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Fund name, symbol, category, or fund comparison query"
                },
                "market": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let url = format!("https://www.valueresearchonline.com/funds/?t={}", Utc::now().timestamp());
        let result = scrape_via_brightdata(&url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "💼 **Mutual Fund Data for: {}**\n\n{}",
            query,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

// Commodity Data Tool
pub struct CommodityDataTool;

#[async_trait]
impl Tool for CommodityDataTool {
    fn name(&self) -> &str {
        "get_commodity_data"
    }

    fn description(&self) -> &str {
        "Get commodity prices and market data including gold, silver, oil, agricultural commodities"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Commodity name, symbol, or commodity market overview"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrightDataError::ToolError("Missing 'query' parameter".into()))?;

        let url = format!("https://www.investing.com/commodities/?t={}", Utc::now().timestamp());
        let result = scrape_via_brightdata(&url, query).await?;

        let mcp_content = vec![McpContent::text(format!(
            "🥇 **Commodity Data for: {}**\n\n{}",
            query,
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}

// Market Overview Tool
pub struct MarketOverviewTool;

#[async_trait]
impl Tool for MarketOverviewTool {
    fn name(&self) -> &str {
        "get_market_overview"
    }

    fn description(&self) -> &str {
        "Get comprehensive market overview including major indices, sector performance, market sentiment, and overall market trends"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "market_type": {
                    "type": "string",
                    "enum": ["stocks", "crypto", "bonds", "commodities", "overall"],
                    "default": "overall",
                    "description": "Type of market overview"
                },
                "region": {
                    "type": "string",
                    "enum": ["indian", "us", "global"],
                    "default": "indian"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, BrightDataError> {
        let market_type = parameters
            .get("market_type")
            .and_then(|v| v.as_str())
            .unwrap_or("overall");

        let region = parameters
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("indian");

        let url = format!("https://www.moneycontrol.com/?t={}", Utc::now().timestamp());
        let result = scrape_via_brightdata(&url, &format!("{} market overview", market_type)).await?;

        let mcp_content = vec![McpContent::text(format!(
            "🌍 **Market Overview: {} Market ({})**\n\n{}",
            market_type.to_uppercase(),
            region.to_uppercase(),
            result.get("content").and_then(|c| c.as_str()).unwrap_or("No data")
        ))];

        Ok(ToolResult::success_with_raw(mcp_content, result))
    }
}