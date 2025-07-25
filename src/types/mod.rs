//! Core protocol types for MCP.
//!
//! This module contains all the type definitions for the Model Context Protocol,
//! including requests, responses, notifications, and capability definitions.

pub mod auth;
pub mod capabilities;
pub mod jsonrpc;
pub mod protocol;

// Re-export commonly used types
pub use auth::{AuthInfo, AuthScheme};
pub use capabilities::{
    ClientCapabilities, LoggingCapabilities, PromptCapabilities, ResourceCapabilities,
    SamplingCapabilities, ServerCapabilities, ToolCapabilities,
};
pub use jsonrpc::{JSONRPCError, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId};
pub use protocol::{
    CallToolParams, CallToolResult, ClientNotification, ClientRequest, Content, GetPromptParams,
    GetPromptResult, Implementation, InitializeParams, InitializeResult, ListPromptsParams,
    ListPromptsResult, ListResourcesParams, ListResourcesResult, ListToolsParams, ListToolsResult,
    ModelPreferences, Progress, ProgressToken, PromptInfo, ProtocolVersion, ReadResourceParams,
    ReadResourceResult, ResourceInfo, Role, ServerNotification, ServerRequest, ToolInfo,
};
