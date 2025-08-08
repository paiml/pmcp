//! Procedural macros for PMCP SDK
//!
//! This crate provides attribute macros to reduce boilerplate when implementing
//! MCP servers with tools, prompts, and resources.
//!
//! # Features
//!
//! - `#[tool]` - Define a tool with automatic schema generation
//! - `#[tool_router]` - Collect tools from an impl block
//! - `#[prompt]` - Define a prompt template
//! - `#[resource]` - Define a resource handler
//!
//! # Examples
//!
//! ## Tool Definition
//!
//! ```rust,ignore
//! use pmcp_macros::{tool, tool_router};
//! use serde::{Deserialize, Serialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Debug, Deserialize, JsonSchema)]
//! struct CalculateParams {
//!     a: i32,
//!     b: i32,
//!     operation: String,
//! }
//!
//! #[derive(Debug, Serialize, JsonSchema)]
//! struct CalculateResult {
//!     result: i32,
//! }
//!
//! #[tool_router]
//! impl Calculator {
//!     #[tool(description = "Perform arithmetic operations")]
//!     async fn calculate(&self, params: CalculateParams) -> Result<CalculateResult, String> {
//!         let result = match params.operation.as_str() {
//!             "add" => params.a + params.b,
//!             "subtract" => params.a - params.b,
//!             "multiply" => params.a * params.b,
//!             "divide" => {
//!                 if params.b == 0 {
//!                     return Err("Division by zero".to_string());
//!                 }
//!                 params.a / params.b
//!             }
//!             _ => return Err("Unknown operation".to_string()),
//!         };
//!         Ok(CalculateResult { result })
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn, ItemImpl};

mod tool;
mod tool_router;
mod utils;

/// Defines a tool handler with automatic schema generation.
///
/// # Attributes
///
/// - `name` - Optional tool name (defaults to function name)
/// - `description` - Tool description (required)
/// - `annotations` - Additional metadata for the tool
///
/// # Examples
///
/// ```rust,ignore
/// #[tool(description = "Add two numbers")]
/// async fn add(a: i32, b: i32) -> Result<i32, String> {
///     Ok(a + b)
/// }
/// ```
///
/// With custom name and annotations:
///
/// ```rust,ignore
/// #[tool(
///     name = "math_add",
///     description = "Add two numbers",
///     annotations(category = "math", complexity = "simple")
/// )]
/// async fn add(a: i32, b: i32) -> Result<i32, String> {
///     Ok(a + b)
/// }
/// ```
#[proc_macro_attribute]
pub fn tool(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);
    
    tool::expand_tool(args.into(), input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Collects all tool methods from an impl block and generates a router.
///
/// This macro scans an impl block for methods marked with `#[tool]` and
/// automatically generates registration code for them.
///
/// # Examples
///
/// ```rust,ignore
/// #[tool_router]
/// impl MyServer {
///     #[tool(description = "Get current time")]
///     async fn get_time(&self) -> Result<String, Error> {
///         Ok(chrono::Utc::now().to_string())
///     }
///     
///     #[tool(description = "Echo message")]
///     async fn echo(&self, message: String) -> Result<String, Error> {
///         Ok(message)
///     }
/// }
/// ```
///
/// The macro generates:
/// - A `tools()` method returning all tool definitions
/// - A `handle_tool()` method for routing tool calls
/// - Automatic schema generation for parameters
#[proc_macro_attribute]
pub fn tool_router(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemImpl);
    
    tool_router::expand_tool_router(args.into(), input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Defines a prompt template with typed arguments.
///
/// # Examples
///
/// ```rust,ignore
/// #[prompt(
///     name = "code_review",
///     description = "Review code for quality issues"
/// )]
/// async fn review_code(&self, language: String, code: String) -> Result<String, Error> {
///     Ok(format!("Review this {} code:\n{}", language, code))
/// }
/// ```
#[proc_macro_attribute]
pub fn prompt(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Prompt macro implementation deferred to future release
    input
}

/// Defines a resource handler with URI pattern matching.
///
/// # Examples
///
/// ```rust,ignore
/// #[resource(
///     uri_template = "file:///{path}",
///     mime_type = "text/plain"
/// )]
/// async fn read_file(&self, path: String) -> Result<String, Error> {
///     std::fs::read_to_string(path).map_err(|e| e.into())
/// }
/// ```
#[proc_macro_attribute]
pub fn resource(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Resource macro implementation deferred to future release
    input
}
