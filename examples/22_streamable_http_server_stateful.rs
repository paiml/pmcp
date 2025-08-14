//! Example: Stateful Streamable HTTP Server
//!
//! This example demonstrates:
//! - Running an MCP server over HTTP with session management
//! - Session creation and tracking
//! - Multiple clients with separate sessions
//! - Tool implementation over HTTP transport
//!
//! Protocol Version Header:
//! - The server automatically validates the `mcp-protocol-version` header
//! - Initialize requests negotiate the protocol version
//! - All subsequent (non-initialize) requests MUST include the `mcp-protocol-version` header
//! - The server will reject requests with missing or mismatched protocol versions
//!
//! Run this server with:
//! ```bash
//! cargo run --example 22_streamable_http_server_stateful
//! ```
//!
//! Then connect with the HTTP client example or any MCP-compatible HTTP client.

use async_trait::async_trait;
use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::{Server, ToolHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// === Tool Implementations ===

/// Echo tool - returns the input message
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("(no message provided)");

        Ok(json!({
            "echo": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Calculator tool - performs basic arithmetic
#[derive(Debug, Deserialize)]
struct CalculatorArgs {
    operation: String,
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize)]
struct CalculatorResult {
    result: f64,
    expression: String,
}

struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: CalculatorArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        let result = match params.operation.as_str() {
            "add" => params.a + params.b,
            "subtract" => params.a - params.b,
            "multiply" => params.a * params.b,
            "divide" => {
                if params.b == 0.0 {
                    return Err(pmcp::Error::validation("Division by zero"));
                }
                params.a / params.b
            },
            op => {
                return Err(pmcp::Error::validation(format!(
                    "Unknown operation: {}",
                    op
                )));
            },
        };

        let expression = format!(
            "{} {} {} = {}",
            params.a, params.operation, params.b, result
        );

        Ok(serde_json::to_value(CalculatorResult {
            result,
            expression,
        })?)
    }
}

/// Session info tool - returns information about the current session
struct SessionInfoTool;

#[async_trait]
impl ToolHandler for SessionInfoTool {
    async fn handle(&self, _args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        // In a real implementation, we could access session information
        // For now, return basic info
        Ok(json!({
            "message": "This is a stateful server with session management",
            "session_active": true,
            "server_mode": "stateful",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    info!("Starting Stateful Streamable HTTP Server Example");

    // Build the MCP server with tools
    let server = Server::builder()
        .name("stateful-http-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("echo", EchoTool)
        .tool("calculate", CalculatorTool)
        .tool("session_info", SessionInfoTool)
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Configure the HTTP server address
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080);

    info!("Creating stateful HTTP server on {}", addr);

    // Create the streamable HTTP server with default config (stateful mode)
    // The default configuration includes:
    // - Session ID generation enabled
    // - Event store for resumability
    // - Session lifecycle callbacks
    let http_server = StreamableHttpServer::new(addr, server);

    // Start the server
    let (bound_addr, server_handle) = http_server
        .start()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║        STATEFUL STREAMABLE HTTP SERVER RUNNING            ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Address: http://{:43} ║", bound_addr);
    println!("║ Mode:    Stateful (with session management)               ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Features:                                                  ║");
    println!("║ • Session IDs are generated on initialization             ║");
    println!("║ • Each client gets a unique session                       ║");
    println!("║ • Sessions are tracked and validated                      ║");
    println!("║ • Re-initialization is prevented                          ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Available Tools:                                           ║");
    println!("║ • echo         - Echo back messages                       ║");
    println!("║ • calculate    - Perform arithmetic                       ║");
    println!("║ • session_info - Get session information                  ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Connect with:                                              ║");
    println!("║ cargo run --example 24_streamable_http_client             ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Press Ctrl+C to stop the server");

    // Keep the server running
    server_handle
        .await
        .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

    Ok(())
}
