//! Integration tests for TypeScript SDK interoperability.
//!
//! These tests ensure that PMCP Rust SDK can communicate correctly with
//! the official TypeScript SDK implementation.

use pmcp::{
    Client, ClientCapabilities, Error, Result, Server, ServerBuilder, ServerCapabilities,
    StdioTransport, ToolHandler, PromptHandler, ResourceHandler,
};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::sync::RwLock;
use async_trait::async_trait;

/// Test tool handler for integration tests
#[derive(Clone)]
struct TestToolHandler;

#[async_trait]
impl ToolHandler for TestToolHandler {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value> {
        match args.get("operation").and_then(|v| v.as_str()) {
            Some("add") => {
                let a = args.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
                let b = args.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
                Ok(json!({ "result": a + b }))
            }
            Some("echo") => {
                let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!({ "message": message }))
            }
            _ => Err(Error::InvalidParams("Unknown operation".to_string())),
        }
    }
}

/// Test resource handler
#[derive(Clone)]
struct TestResourceHandler;

#[async_trait]
impl ResourceHandler for TestResourceHandler {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> Result<pmcp::types::ReadResourceResult> {
        if uri == "test://example.txt" {
            Ok(pmcp::types::ReadResourceResult {
                contents: vec![pmcp::types::ResourceContent {
                    uri: uri.to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: Some("Hello from Rust server!".to_string()),
                    blob: None,
                }],
            })
        } else {
            Err(Error::ResourceNotFound(uri.to_string()))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> Result<pmcp::types::ListResourcesResult> {
        Ok(pmcp::types::ListResourcesResult {
            resources: vec![pmcp::types::ResourceInfo {
                uri: "test://example.txt".to_string(),
                name: Some("Example Text File".to_string()),
                description: Some("A test resource from Rust".to_string()),
                mime_type: Some("text/plain".to_string()),
            }],
            next_cursor: None,
        })
    }
}

/// Test prompt handler
#[derive(Clone)]
struct TestPromptHandler;

#[async_trait]
impl PromptHandler for TestPromptHandler {
    async fn handle(
        &self,
        args: std::collections::HashMap<String, String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> Result<pmcp::types::GetPromptResult> {
        let name = args.get("name").map(|s| s.as_str()).unwrap_or("User");
        
        Ok(pmcp::types::GetPromptResult {
            description: Some(format!("Greeting for {}", name)),
            messages: vec![pmcp::types::PromptMessage {
                role: pmcp::types::Role::User,
                content: pmcp::types::Content::Text {
                    text: format!("Please greet {}", name),
                },
            }],
        })
    }
}

#[tokio::test]
async fn test_rust_client_typescript_server() -> Result<()> {
    // Skip if Node.js is not available
    if !is_node_available() {
        eprintln!("Node.js not found, skipping TypeScript interop tests");
        return Ok(());
    }

    // Install TypeScript SDK if needed
    install_typescript_sdk()?;

    // Start TypeScript server
    let mut ts_server = start_typescript_server()?;

    // Give server time to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create Rust client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Initialize connection
    let init_result = client.initialize(ClientCapabilities::default()).await?;
    assert_eq!(init_result.server_info.name, "typescript-test-server");

    // Test tool listing
    let tools = client.list_tools(None).await?;
    assert!(tools.tools.len() >= 2);
    assert!(tools.tools.iter().any(|t| t.name == "add"));
    assert!(tools.tools.iter().any(|t| t.name == "echo"));

    // Test tool calling
    let add_result = client
        .call_tool("add".to_string(), json!({ "a": 5, "b": 3 }))
        .await?;
    assert_eq!(
        add_result.content[0],
        pmcp::types::ToolContent::Text {
            text: "8".to_string()
        }
    );

    let echo_result = client
        .call_tool("echo".to_string(), json!({ "message": "Hello from Rust!" }))
        .await?;
    assert_eq!(
        echo_result.content[0],
        pmcp::types::ToolContent::Text {
            text: "Hello from Rust!".to_string()
        }
    );

    // Test resource listing
    let resources = client.list_resources(None).await?;
    assert!(resources.resources.len() >= 1);
    assert_eq!(resources.resources[0].uri, "test://example.txt");

    // Test resource reading
    let resource = client.read_resource("test://example.txt".to_string()).await?;
    assert_eq!(resource.contents.len(), 1);
    assert_eq!(
        resource.contents[0].text,
        Some("Hello from TypeScript server!".to_string())
    );

    // Test prompt listing
    let prompts = client.list_prompts(None).await?;
    assert!(prompts.prompts.len() >= 1);
    assert_eq!(prompts.prompts[0].name, "greeting");

    // Test prompt getting
    let prompt = client
        .get_prompt(
            "greeting".to_string(),
            [("name".to_string(), "Alice".to_string())]
                .into_iter()
                .collect(),
        )
        .await?;
    assert_eq!(prompt.messages.len(), 1);

    // Clean up
    ts_server.kill().await?;

    Ok(())
}

#[tokio::test]
async fn test_typescript_client_rust_server() -> Result<()> {
    // Skip if Node.js is not available
    if !is_node_available() {
        eprintln!("Node.js not found, skipping TypeScript interop tests");
        return Ok(());
    }

    // Install TypeScript SDK if needed
    install_typescript_sdk()?;

    // Create and start Rust server
    let server = ServerBuilder::new("rust-test-server", "1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::types::ToolsCapability {
                list_changed: Some(false),
            }),
            resources: Some(pmcp::types::ResourcesCapability {
                subscribe: Some(false),
                list_changed: Some(false),
            }),
            prompts: Some(pmcp::types::PromptsCapability {
                list_changed: Some(false),
            }),
            ..Default::default()
        })
        .tool(
            "echo",
            "Echo the input",
            json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }),
            TestToolHandler,
        )?
        .tool(
            "add",
            "Add two numbers",
            json!({
                "type": "object",
                "properties": {
                    "a": { "type": "number" },
                    "b": { "type": "number" }
                },
                "required": ["a", "b"]
            }),
            TestToolHandler,
        )?
        .resource_handler(TestResourceHandler)?
        .prompt("greeting", "Generate a greeting", TestPromptHandler)?
        .build()?;

    // Run server in background
    let server_handle = tokio::spawn(async move {
        server.run_stdio().await
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Run TypeScript client tests
    let output = Command::new("npm")
        .args(&["test", "--", "test-client.js"])
        .current_dir("tests/integration/typescript-interop")
        .output()?;

    if !output.status.success() {
        eprintln!("TypeScript client test failed:");
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("TypeScript client tests failed");
    }

    // Clean up
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_protocol_compatibility() -> Result<()> {
    // Skip if Node.js is not available
    if !is_node_available() {
        eprintln!("Node.js not found, skipping TypeScript interop tests");
        return Ok(());
    }

    // Test protocol version negotiation
    let versions = vec!["2024-11-05", "2025-03-26", "2025-06-18"];
    
    for version in versions {
        println!("Testing protocol version: {}", version);
        
        // TODO: Test with specific protocol version
        // This would require modifying the client/server initialization
        // to specify protocol version
    }

    Ok(())
}

#[tokio::test]
async fn test_error_handling_interop() -> Result<()> {
    // Skip if Node.js is not available
    if !is_node_available() {
        eprintln!("Node.js not found, skipping TypeScript interop tests");
        return Ok(());
    }

    // Test that errors are properly propagated between implementations
    
    // Test Rust client handling TypeScript server errors
    // TODO: Implement error test scenarios

    // Test TypeScript client handling Rust server errors
    // TODO: Implement error test scenarios

    Ok(())
}

#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    // Skip if Node.js is not available
    if !is_node_available() {
        eprintln!("Node.js not found, skipping TypeScript interop tests");
        return Ok(());
    }

    // Test multiple concurrent operations between Rust and TypeScript
    
    // TODO: Implement concurrent operation tests

    Ok(())
}

// Helper functions

fn is_node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn install_typescript_sdk() -> Result<()> {
    let output = Command::new("npm")
        .arg("install")
        .current_dir("tests/integration/typescript-interop")
        .output()
        .map_err(|e| Error::InternalError(format!("Failed to run npm install: {}", e)))?;

    if !output.status.success() {
        return Err(Error::InternalError(format!(
            "npm install failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(())
}

fn start_typescript_server() -> Result<TokioCommand> {
    let mut cmd = TokioCommand::new("node");
    cmd.arg("test-server.js")
        .current_dir("tests/integration/typescript-interop")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    Ok(cmd)
}

fn start_typescript_client() -> Result<TokioCommand> {
    let mut cmd = TokioCommand::new("node");
    cmd.arg("test-client.js")
        .current_dir("tests/integration/typescript-interop")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    Ok(cmd)
}