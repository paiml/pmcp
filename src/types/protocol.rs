//! MCP protocol-specific types.
//!
//! This module contains all the protocol-specific request, response, and
//! notification types defined by the MCP specification.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::capabilities::{ClientCapabilities, ServerCapabilities};

/// Protocol version identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub String);

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self(crate::DEFAULT_PROTOCOL_VERSION.to_string())
    }
}

/// Implementation information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Implementation name (e.g., "mcp-sdk-rust")
    pub name: String,
    /// Implementation version
    pub version: String,
}

/// Initialize request parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Protocol version the client wants to use
    pub protocol_version: ProtocolVersion,
    /// Client capabilities
    pub capabilities: ClientCapabilities,
    /// Client implementation info
    pub client_info: Implementation,
}

/// Initialize response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Negotiated protocol version
    pub protocol_version: ProtocolVersion,
    /// Server capabilities
    pub capabilities: ServerCapabilities,
    /// Server implementation info
    pub server_info: Implementation,
}

/// Pagination cursor.
pub type Cursor = Option<String>;

/// List tools request parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsParams {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Tool information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Tool name (unique identifier)
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for tool parameters
    pub input_schema: Value,
}

/// List tools response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Available tools
    pub tools: Vec<ToolInfo>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

/// Tool call parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolParams {
    /// Tool name to invoke
    pub name: String,
    /// Tool arguments (must match input schema)
    #[serde(default)]
    pub arguments: Value,
}

/// Tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Tool execution result
    #[serde(default)]
    pub content: Vec<Content>,
    /// Whether the tool call represents an error
    #[serde(default)]
    pub is_error: bool,
}

/// Content item in responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    /// Text content
    #[serde(rename_all = "camelCase")]
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    #[serde(rename_all = "camelCase")]
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
    },
    /// Resource reference
    #[serde(rename_all = "camelCase")]
    Resource {
        /// Resource URI
        uri: String,
        /// Optional resource content
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// MIME type
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

/// List prompts parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsParams {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Prompt information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    /// Prompt name (unique identifier)
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt arguments schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the argument is required
    #[serde(default)]
    pub required: bool,
}

/// List prompts response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompts
    pub prompts: Vec<PromptInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

/// Get prompt parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptParams {
    /// Prompt name
    pub name: String,
    /// Prompt arguments
    #[serde(default)]
    pub arguments: IndexMap<String, String>,
}

/// Get prompt result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt messages
    pub messages: Vec<PromptMessage>,
}

/// Message in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMessage {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: Content,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// User message
    User,
    /// Assistant message
    Assistant,
}

/// List resources parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesParams {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Cursor,
}

/// Resource information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    /// Resource URI
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// List resources response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Available resources
    pub resources: Vec<ResourceInfo>,
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Cursor,
}

/// Read resource parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceParams {
    /// Resource URI
    pub uri: String,
}

/// Read resource result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    /// Resource contents
    pub contents: Vec<Content>,
}

/// Model preferences for sampling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreferences {
    /// Hints for model selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    /// Cost priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f64>,
    /// Speed priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f64>,
    /// Intelligence priority (0-1, higher = more important)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f64>,
}

/// Model hint for sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHint {
    /// Model name/identifier hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Progress notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Progress token from the original request
    pub progress_token: ProgressToken,
    /// Progress percentage (0-100)
    pub progress: f64,
    /// Optional progress message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Progress token type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProgressToken {
    /// String token
    String(String),
    /// Numeric token
    Number(i64),
}

/// Client request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientRequest {
    /// Initialize the connection
    #[serde(rename = "initialize")]
    Initialize(InitializeParams),
    /// List available tools
    #[serde(rename = "tools/list")]
    ListTools(ListToolsParams),
    /// Call a tool
    #[serde(rename = "tools/call")]
    CallTool(CallToolParams),
    /// List available prompts
    #[serde(rename = "prompts/list")]
    ListPrompts(ListPromptsParams),
    /// Get a prompt
    #[serde(rename = "prompts/get")]
    GetPrompt(GetPromptParams),
    /// List available resources
    #[serde(rename = "resources/list")]
    ListResources(ListResourcesParams),
    /// Read a resource
    #[serde(rename = "resources/read")]
    ReadResource(ReadResourceParams),
    /// Ping request
    #[serde(rename = "ping")]
    Ping,
}

/// Server request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerRequest {
    /// Request to create a message (sampling)
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(CreateMessageParams),
}

/// Create message parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageParams {
    /// Messages to sample from
    pub messages: Vec<SamplingMessage>,
    /// Optional model preferences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Include context from MCP
    #[serde(default)]
    pub include_context: IncludeContext,
    /// Temperature (0-1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Additional model-specific parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Sampling message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingMessage {
    /// Message role
    pub role: Role,
    /// Message content
    pub content: Content,
}

/// Context to include in sampling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IncludeContext {
    /// Include all context
    All,
    /// Include no context
    None,
    /// Include specific context types
    ThisServerOnly,
}

impl Default for IncludeContext {
    fn default() -> Self {
        Self::None
    }
}

/// Client notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ClientNotification {
    /// Notification that client has been initialized
    #[serde(rename = "notifications/initialized")]
    Initialized,
    /// Notification that a request was cancelled
    #[serde(rename = "notifications/cancelled")]
    Cancelled(CancelledParams),
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(Progress),
}

/// Cancelled notification parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledParams {
    /// The request ID that was cancelled
    pub request_id: crate::types::RequestId,
    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Server notification types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerNotification {
    /// Progress update
    #[serde(rename = "notifications/progress")]
    Progress(Progress),
    /// Tools have changed
    #[serde(rename = "notifications/tools/list_changed")]
    ToolsChanged,
    /// Prompts have changed
    #[serde(rename = "notifications/prompts/list_changed")]
    PromptsChanged,
    /// Resources have changed
    #[serde(rename = "notifications/resources/list_changed")]
    ResourcesChanged,
    /// Resource was updated
    #[serde(rename = "notifications/resources/updated")]
    ResourceUpdated(ResourceUpdatedParams),
    /// Log message
    #[serde(rename = "notifications/message")]
    LogMessage(LogMessageParams),
}

/// Resource updated notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdatedParams {
    /// Resource URI that was updated
    pub uri: String,
}

/// Log message notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogMessageParams {
    /// Log level
    pub level: LogLevel,
    /// Logger name/category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// Log message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warning,
    /// Error level
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_client_request() {
        let req = ClientRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "ping");

        let req = ClientRequest::ListTools(ListToolsParams::default());
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "tools/list");
    }

    #[test]
    fn serialize_content() {
        let content = Content::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn tool_info_serialization() {
        let tool = ToolInfo {
            name: "test-tool".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }),
        };

        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "test-tool");
        assert_eq!(json["description"], "A test tool");
        assert_eq!(json["inputSchema"]["type"], "object");
    }
}
