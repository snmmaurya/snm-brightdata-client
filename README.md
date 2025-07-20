# snm-brightdata-client

A modern, async Rust client for interacting with the Bright Data (formerly Luminati) API, including search, scraping, and browser automation capabilities. Built for flexibility and reliability in production environments.

## 🚀 Features

- ✅ Easy-to-use async client
- 🔐 Secure API token-based authentication
- 🔍 Supports Bright Data’s SERP & Web Unlocker APIs
- ⚙️ Configurable request options (timeouts, retries, engines)
- 🛡️ Robust error handling with `anyhow` and typed errors
- 📦 Lightweight dependencies and fully async with `reqwest`

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
snm-brightdata-client = "0.1.0"
```

Or use:

```bash
cargo add snm-brightdata-client
```

## ⚡ Usage Example

```rust
use snm_brightdata_client::{BrightDataClient, ClientConfig, SearchRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig::builder()
        .api_token("your-api-token")
        .build()?;

    let client = BrightDataClient::new(config).await?;

    let search = SearchRequest {
        query: "Rust async client".into(),
        engine: Some("google".into()),
        ..Default::default()
    };

    let result = client.search(search).await?;
    println!("{:#?}", result);

    Ok(())
}
```

## 📚 Documentation

- [Bright Data API Docs](https://brightdata.com/)
- Full crate docs coming soon.

## 🛠 Development

```bash
cargo build
cargo test
```

## 📄 License

MIT © 2025 SNM Maurya
