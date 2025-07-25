//! Example: Client initialization and capability negotiation
//!
//! This example demonstrates:
//! - Creating a client with stdio transport
//! - Initializing connection with server
//! - Specifying client capabilities
//! - Handling server capability response

use pmcp::{Client, ClientCapabilities, StdioTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=debug")
        .init();

    println!("=== MCP Client Initialization Example ===\n");

    // Create client with stdio transport
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Define client capabilities
    let capabilities = ClientCapabilities {
        // Client supports tools
        tools: Some(pmcp::types::capabilities::ToolCapabilities::default()),
        // Client supports prompts
        prompts: Some(pmcp::types::capabilities::PromptCapabilities::default()),
        // Client supports resources
        resources: Some(pmcp::types::capabilities::ResourceCapabilities::default()),
        // Client supports logging
        logging: Some(pmcp::types::capabilities::LoggingCapabilities::default()),
        // Client supports sampling
        sampling: Some(pmcp::types::capabilities::SamplingCapabilities::default()),
        // Client supports roots
        roots: Some(pmcp::types::capabilities::RootsCapabilities::default()),
        // No experimental features
        experimental: None,
    };

    println!("Initializing connection with capabilities:");
    println!("{:#?}\n", capabilities);

    // Initialize connection
    match client.initialize(capabilities).await {
        Ok(result) => {
            println!("✅ Successfully connected to server!");
            println!(
                "Server: {} v{}",
                result.server_info.name, result.server_info.version
            );
            println!("\nServer capabilities:");
            println!("{:#?}", result.capabilities);

            // Check what the server supports
            if result.capabilities.provides_tools() {
                println!("\n✓ Server supports tools");
            }
            if result.capabilities.provides_prompts() {
                println!("✓ Server supports prompts");
            }
            if result.capabilities.provides_resources() {
                println!("✓ Server supports resources");
            }
        },
        Err(e) => {
            eprintln!("❌ Failed to initialize: {}", e);
            return Err(e.into());
        },
    }

    Ok(())
}
