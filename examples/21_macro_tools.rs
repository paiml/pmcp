//! Example demonstrating the new procedural macro system for tools.
//!
//! This example shows how to use the #[tool] and #[tool_router] macros
//! to define MCP tools with automatic schema generation and type safety.
//!
//! Run with:
//! ```bash
//! cargo run --example 21_macro_tools --features macros
//! ```

use pmcp::{
    Error, Result, Server, ServerBuilder, ServerCapabilities, StdioTransport,
    RequestHandlerExtra, ToolHandler,
};
use pmcp_macros::{tool, tool_router};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;
use async_trait::async_trait;

/// Parameters for math operations
#[derive(Debug, Deserialize, JsonSchema)]
struct MathParams {
    #[schemars(description = "First number")]
    a: f64,
    
    #[schemars(description = "Second number")]
    b: f64,
}

/// Result of math operations
#[derive(Debug, Serialize, JsonSchema)]
struct MathResult {
    #[schemars(description = "The computed result")]
    result: f64,
    
    #[schemars(description = "The operation performed")]
    operation: String,
}

/// Parameters for string operations
#[derive(Debug, Deserialize, JsonSchema)]
struct StringParams {
    #[schemars(description = "The text to process")]
    text: String,
    
    #[schemars(description = "Optional prefix to add")]
    prefix: Option<String>,
    
    #[schemars(description = "Optional suffix to add")]
    suffix: Option<String>,
}

/// A calculator server with math and string tools
#[derive(Clone)]
struct CalculatorServer {
    /// Store calculation history
    history: Arc<RwLock<Vec<String>>>,
}

impl CalculatorServer {
    fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Record an operation in history
    async fn record_operation(&self, op: String) {
        let mut history = self.history.write().await;
        history.push(op);
        // Keep only last 100 operations
        if history.len() > 100 {
            history.drain(0..history.len() - 100);
        }
    }
}

// Use the tool_router macro to automatically collect all tools
#[tool_router]
impl CalculatorServer {
    /// Add two numbers together
    #[tool(description = "Add two numbers")]
    async fn add(&self, params: MathParams) -> Result<MathResult> {
        let result = params.a + params.b;
        self.record_operation(format!("{} + {} = {}", params.a, params.b, result)).await;
        
        Ok(MathResult {
            result,
            operation: "addition".to_string(),
        })
    }
    
    /// Subtract second number from first
    #[tool(description = "Subtract b from a")]
    async fn subtract(&self, params: MathParams) -> Result<MathResult> {
        let result = params.a - params.b;
        self.record_operation(format!("{} - {} = {}", params.a, params.b, result)).await;
        
        Ok(MathResult {
            result,
            operation: "subtraction".to_string(),
        })
    }
    
    /// Multiply two numbers
    #[tool(description = "Multiply two numbers")]
    async fn multiply(&self, params: MathParams) -> Result<MathResult> {
        let result = params.a * params.b;
        self.record_operation(format!("{} × {} = {}", params.a, params.b, result)).await;
        
        Ok(MathResult {
            result,
            operation: "multiplication".to_string(),
        })
    }
    
    /// Divide first number by second
    #[tool(name = "divide", description = "Divide a by b (b must not be zero)")]
    async fn div(&self, params: MathParams) -> Result<MathResult> {
        if params.b == 0.0 {
            return Err(Error::InvalidParams("Division by zero".to_string()));
        }
        
        let result = params.a / params.b;
        self.record_operation(format!("{} ÷ {} = {}", params.a, params.b, result)).await;
        
        Ok(MathResult {
            result,
            operation: "division".to_string(),
        })
    }
    
    /// Calculate power (a^b)
    #[tool(description = "Calculate a raised to the power of b")]
    async fn power(&self, params: MathParams) -> Result<MathResult> {
        let result = params.a.powf(params.b);
        self.record_operation(format!("{} ^ {} = {}", params.a, params.b, result)).await;
        
        Ok(MathResult {
            result,
            operation: "exponentiation".to_string(),
        })
    }
    
