# snm-brightdata-client

**A fully async Bright Data MCP Client implemented in Rust with Actix Web, Reqwest, and modular architecture.**

This crate is designed to simplify integration with Bright Data's [MCP (Mobile Carrier Proxy)](https://brightdata.com/products/proxy/mobile) network. It provides both a **client SDK**, a **command-line interface**, and an **Actix Web-based API server** out of the box. Use it to build scalable scraping tools, proxy integrations, or backend APIs powered by Bright Data's infrastructure.

---

## 🔥 Features

- ✅ **Fully async** using `tokio` and `reqwest`
- ⚙️ **Flexible Tool interface** using the `Tool` trait (like plugin system)
- 🌐 **Built-in HTTP server** with Actix Web (`snm_server`)
- 💻 **CLI support** via Clap (`snm_cli`)
- 🧪 **Integration tested** (`integration_test.rs`)
- 🪝 **Modular structure** ready for extension and reuse
- 🔑 **Token-based auth** for client config
- 🪄 **Structured types** for request/response via `serde`

---

## 📦 Installation

Include in your `Cargo.toml`:

```toml
snm-brightdata-client = { git = "https://github.com/snmmaurya/snm-brightdata-client" }
```

> Or clone the repo:
> ```bash
> git clone https://github.com/snmmaurya/snm-brightdata-client.git
> cd snm-brightdata-client
> ```

---

## 🛠️ Usage

### 🧱 As a Client

```rust
use snm_brightdata_client::{BrightDataClient, ClientConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig::builder()
        .api_token("your-brightdata-token")
        .timeout(Duration::from_secs(30))
        .build()?;

    let client = BrightDataClient::new(config).await?;

    // Add your custom Tool and run it
    let result = client.run("echo", serde_json::json!({"message": "Hello"})).await?;
    println!("Response: {:?}", result);

    Ok(())
}
```

### 🖥 CLI Mode

```bash
cargo run --bin snm_cli -- --tool echo --payload '{"message":"Hello"}'
```

### 🌐 Server Mode

```bash
cargo run --bin snm_server
```

This starts an Actix Web HTTP API for running tools via REST POST requests.

---

## 📁 Project Structure

| File               | Description                                 |
|--------------------|---------------------------------------------|
| `client.rs`        | BrightDataClient implementation             |
| `rpc_client.rs`    | CLI tool runner using Tool trait            |
| `server.rs`        | Actix Web REST API interface                |
| `tool.rs`          | Trait-based Tool executor (`Tool`)          |
| `types.rs`         | All request/response models                 |
| `config.rs`        | Configuration management (env + CLI)        |
| `error.rs`         | Unified error handling via `anyhow`/`thiserror` |
| `integration_test.rs` | Integration test for actual tool runs    |

---

## 🧪 Testing

```bash
cargo test
```

---

## ✨ Extending Tools

To add your own tool:
1. Implement the `Tool` trait
2. Register it inside the tool resolver

Example:
```rust
pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &'static str { "my_tool" }

    async fn run(&self, input: Value) -> Result<Value, ToolError> {
        // your logic
        Ok(json!({ "status": "ok" }))
    }
}
```

---

## 📜 License

MIT © [Snm Maurya](https://github.com/snmmaurya/snm-brightdata-client)

---

## 📌 Related Projects

- [`brightdata-mcp`](https://github.com/astwrks/brightdata-mcp) – Minimal Bright Data client
- [`actix-web`](https://github.com/actix/actix-web) – Web framework used for server