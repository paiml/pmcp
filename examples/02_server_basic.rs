//! Example: Basic MCP server with tool support
//!
//! This example demonstrates:
//! - Creating a server with tools
//! - Implementing tool handlers
//! - Running server with stdio transport

use async_trait::async_trait;
use pmcp::{Server, ServerCapabilities, ToolHandler};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Define input/output types for better type safety
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

// Calculator tool implementation
struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    async fn handle(&self, args: Value) -> pmcp::Result<Value> {
        // Parse arguments
        let params: CalculatorArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {}", e)))?;

        // Perform calculation
        let result = match params.operation.as_str() {
            "add" => params.a + params.b,
            "subtract" => params.a - params.b,
            "multiply" => params.a * params.b,
            "divide" => {
                if params.b == 0.0 {
                    return Err(pmcp::Error::validation("Division by zero"));
                }
                params.a / params.b
            }
            op => {
                return Err(pmcp::Error::validation(format!(
                    "Unknown operation: {}",
                    op
                )))
            }
        };

        // Return result
        Ok(serde_json::to_value(CalculatorResult {
            result,
            expression: format!("{} {} {} = {}", params.a, params.operation, params.b, result),
        })?)
    }
}

// String manipulation tool
struct StringTool;

#[async_trait]
impl ToolHandler for StringTool {
    async fn handle(&self, args: Value) -> pmcp::Result<Value> {
        let text = args["text"]
            .as_str()
            .ok_or_else(|| pmcp::Error::validation("text field required"))?;
        let operation = args["operation"]
            .as_str()
            .ok_or_else(|| pmcp::Error::validation("operation field required"))?;

        let result = match operation {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            "reverse" => text.chars().rev().collect(),
            "length" => text.len().to_string(),
            _ => return Err(pmcp::Error::validation("Unknown operation")),
        };

        Ok(serde_json::json!({
            "original": text,
            "result": result,
            "operation": operation
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Server Example ===");
    println!("Starting server with calculator and string tools...\n");

    // Build server with tools
    let server = Server::builder()
        .name("example-tools-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("calculator", CalculatorTool)
        .tool("string_manipulator", StringTool)
        .build()?;

    println!("Server ready! Listening on stdio...");
    println!("Available tools:");
    println!("  - calculator: Basic math operations (add, subtract, multiply, divide)");
    println!("  - string_manipulator: String operations (uppercase, lowercase, reverse, length)");

    // Run server
    server.run_stdio().await?;

    Ok(())
}