# PMCP - Pragmatic Model Context Protocol

[![CI](https://github.com/paiml/pmcp/actions/workflows/ci.yml/badge.svg)](https://github.com/paiml/pmcp/actions/workflows/ci.yml)
[![Quality Gate](https://img.shields.io/badge/quality%20gate-passing-brightgreen)](https://github.com/paiml/pmcp/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-84.83%25-brightgreen.svg)](https://github.com/paiml/pmcp)
[![Crates.io](https://img.shields.io/crates/v/pmcp.svg)](https://crates.io/crates/pmcp)
[![Documentation](https://docs.rs/pmcp/badge.svg)](https://docs.rs/pmcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80+-orange.svg)](https://www.rust-lang.org)

A high-quality Rust implementation of the [Model Context Protocol](https://modelcontextprotocol.io) (MCP) SDK, maintaining full compatibility with the TypeScript SDK while leveraging Rust's performance and safety guarantees.

Code Name: *Angel Rust*

## Features

- 🚀 **Full Protocol Support**: Complete implementation of MCP specification
- 🔄 **Multiple Transports**: stdio, HTTP/SSE, and WebSocket with auto-reconnection
- 🛡️ **Type Safety**: Compile-time protocol validation
- ⚡ **Zero-Copy Parsing**: Efficient message handling
- 🔐 **Built-in Auth**: OAuth 2.0 and bearer token support
- 🤖 **LLM Sampling**: Native support for model sampling operations
- 🔌 **Middleware System**: Request/response interceptors for custom logic
- 🔁 **Retry Logic**: Built-in exponential backoff for resilient connections
- 📦 **Message Batching**: Efficient notification grouping and debouncing
- 📬 **Resource Subscriptions**: Real-time resource change notifications
- ❌ **Request Cancellation**: Full async cancellation support with CancellationToken
- 🌐 **WebSocket Server**: Complete server-side WebSocket transport implementation
- 📁 **Roots Management**: Directory/URI registration and management
- 📊 **Comprehensive Testing**: Property tests with 100% invariant coverage
- 🏗️ **Quality First**: Zero technical debt, no unwraps in production code

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pmcp = "0.3.1"
```

## Examples

The SDK includes comprehensive examples for all major features:

```bash
# Client initialization and connection
cargo run --example 01_client_initialize

# Basic server with tools
cargo run --example 02_server_basic

# Client tool usage
cargo run --example 03_client_tools

# Server with resources
cargo run --example 04_server_resources

# Client resource access
cargo run --example 05_client_resources

# Server with prompts
cargo run --example 06_server_prompts

# Client prompts usage
cargo run --example 07_client_prompts

# Logging
cargo run --example 08_logging

# Authentication (OAuth, Bearer tokens)
cargo run --example 09_authentication

# Progress notifications
cargo run --example 10_progress_notifications

# Request cancellation
cargo run --example 11_request_cancellation

# Error handling patterns
cargo run --example 12_error_handling

# WebSocket transport
cargo run --example 13_websocket_transport

# LLM sampling operations
cargo run --example 14_sampling_llm

# Middleware and interceptors
cargo run --example 15_middleware

# Message batching and debouncing
cargo run --example 16_batching
```

See the [examples directory](examples/) for detailed documentation.

## What's New in v0.2.0

### 🆕 WebSocket Transport with Auto-Reconnection
Full WebSocket support with automatic reconnection, exponential backoff, and keepalive ping/pong.

### 🆕 HTTP/SSE Transport
HTTP transport with Server-Sent Events for real-time notifications and long-polling support.

### 🆕 LLM Sampling Support
Native support for model sampling operations with the `createMessage` API:
```rust
let result = client.create_message(CreateMessageRequest {
    messages: vec![SamplingMessage {
        role: Role::User,
        content: Content::Text { text: "Hello!".to_string() },
    }],
    ..Default::default()
}).await?;
```

### 🆕 Middleware System
Powerful middleware chain for request/response processing:
```rust
use pmcp::{MiddlewareChain, LoggingMiddleware, AuthMiddleware};

let mut chain = MiddlewareChain::new();
chain.add(Arc::new(LoggingMiddleware::default()));
chain.add(Arc::new(AuthMiddleware::new("token".to_string())));
```

### 🆕 Message Batching & Debouncing
Optimize notification delivery with batching and debouncing:
```rust
use pmcp::{MessageBatcher, BatchingConfig};

let batcher = MessageBatcher::new(BatchingConfig {
    max_batch_size: 10,
    max_wait_time: Duration::from_millis(100),
    ..Default::default()
});
```

## Quick Start

### Client Example

```rust
use pmcp::{Client, StdioTransport, ClientCapabilities};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with stdio transport
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);
    
    // Initialize connection
    let server_info = client.initialize(ClientCapabilities::default()).await?;
    println!("Connected to: {}", server_info.server_info.name);
    
    // List available tools
    let tools = client.list_tools(None).await?;
    for tool in tools.tools {
        println!("Tool: {} - {:?}", tool.name, tool.description);
    }
    
    // Call a tool
    let result = client.call_tool("get-weather", serde_json::json!({
        "location": "San Francisco"
    })).await?;
    
    Ok(())
}
```

### Server Example

```rust
use pmcp::{Server, ServerCapabilities, ToolHandler};
use async_trait::async_trait;
use serde_json::Value;

struct WeatherTool;

#[async_trait]
impl ToolHandler for WeatherTool {
    async fn handle(&self, args: Value) -> pmcp::Result<Value> {
        let location = args["location"].as_str()
            .ok_or_else(|| pmcp::Error::validation("location required"))?;
        
        // Implement weather fetching logic
        Ok(serde_json::json!({
            "temperature": 72,
            "condition": "sunny",
            "location": location
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::builder()
        .name("weather-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("get-weather", WeatherTool)
        .build()?;
    
    // Run with stdio transport
    server.run_stdio().await?;
    Ok(())
}
```

## Transport Options

### stdio (Default)
```rust
let transport = StdioTransport::new();
```

### HTTP/SSE
```rust
use pmcp::{HttpTransport, HttpConfig};

let config = HttpConfig {
    base_url: "http://localhost:8080".parse()?,
    sse_endpoint: Some("/events".to_string()),
    ..Default::default()
};
let transport = HttpTransport::new(config);
```

### WebSocket  
```rust
use pmcp::{WebSocketTransport, WebSocketConfig};

let config = WebSocketConfig {
    url: "ws://localhost:8080".parse()?,
    auto_reconnect: true,
    ..Default::default()
};
let transport = WebSocketTransport::new(config);
```

## Development

### Prerequisites

- Rust 1.80.0 or later
- Git

### Setup

```bash
# Clone the repository
git clone https://github.com/paiml/rust-pmcp
cd rust-pmcp

# Install development tools
make setup

# Run quality checks
make quality-gate
```

### Quality Standards

This project maintains pmat-level quality standards:

- **Zero Technical Debt**: No TODO/FIXME comments
- **No `unwrap()`**: All errors handled explicitly
- **100% Documentation**: Every public API documented
- **Property Testing**: Comprehensive invariant testing
- **Benchmarks**: Performance regression prevention

### Testing

```bash
# Run all tests
make test-all

# Run property tests (slower, more thorough)
make test-property

# Generate coverage report
make coverage

# Run mutation tests
make mutants
```

### Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Ensure all quality checks pass (`make quality-gate`)
4. Commit your changes (following conventional commits)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## Architecture

```
pmcp/
├── src/
│   ├── client/          # Client implementation
│   ├── server/          # Server implementation
│   ├── shared/          # Shared transport/protocol code
│   ├── types/           # Protocol type definitions
│   └── utils/           # Utility functions
├── tests/
│   ├── integration/     # Integration tests
│   └── property/        # Property-based tests
├── benches/             # Performance benchmarks
└── examples/            # Example implementations
```

## Compatibility

| Feature | TypeScript SDK | Rust SDK |
|---------|---------------|----------|
| Protocol Versions | 2024-10-07+ | 2024-10-07+ |
| Transports | stdio, SSE, WebSocket | stdio, SSE, WebSocket |
| Authentication | OAuth 2.0, Bearer | OAuth 2.0, Bearer |
| Tools | ✓ | ✓ |
| Prompts | ✓ | ✓ |
| Resources | ✓ | ✓ |
| Sampling | ✓ | ✓ |

## Performance

Benchmarks show 10x improvement over TypeScript SDK:

- Message parsing: < 1μs
- Round-trip latency: < 100μs (stdio)
- Memory usage: < 10MB baseline

Run benchmarks:
```bash
make bench
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Model Context Protocol](https://modelcontextprotocol.io) specification
- [TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk) for reference implementation
- [PAIML MCP Agent Toolkit](https://github.com/paiml/paiml-mcp-agent-toolkit) for quality standards 
