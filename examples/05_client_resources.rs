//! Example: Client resource access
//!
//! This example demonstrates:
//! - Listing available resources
//! - Reading resource contents
//! - Handling different content types
//! - Resource pagination

use pmcp::{Client, ClientCapabilities, StdioTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Client Resources Example ===\n");

    // Create and initialize client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Initialize with resource support
    let capabilities = ClientCapabilities {
        resources: Some(Default::default()),
        ..Default::default()
    };

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // List all resources with pagination
    println!("📋 Listing all available resources:\n");
    
    let mut cursor: Option<String> = None;
    let mut page = 1;
    
    loop {
        let result = client.list_resources(cursor).await?;
        
        if !result.resources.is_empty() {
            println!("📄 Page {}:", page);
            for resource in &result.resources {
                println!("\n  🔗 URI: {}", resource.uri);
                println!("     Name: {}", resource.name);
                if let Some(desc) = &resource.description {
                    println!("     Description: {}", desc);
                }
                if let Some(mime) = &resource.mime_type {
                    println!("     MIME type: {}", mime);
                }
            }
        }
        
        cursor = result.next_cursor;
        if cursor.is_none() {
            break;
        }
        
        page += 1;
        println!("\n--- More resources available ---");
    }

    // Read specific resources
    println!("\n\n📖 Reading specific resources:");

    // Example 1: Read a JSON configuration file
    println!("\n1️⃣ Reading JSON config:");
    match client.read_resource("file://config/app.json").await {
        Ok(result) => {
            for content in result.contents {
                match content {
                    pmcp::types::ResourceContent::Text { uri, text, mime_type } => {
                        println!("   URI: {}", uri);
                        if let Some(mime) = mime_type {
                            println!("   Type: {}", mime);
                        }
                        println!("   Content:\n{}", text);
                        
                        // Parse JSON if it's JSON
                        if mime_type.as_deref() == Some("application/json") {
                            match serde_json::from_str::<serde_json::Value>(&text) {
                                Ok(json) => {
                                    println!("   Parsed JSON: {:#?}", json);
                                }
                                Err(e) => {
                                    println!("   Failed to parse JSON: {}", e);
                                }
                            }
                        }
                    }
                    pmcp::types::ResourceContent::Blob { uri, mime_type, blob } => {
                        println!("   URI: {} (binary data, {} bytes)", uri, blob.len());
                        if let Some(mime) = mime_type {
                            println!("   Type: {}", mime);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }

    // Example 2: Read a CSV file
    println!("\n2️⃣ Reading CSV data:");
    match client.read_resource("file://data/users.csv").await {
        Ok(result) => {
            for content in result.contents {
                if let pmcp::types::ResourceContent::Text { text, .. } = content {
                    println!("   CSV Content:");
                    for line in text.lines() {
                        println!("   {}", line);
                    }
                }
            }
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }

    // Example 3: Read a template resource with parameters
    println!("\n3️⃣ Reading template resource:");
    match client.read_resource("template://greeting/Alice").await {
        Ok(result) => {
            for content in result.contents {
                if let pmcp::types::ResourceContent::Text { text, .. } = content {
                    println!("   Message: {}", text);
                }
            }
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }

    // Example 4: Handle non-existent resource
    println!("\n4️⃣ Testing error handling:");
    match client.read_resource("file://nonexistent.txt").await {
        Ok(_) => {
            println!("   Unexpected success!");
        }
        Err(e) => {
            println!("   ✅ Error caught: {}", e);
            match e {
                pmcp::Error::ResourceNotFound(uri) => {
                    println!("   Resource not found: {}", uri);
                }
                _ => {
                    println!("   Other error type");
                }
            }
        }
    }

    Ok(())
}