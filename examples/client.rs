//! Example MCP client implementation

use mcp_sdk::{Client, ClientCapabilities, StdioTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create client with stdio transport
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Initialize connection
    match client.initialize(ClientCapabilities::default()).await {
        Ok(server_info) => {
            println!("Connected to: {}", server_info.server_info.name);
            println!("Server version: {}", server_info.server_info.version);
        },
        Err(e) => {
            eprintln!("Failed to initialize: {}", e);
            return Err(e.into());
        },
    }

    Ok(())
}
