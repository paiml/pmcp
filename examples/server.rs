//! Example MCP server implementation

use async_trait::async_trait;
use mcp_sdk::{Server, ServerCapabilities, ToolHandler};
use serde_json::Value;

struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value) -> mcp_sdk::Result<Value> {
        Ok(serde_json::json!({
            "echo": args.get("message").unwrap_or(&Value::String(String::new()))
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let server = Server::builder()
        .name("example-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("echo", EchoTool)
        .build()?;

    println!("Starting example MCP server...");
    server.run_stdio().await?;

    Ok(())
}
