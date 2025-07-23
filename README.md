# 🌐 SNM BrightData Client

A powerful Rust crate providing MCP-compatible integration with BrightData's web scraping and data extraction services. Built with Actix Web for high-performance web scraping, search, data extraction, and screenshot capabilities.

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Maintenance](https://img.shields.io/badge/Maintained%3F-yes-green.svg)](https://github.com/snmmaurya/snm-brightdata-client)

## ✨ Features

- 🔍 **Web Search**: Search across Google, Bing, Yandex, and DuckDuckGo
- 🌐 **Website Scraping**: Extract content in markdown, raw HTML, or structured formats
- 📊 **Data Extraction**: Intelligent data extraction from any webpage
- 📸 **Screenshots**: Capture website screenshots using BrightData Browser
- 🤖 **MCP Compatible**: Full Model Context Protocol support for AI integrations
- ⚡ **Multiple Interfaces**: Library, CLI, and HTTP server
- 🔒 **Authentication**: Secure token-based authentication
- 📈 **Rate Limiting**: Built-in rate limiting and error handling
- 🚀 **High Performance**: Built with Actix Web for production workloads

## 🚀 Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
snm-brightdata-client = "0.1.0"
```

### Environment Setup

```bash
# BrightData Configuration
export BRIGHTDATA_API_TOKEN="your_api_token"
export BRIGHTDATA_BASE_URL="https://api.brightdata.com"
export WEB_UNLOCKER_ZONE="your_zone_name"
export BROWSER_ZONE="your_browser_zone"

# Proxy Credentials (optional)
export BRIGHTDATA_PROXY_USERNAME="your_username"
export BRIGHTDATA_PROXY_PASSWORD="your_password"

# Server Configuration
export MCP_AUTH_TOKEN="your_secure_token"
export PORT="8080"
```

## 📖 Usage

### As a Library

```rust
use snm_brightdata_client::{BrightDataClient, BrightDataConfig};
use snm_brightdata_client::tool::{ToolResolver, Tool};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let config = BrightDataConfig::from_env()?;
    let client = BrightDataClient::new(config);

    // Use tools directly
    let resolver = ToolResolver::default();
    let search_tool = resolver.resolve("search_web").unwrap();
    
    let result = search_tool.execute(json!({
        "query": "Rust programming language",
        "engine": "google"
    })).await?;

    println!("Search results: {:#?}", result);
    Ok(())
}
```

### CLI Usage

```bash
# Search the web
snm_cli search "Bitcoin price today" --engine google

# Scrape a website
snm_cli scrape https://example.com --format markdown

# Extract data
snm_cli extract https://example.com --format json

# Take screenshot
snm_cli screenshot https://example.com --width 1920 --height 1080
```

### HTTP Server

```bash
# Start the server
cargo run --bin snm_server

# Health check
curl http://localhost:8080/health

# List available tools
curl http://localhost:8080/tools

# Use tools via API
curl -X POST http://localhost:8080/invoke \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "tool": "search_web",
    "parameters": {
      "query": "Rust web scraping",
      "engine": "google"
    }
  }'
