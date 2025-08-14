//! Example: Streamable HTTP Client
//!
//! This example demonstrates:
//! - Connecting to MCP servers over HTTP
//! - Working with both stateful and stateless servers
//! - Session management when connecting to stateful servers
//! - Tool discovery and invocation over HTTP
//!
//! Usage:
//! ```bash
//! # Connect to stateful server (default - port 8080)
//! cargo run --example 24_streamable_http_client
//!
//! # Connect to stateless server (port 8081)
//! cargo run --example 24_streamable_http_client -- stateless
//! ```
//!
//! Make sure to start the corresponding server example first!

use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::{Client, ClientCapabilities, Result};
use serde_json::json;
use tracing::info;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let is_stateless = args.len() > 1 && args[1] == "stateless";

    let (server_url, server_mode) = if is_stateless {
        ("http://localhost:8081", "stateless")
    } else {
        ("http://localhost:8080", "stateful")
    };

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         STREAMABLE HTTP CLIENT EXAMPLE                    ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Connecting to: {:44} ║", server_url);
    println!("║ Server mode:   {:44} ║", server_mode);
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Configure the HTTP transport
    let config = StreamableHttpTransportConfig {
        url: Url::parse(server_url).map_err(|e| pmcp::Error::Internal(e.to_string()))?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,           // Will be set by stateful server if applicable
        enable_json_response: true, // Use simple JSON responses
        on_resumption_token: None,
    };

    // Create the transport - it's already Clone so we can share it
    let transport = StreamableHttpTransport::new(config);

    // Create the client with a clone of the transport
    let mut client = Client::new(transport.clone());

    // Define client capabilities
    let capabilities = ClientCapabilities {
        tools: Some(Default::default()),
        ..Default::default()
    };

    // === Initialize Connection ===
    println!("📡 Initializing connection...");
    let _protocol_version = match client.initialize(capabilities).await {
        Ok(result) => {
            println!("✅ Successfully connected!");
            println!(
                "   Server: {} v{}",
                result.server_info.name, result.server_info.version
            );
            println!("   Protocol: {}", result.protocol_version.0);

            // Set the protocol version on the transport for subsequent requests
            transport.set_protocol_version(Some(result.protocol_version.0.clone()));

            // Get the session ID from the transport (if any)
            let session_id = transport.session_id();

            if let Some(ref sid) = session_id {
                println!("   Session ID: {}", sid);
                println!("   Mode: Stateful (session tracked)");
            } else {
                println!("   Mode: Stateless (no session)");
            }

            // Print the protocol version that will be used for subsequent requests
            if let Some(version) = transport.protocol_version() {
                println!("   Protocol version for subsequent requests: {}", version);
            }

            result.protocol_version.0
        },
        Err(e) => {
            println!("❌ Failed to initialize: {}", e);
            return Err(e.into());
        },
    };
    println!();

    // === List Available Tools ===
    println!("🔧 Discovering available tools...");
    let tools = client.list_tools(None).await?;
    println!("Found {} tools:", tools.tools.len());
    for tool in &tools.tools {
        println!(
            "   • {} - {}",
            tool.name,
            tool.description.as_deref().unwrap_or("(no description)")
        );
    }
    println!();

    // === Demo Tool Calls ===
    println!("📝 Demonstrating tool calls:");
    println!();

    // 1. Echo tool
    println!("1️⃣  Calling 'echo' tool...");
    let echo_result = client
        .call_tool(
            "echo".to_string(),
            json!({
                "message": format!("Hello from {} client!", server_mode)
            }),
        )
        .await?;
    println!(
        "   Response: {}",
        serde_json::to_string_pretty(&echo_result)?
    );
    println!();

    // 2. Calculator tool
    println!("2️⃣  Calling 'calculate' tool...");
    let calc_result = client
        .call_tool(
            "calculate".to_string(),
            json!({
                "operation": "multiply",
                "a": 7,
                "b": 6
            }),
        )
        .await?;
    println!(
        "   Response: {}",
        serde_json::to_string_pretty(&calc_result)?
    );
    println!();

    // 3. Server-specific tool
    if is_stateless {
        // Call random tool (stateless server)
        println!("3️⃣  Calling 'random' tool (stateless server)...");
        let random_result = client
            .call_tool(
                "random".to_string(),
                json!({
                    "min": 1,
                    "max": 100
                }),
            )
            .await?;
        println!(
            "   Response: {}",
            serde_json::to_string_pretty(&random_result)?
        );
    } else {
        // Call session_info tool (stateful server)
        println!("3️⃣  Calling 'session_info' tool (stateful server)...");
        let session_result = client
            .call_tool("session_info".to_string(), json!({}))
            .await?;
        println!(
            "   Response: {}",
            serde_json::to_string_pretty(&session_result)?
        );
    }
    println!();

    // 4. Server info tool (available in both)
    println!("4️⃣  Calling 'server_info' tool...");
    match client.call_tool("server_info".to_string(), json!({})).await {
        Ok(info_result) => {
            println!(
                "   Response: {}",
                serde_json::to_string_pretty(&info_result)?
            );
        },
        Err(e) => {
            println!("   Note: server_info tool not available ({})", e);
        },
    }
    println!();

    // === Demonstrate Session Behavior ===
    if !is_stateless {
        println!("🔐 Session Management Test (stateful server only):");
        // Get current session ID from transport
        let current_session = transport.session_id();
        println!(
            "   Current session ID: {}",
            current_session.as_ref().unwrap_or(&"none".to_string())
        );

        // Try to re-initialize (should fail for stateful server)
        println!("   Attempting re-initialization...");
        match client.initialize(ClientCapabilities::default()).await {
            Ok(_) => println!("   ✅ Re-initialization succeeded (unexpected for stateful)"),
            Err(e) => println!("   ❌ Re-initialization failed as expected: {}", e),
        }
    } else {
        println!("🔄 Stateless Behavior Test:");
        println!("   No session management - each request is independent");

        // Re-initialization should work for stateless
        println!("   Attempting re-initialization...");
        match client.initialize(ClientCapabilities::default()).await {
            Ok(_) => println!("   ✅ Re-initialization succeeded (expected for stateless)"),
            Err(e) => println!("   ❌ Re-initialization failed (unexpected): {}", e),
        }
    }
    println!();

    // === Error Handling Demo ===
    println!("⚠️  Error Handling Demo:");
    println!("   Calling non-existent tool...");
    match client.call_tool("nonexistent".to_string(), json!({})).await {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   ❌ Expected error: {}", e),
    }

    println!("   Division by zero test...");
    match client
        .call_tool(
            "calculate".to_string(),
            json!({
                "operation": "divide",
                "a": 10,
                "b": 0
            }),
        )
        .await
    {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   ❌ Expected error: {}", e),
    }
    println!();

    // === Summary ===
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    SESSION COMPLETE                       ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    if !is_stateless {
        println!("║ Connected to:  Stateful server                            ║");
        // Get final session ID from transport
        let final_session = transport.session_id();
        println!(
            "║ Session ID:    {:43} ║",
            final_session
                .unwrap_or_else(|| "none".to_string())
                .chars()
                .take(43)
                .collect::<String>()
        );
        println!("║ Session tracked and validated by server                   ║");
    } else {
        println!("║ Connected to:  Stateless server                           ║");
        println!("║ No session management - simple and efficient              ║");
        println!("║ Perfect for serverless deployments                        ║");
    }
    println!("╚════════════════════════════════════════════════════════════╝");

    info!("Client example completed successfully");

    Ok(())
}
