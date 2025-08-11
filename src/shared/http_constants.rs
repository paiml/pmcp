//! Constants for HTTP headers and content types used in MCP.

// Header Names
pub const MCP_SESSION_ID: &str = "mcp-session-id";
pub const MCP_PROTOCOL_VERSION: &str = "mcp-protocol-version";
pub const LAST_EVENT_ID: &str = "Last-Event-ID";
pub const ACCEPT: &str = "Accept";
pub const CONTENT_TYPE: &str = "Content-Type";

// Content Types
pub const APPLICATION_JSON: &str = "application/json";
pub const TEXT_EVENT_STREAM: &str = "text/event-stream";
pub const ACCEPT_STREAMABLE: &str = "application/json, text/event-stream";