```

## 🛠️ Available Tools

### 🔍 Search Web (`search_web`)

Search across multiple search engines with BrightData's unblocking capabilities.

```json
{
  "tool": "search_web",
  "parameters": {
    "query": "your search query",
    "engine": "google"  // google, bing, yandex, duckduckgo
  }
}
```

### 🌐 Scrape Website (`scrape_website`)

Extract content from any website, bypassing anti-bot protections.

```json
{
  "tool": "scrape_website",
  "parameters": {
    "url": "https://example.com",
    "format": "markdown"  // raw, markdown
  }
}
```

### 📊 Extract Data (`extract_data`)

Intelligent data extraction from webpages.

```json
{
  "tool": "extract_data",
  "parameters": {
    "url": "https://example.com"
  }
}
```

### 📸 Take Screenshot (`take_screenshot`)

Capture high-quality screenshots of websites.

```json
{
  "tool": "take_screenshot",
  "parameters": {
    "url": "https://example.com"
  }
}
```

## 🤖 MCP Integration

This crate is fully compatible with the Model Context Protocol (MCP), making it easy to integrate with AI systems like Claude.

### MCP Server Configuration

```json
{
  "type": "url",
  "url": "https://your-server.com/sse",
  "name": "brightdata-mcp",
  "authorization_token": "your_token",
  "tool_configuration": {
    "enabled": true,
    "allowed_tools": ["search_web", "scrape_website", "extract_data", "take_screenshot"]
  }
}
```

### Example with Claude

```bash
curl https://api.anthropic.com/v1/messages \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "anthropic-beta: mcp-client-2025-04-04" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 2000,
    "messages": [
      {
        "role": "user",
        "content": "Search for the latest news about Rust programming language"
      }
    ],
    "mcp_servers": [
      {
        "type": "url",
        "url": "https://your-server.com/sse",
        "name": "brightdata-mcp",
        "authorization_token": "your_token"
      }
    ]
  }'
```

## 🏗️ API Reference

### HTTP Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/tools` | GET | List available tools |
| `/invoke` | POST | Direct tool invocation |
| `/sse` | POST | Server-Sent Events streaming |
| `/mcp` | POST | MCP JSON-RPC protocol |

### Response Format

All tools return MCP-compatible responses:

```json
{
  "content": [
    {
      "type": "text",
      "text": "Response content here"
    }
  ],
  "is_error": false,
  "raw_value": {
    // Original response data
  }
}
```

## ⚙️ Configuration

### BrightData Setup

1. **Sign up** for BrightData account
2. **Create zones** for Web Unlocker and Browser
3. **Get API credentials** from your dashboard
4. **Set environment variables** as shown above

### Zone Configuration

- **Web Unlocker Zone**: For web scraping and search
- **Browser Zone**: For screenshots and JavaScript rendering

## 🔧 Development

### Building

```bash
# Build library
cargo build

# Build with all features
cargo build --all-features

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run --bin snm_server
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## 📊 Performance

- **Concurrent Requests**: Supports high-concurrency workloads
- **Rate Limiting**: Built-in 10 requests/minute per tool (configurable)
- **Timeout Handling**: Configurable timeouts for different operations
- **Error Recovery**: Automatic retry mechanisms with backoff

## 🛡️ Security

- **Token Authentication**: Secure API access
- **Rate Limiting**: Prevents abuse
- **Input Validation**: Comprehensive parameter validation
- **CORS Support**: Configurable cross-origin requests

## 📝 Examples

Check out the `examples/` directory for:

- Basic usage examples
- Integration patterns
- Advanced configurations
- Error handling strategies

## 🤝 Integration Examples

### With Anthropic Claude

Use as an MCP server to enhance Claude with web scraping capabilities.

### With Custom Applications

Integrate into your Rust applications for:
- E-commerce price monitoring
- Content aggregation
- Market research
- Competitive analysis

## 📋 Requirements

- **Rust**: 1.70 or later
- **BrightData Account**: With API access
- **Network Access**: HTTPS outbound connections

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **BrightData** for providing robust web scraping infrastructure
- **Actix Web** for high-performance HTTP server framework
- **Anthropic** for MCP protocol specification

## 📞 Support

- 📧 **Email**: inxmaurya@gmail.com
- 🐛 **Issues**: [GitHub Issues](https://github.com/snmmaurya/snm-brightdata-client/issues)
- 📖 **Documentation**: [API Docs](https://docs.rs/snm-brightdata-client)

## 🚀 Roadmap

- [ ] Additional search engines
- [ ] Enhanced data extraction templates
- [ ] WebSocket support for real-time scraping
- [ ] GraphQL API interface
- [ ] Kubernetes deployment examples
- [ ] Advanced proxy rotation
- [ ] Machine learning integration for content classification

---

**Made with ❤️ by [SNM Maurya](https://snmmaurya.com.com/solutions/snm-brightdata-client)**