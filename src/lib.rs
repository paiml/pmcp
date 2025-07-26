//! # MCP SDK for Rust
//!
//! A high-quality Rust implementation of the Model Context Protocol (MCP) SDK.
//!
//! This crate provides both client and server implementations of MCP with:
//! - Full protocol compatibility with the TypeScript SDK
//! - Zero-copy parsing where possible
//! - Comprehensive type safety
//! - Multiple transport options (stdio, HTTP/SSE, WebSocket)
//! - Built-in authentication support
//!
//! ## Quick Start
//!
//! ### Client Example
//!
//! ```rust
//! use pmcp::{Client, StdioTransport, ClientCapabilities};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client with stdio transport
//! let transport = StdioTransport::new();
//! let mut client = Client::new(transport);
//!
//! // Initialize the connection
//! let server_info = client.initialize(ClientCapabilities::default()).await?;
//!
//! // List available tools
//! let tools = client.list_tools(None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Server Example
//!
//! ```rust
//! use pmcp::{Server, ServerCapabilities, ToolHandler};
//! use async_trait::async_trait;
//! use serde_json::Value;
//!
//! struct MyTool;
//!
//! #[async_trait]
//! impl ToolHandler for MyTool {
//!     async fn handle(&self, args: Value) -> Result<Value, pmcp::Error> {
//!         Ok(serde_json::json!({"result": "success"}))
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Server::builder()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .capabilities(ServerCapabilities::default())
//!     .tool("my-tool", MyTool)
//!     .build()?;
//!
//! // Run with stdio transport
//! server.run_stdio().await?;
//! # Ok(())
//! # }
//! ```

#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Allow certain clippy lints that are too pedantic for this codebase
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::result_large_err)]

pub mod client;
pub mod error;
pub mod server;
pub mod shared;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use client::{Client, ClientBuilder};
pub use error::{Error, ErrorCode, Result};
pub use server::{PromptHandler, ResourceHandler, Server, ServerBuilder, ToolHandler};
pub use shared::{StdioTransport, Transport};
pub use types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientNotification, ClientRequest,
    CompleteRequest, CompleteResult, CompletionArgument, CompletionReference, Content,
    GetPromptResult, Implementation, ListResourcesResult, ListToolsResult, MessageContent,
    ProgressNotification, ProgressToken, PromptMessage, ProtocolVersion, ReadResourceResult,
    RequestId, ResourceInfo, Role, RootsCapabilities, ServerCapabilities, ServerNotification,
    ServerRequest, ToolCapabilities,
};

// Re-export async_trait for convenience
pub use async_trait::async_trait;

/// Protocol version constants
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

/// Default protocol version to use for negotiation
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

/// List of all protocol versions supported by this SDK
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

/// Default request timeout in milliseconds
pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 60_000;

/// Server-side logging function (placeholder for examples).
///
/// In a real server context, this would send a `LogMessage` notification.
/// For examples, this is a no-op.
#[allow(clippy::unused_async)]
pub async fn log(
    _level: types::protocol::LogLevel,
    _message: &str,
    _data: Option<serde_json::Value>,
) {
    // In a real implementation, this would:
    // 1. Get the current server context
    // 2. Send a LogMessage notification through the transport
    // For now, this is a placeholder for the examples
}
