//! Example of using WebSocket transport for MCP communication.

use pmcp::{Client, ClientCapabilities, WebSocketConfig, WebSocketTransport};
use tracing::info;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Configure WebSocket transport
    let config = WebSocketConfig {
        url: Url::parse("ws://localhost:3000/mcp")?,
        auto_reconnect: true,
        reconnect_delay: std::time::Duration::from_secs(1),
        max_reconnect_delay: std::time::Duration::from_secs(30),
        max_reconnect_attempts: Some(5),
        ping_interval: Some(std::time::Duration::from_secs(30)),
        request_timeout: std::time::Duration::from_secs(30),
    };

    info!("Creating WebSocket transport");
    let transport = WebSocketTransport::new(config);

    // Connect to the server
    info!("Connecting to WebSocket server");
    transport.connect().await?;

    // Create client
    let mut client = Client::new(transport);

    // Initialize the connection
    info!("Initializing MCP connection");
    let capabilities = ClientCapabilities {
        tools: Some(Default::default()),
        ..Default::default()
    };

    let server_info = client.initialize(capabilities).await?;
    info!(
        "Connected to server: {} v{}",
        server_info.server_info.name, server_info.server_info.version
    );

    // Use the client normally
    let tools = client.list_tools(None).await?;
    info!("Available tools: {:?}", tools.tools.len());

    Ok(())
}
