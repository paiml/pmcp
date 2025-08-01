//! Core protocol types for MCP.
//!
//! This module contains all the type definitions for the Model Context Protocol,
//! including requests, responses, notifications, and capability definitions.

pub mod auth;
pub mod capabilities;
pub mod completable;
pub mod elicitation;
pub mod jsonrpc;
pub mod protocol;

// Re-export transport message type
pub use crate::shared::transport::TransportMessage;

// Re-export protocol version constants
pub use crate::{DEFAULT_PROTOCOL_VERSION, LATEST_PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS};

// Re-export commonly used types
pub use auth::{AuthInfo, AuthScheme};
pub use capabilities::{
    ClientCapabilities, CompletionCapabilities, LoggingCapabilities, PromptCapabilities,
    ResourceCapabilities, RootsCapabilities, SamplingCapabilities, ServerCapabilities,
    ToolCapabilities,
};
pub use jsonrpc::{JSONRPCError, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId};
pub use protocol::{
    CallToolParams, CallToolRequest, CallToolResult, CancelledNotification, CancelledParams,
    ClientNotification, ClientRequest, CompleteRequest, CompleteResult, CompletionArgument,
    CompletionReference, CompletionResult, Content, CreateMessageParams, CreateMessageRequest,
    CreateMessageResult, GetPromptParams, GetPromptRequest, GetPromptResult, Implementation,
    IncludeContext, InitializeParams, InitializeRequest, InitializeResult, ListPromptsParams,
    ListPromptsRequest, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesParams, ListResourcesRequest, ListResourcesResult,
    ListToolsParams, ListToolsRequest, ListToolsResult, LoggingLevel, MessageContent, ModelHint,
    ModelPreferences, Notification, Progress, ProgressNotification, ProgressToken, PromptArgument,
    PromptInfo, PromptMessage, ProtocolVersion, ReadResourceParams, ReadResourceRequest,
    ReadResourceResult, Request, ResourceInfo, ResourceTemplate, Role, SamplingMessage,
    ServerNotification, ServerRequest, SubscribeRequest, TokenUsage, ToolInfo, UnsubscribeRequest,
};
