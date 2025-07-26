//! Authentication example demonstrating authentication with MCP
//!
//! This example shows how to create and use authentication information
//! with an MCP client.

use pmcp::{
    types::{AuthInfo, AuthScheme},
    Client, ClientCapabilities, StdioTransport,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== MCP Authentication Example ===");

    // Create client
    let transport = StdioTransport::new();
    let _client = Client::new(transport);

    // Initialize connection
    let _capabilities = ClientCapabilities::default();
    println!("Initializing MCP client...");

    // In a real scenario, you would connect to a server first
    // client.initialize(capabilities).await?;

    // Create bearer token authentication
    let auth_info = AuthInfo {
        scheme: AuthScheme::Bearer,
        token: Some("example-bearer-token".to_string()),
        oauth: None,
        params: std::collections::HashMap::new(),
    };

    println!("Created authentication info:");
    println!("  Scheme: {:?}", auth_info.scheme);
    println!("  Token: {:?}", auth_info.token);

    // In a real implementation, you would authenticate with the server
    // client.authenticate(&auth_info)?;

    println!("Authentication configuration completed!");
    println!("Note: This example shows the authentication structure.");
    println!("In real usage, you would connect to a server that requires authentication.");

    Ok(())
}
