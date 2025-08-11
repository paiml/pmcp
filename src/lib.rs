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
//!     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value, pmcp::Error> {
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

#[cfg(feature = "simd")]
pub mod simd;

// Re-export commonly used types
pub use client::{Client, ClientBuilder};
pub use error::{Error, ErrorCode, Result};
pub use server::{
    cancellation::RequestHandlerExtra, PromptHandler, ResourceHandler, SamplingHandler, Server,
    ServerBuilder, ToolHandler,
};
pub use shared::{
    batch::{BatchRequest, BatchResponse},
    uri_template::UriTemplate,
    AuthMiddleware, LoggingMiddleware, Middleware, MiddlewareChain, RetryMiddleware,
    StdioTransport, Transport,
};

#[cfg(feature = "websocket")]
pub use shared::{WebSocketConfig, WebSocketTransport};

#[cfg(feature = "http")]
pub use shared::{HttpConfig, HttpTransport};
pub use types::{
    AuthInfo, AuthScheme, CallToolRequest, CallToolResult, ClientCapabilities, ClientNotification,
    ClientRequest, CompleteRequest, CompleteResult, CompletionArgument, CompletionReference,
    Content, CreateMessageParams, CreateMessageRequest, CreateMessageResult, GetPromptResult,
    Implementation, IncludeContext, ListResourcesResult, ListToolsResult, LoggingLevel,
    MessageContent, ModelPreferences, ProgressNotification, ProgressToken, PromptMessage,
    ProtocolVersion, ReadResourceResult, RequestId, ResourceInfo, Role, RootsCapabilities,
    SamplingCapabilities, SamplingMessage, ServerCapabilities, ServerNotification, ServerRequest,
    TokenUsage, ToolCapabilities, ToolInfo,
};
pub use utils::{BatchingConfig, DebouncingConfig, MessageBatcher, MessageDebouncer};

// Re-export async_trait for convenience
pub use async_trait::async_trait;

/// Protocol version constants
///
/// # Examples
///
/// ```rust
/// use pmcp::LATEST_PROTOCOL_VERSION;
///
/// // Use in client initialization
/// let protocol_version = LATEST_PROTOCOL_VERSION;
/// println!("Using MCP protocol version: {}", protocol_version);
///
/// // Check if a version is the latest
/// assert_eq!(LATEST_PROTOCOL_VERSION, "2025-06-18");
/// ```
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

/// Default protocol version to use for negotiation
///
/// # Examples
///
/// ```rust
/// use pmcp::DEFAULT_PROTOCOL_VERSION;
///
/// // Use as fallback when negotiating protocol version
/// let negotiated_version = DEFAULT_PROTOCOL_VERSION;
/// println!("Negotiating with protocol version: {}", negotiated_version);
///
/// // This is typically used internally by the SDK
/// assert_eq!(DEFAULT_PROTOCOL_VERSION, "2025-03-26");
/// ```
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

/// List of all protocol versions supported by this SDK
///
/// # Examples
///
/// ```rust
/// use pmcp::SUPPORTED_PROTOCOL_VERSIONS;
///
/// // Check if a version is supported
/// let version_to_check = "2025-03-26";
/// let is_supported = SUPPORTED_PROTOCOL_VERSIONS.contains(&version_to_check);
/// assert!(is_supported);
///
/// // List all supported versions
/// println!("Supported MCP protocol versions:");
/// for version in SUPPORTED_PROTOCOL_VERSIONS {
///     println!("  - {}", version);
/// }
///
/// // Use in version negotiation
/// fn negotiate_version(client_version: &str) -> Option<&'static str> {
///     SUPPORTED_PROTOCOL_VERSIONS.iter()
///         .find(|&&v| v == client_version)
///         .copied()
/// }
/// ```
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    LATEST_PROTOCOL_VERSION,
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

/// Default request timeout in milliseconds
///
/// # Examples
///
/// ```rust
/// use pmcp::DEFAULT_REQUEST_TIMEOUT_MS;
/// use std::time::Duration;
///
/// // Convert to Duration for use with timeouts
/// let timeout = Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS);
/// println!("Default timeout: {:?}", timeout);
///
/// // Use in custom transport configuration
/// struct TransportConfig {
///     timeout_ms: u64,
/// }
///
/// impl Default for TransportConfig {
///     fn default() -> Self {
///         Self {
///             timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
///         }
///     }
/// }
///
/// // Verify default value
/// assert_eq!(DEFAULT_REQUEST_TIMEOUT_MS, 60_000); // 60 seconds
/// ```
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
