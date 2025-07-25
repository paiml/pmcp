//! Example: Client tool discovery and invocation
//!
//! This example demonstrates:
//! - Listing available tools from a server
//! - Calling tools with arguments
//! - Handling tool responses
//! - Error handling for tool calls

use pmcp::{Client, ClientCapabilities, StdioTransport};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Client Tools Example ===\n");

    // Create and initialize client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Initialize with tool support
    let capabilities = ClientCapabilities {
        tools: Some(Default::default()),
        ..Default::default()
    };

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // List available tools
    println!("📋 Listing available tools:");
    let tools_result = client.list_tools(None).await?;

    for tool in &tools_result.tools {
        println!("\n🔧 Tool: {}", tool.name);
        if let Some(desc) = &tool.description {
            println!("   Description: {}", desc);
        }

        // Print input schema if available
        if !tool.input_schema.is_null() {
            println!(
                "   Input schema: {}",
                serde_json::to_string_pretty(&tool.input_schema)?
            );
        }
    }

    // Example: Call a calculator tool
    println!("\n\n📐 Calling calculator tool:");
    let calc_args = json!({
        "operation": "multiply",
        "a": 42,
        "b": 3.14159
    });

    println!(
        "   Arguments: {}",
        serde_json::to_string_pretty(&calc_args)?
    );

    match client.call_tool("calculator".to_string(), calc_args).await {
        Ok(result) => {
            println!(
                "   ✅ Result: {}",
                serde_json::to_string_pretty(&result.content)?
            );
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example: Call a string manipulation tool
    println!("\n\n📝 Calling string manipulation tool:");
    let string_args = json!({
        "text": "Hello, MCP!",
        "operation": "reverse"
    });

    println!(
        "   Arguments: {}",
        serde_json::to_string_pretty(&string_args)?
    );

    match client
        .call_tool("string_manipulator".to_string(), string_args)
        .await
    {
        Ok(result) => {
            println!(
                "   ✅ Result: {}",
                serde_json::to_string_pretty(&result.content)?
            );
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example: Handle tool errors
    println!("\n\n⚠️  Testing error handling:");
    let bad_args = json!({
        "operation": "divide",
        "a": 10,
        "b": 0  // Division by zero
    });

    println!("   Arguments: {}", serde_json::to_string_pretty(&bad_args)?);

    match client.call_tool("calculator".to_string(), bad_args).await {
        Ok(result) => {
            println!(
                "   Result: {}",
                serde_json::to_string_pretty(&result.content)?
            );
        },
        Err(e) => {
            println!("   ✅ Error caught: {}", e);
            // Check error type
            if let Some(code) = e.error_code() {
                println!("   Error code: {:?}", code);
            }
        },
    }

    Ok(())
}
