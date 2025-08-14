//! Constants for HTTP headers and content types used in MCP.

// Header Names
/// MCP session ID header name
pub const MCP_SESSION_ID: &str = "mcp-session-id";

/// MCP protocol version header name
pub const MCP_PROTOCOL_VERSION: &str = "mcp-protocol-version";

/// SSE Last-Event-ID header name for resumption
pub const LAST_EVENT_ID: &str = "Last-Event-ID";

/// HTTP Accept header name
pub const ACCEPT: &str = "Accept";

/// HTTP Content-Type header name
pub const CONTENT_TYPE: &str = "Content-Type";

// Content Types
/// JSON content type value
pub const APPLICATION_JSON: &str = "application/json";

/// Server-Sent Events content type value
pub const TEXT_EVENT_STREAM: &str = "text/event-stream";

/// Accept header value for streamable HTTP (both JSON and SSE)
pub const ACCEPT_STREAMABLE: &str = "application/json, text/event-stream";