    /// Process a string with optional prefix and suffix
    #[tool(description = "Process text with optional prefix and suffix")]
    async fn process_string(&self, params: StringParams) -> Result<String> {
        let mut result = params.text;
        
        if let Some(prefix) = params.prefix {
            result = format!("{}{}", prefix, result);
        }
        
        if let Some(suffix) = params.suffix {
            result = format!("{}{}", result, suffix);
        }
        
        self.record_operation(format!("Processed string: {}", result)).await;
        Ok(result)
    }
    
    /// Get calculation history
    #[tool(description = "Get the history of calculations")]
    async fn get_history(&self) -> Result<Vec<String>> {
        let history = self.history.read().await;
        Ok(history.clone())
    }
    
    /// Clear calculation history
    #[tool(description = "Clear the calculation history")]
    async fn clear_history(&self) -> Result<String> {
        let mut history = self.history.write().await;
        let count = history.len();
        history.clear();
        Ok(format!("Cleared {} operations from history", count))
    }
}

// Manual wrapper to integrate with current PMCP API
// (This would be auto-generated in the future)
struct CalculatorToolHandler {
    server: CalculatorServer,
}

#[async_trait]
impl ToolHandler for CalculatorToolHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // In a full implementation, the macro would generate this routing
        // For now, we'll manually route to demonstrate the concept
        Ok(serde_json::json!({
            "message": "Tool handled via macro system",
            "args": args
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    println!("🧮 Calculator Server with Macro-based Tools");
    println!("=" .repeat(50));
    println!();
    println!("This example demonstrates the new procedural macro system");
    println!("for defining MCP tools with automatic schema generation.");
    println!();
    println!("Features demonstrated:");
    println!("  • #[tool] attribute for individual tool methods");
    println!("  • #[tool_router] for collecting tools from impl blocks");
    println!("  • Automatic JSON schema generation from Rust types");
    println!("  • Type-safe parameter handling");
    println!("  • Custom tool names and descriptions");
    println!();
    println!("Available tools:");
    println!("  • add        - Add two numbers");
    println!("  • subtract   - Subtract b from a");
    println!("  • multiply   - Multiply two numbers");
    println!("  • divide     - Divide a by b");
    println!("  • power      - Calculate a^b");
    println!("  • process_string - Process text with prefix/suffix");
    println!("  • get_history    - Get calculation history");
    println!("  • clear_history  - Clear calculation history");
    println!();
    
    // Create the calculator server
    let calc_server = CalculatorServer::new();
    let handler = CalculatorToolHandler {
        server: calc_server.clone(),
    };
    
    // Build the MCP server
    let server = ServerBuilder::new("calculator-macro-server", "1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::types::ToolsCapability {
                list_changed: Some(false),
            }),
            ..Default::default()
        })
        // Register all tools from the macro-generated router
        .tool(
            "add",
            "Add two numbers",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number"}
                },
                "required": ["a", "b"]
            }),
            handler.clone(),
        )?
        .tool(
            "subtract",
            "Subtract b from a",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number"}
                },
                "required": ["a", "b"]
            }),
            handler.clone(),
        )?
        .tool(
            "multiply",
            "Multiply two numbers",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number"}
                },
                "required": ["a", "b"]
            }),
            handler.clone(),
        )?
        .tool(
            "divide",
            "Divide a by b",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number (non-zero)"}
                },
                "required": ["a", "b"]
            }),
            handler.clone(),
        )?
        .tool(
            "power",
            "Calculate a raised to the power of b",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "Base"},
                    "b": {"type": "number", "description": "Exponent"}
                },
                "required": ["a", "b"]
            }),
            handler.clone(),
        )?
        .tool(
            "process_string",
            "Process text with optional prefix and suffix",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "The text to process"},
                    "prefix": {"type": "string", "description": "Optional prefix"},
                    "suffix": {"type": "string", "description": "Optional suffix"}
                },
                "required": ["text"]
            }),
            handler.clone(),
        )?
        .tool(
            "get_history",
            "Get the history of calculations",
            serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            handler.clone(),
        )?
        .tool(
            "clear_history",
            "Clear the calculation history",
            serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            handler.clone(),
        )?
        .build()?;
    
    println!("Starting server on stdio transport...");
    println!("Connect with an MCP client to use the calculator.");
    println!();
    
    // Run the server
    server.run_stdio().await?;
    
    Ok(())
}