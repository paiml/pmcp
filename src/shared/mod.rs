//! Shared components used by both client and server.

pub mod batch;
pub mod context;
pub mod logging;
pub mod middleware;
pub mod protocol;
pub mod protocol_helpers;
pub mod reconnect;
pub mod session;
pub mod sse;
pub mod stdio;
pub mod transport;
pub mod uri_template;

// Cross-platform runtime abstraction
pub mod runtime;

// Platform-specific WebSocket modules
#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub mod websocket;

#[cfg(all(feature = "websocket", target_arch = "wasm32"))]
pub mod wasm_websocket;

#[cfg(feature = "http")]
pub mod http;
pub mod http_constants;

#[cfg(feature = "streamable-http")]
/// Streamable HTTP transport implementation for MCP.
pub mod streamable_http;

// Re-export commonly used types
pub use batch::{BatchRequest, BatchResponse};
pub use context::{ClientInfo, ContextPropagator, RequestContext};
pub use logging::{init_logging, CorrelatedLogger, LogConfig, LogEntry, LogFormat, LogLevel};
pub use middleware::{
    AuthMiddleware, LoggingMiddleware, Middleware, MiddlewareChain, RetryMiddleware,
};
pub use protocol::{ProgressCallback, Protocol, ProtocolOptions, RequestOptions};
pub use protocol_helpers::{
    create_notification, create_request, parse_notification, parse_request,
};
pub use reconnect::{ReconnectConfig, ReconnectGuard, ReconnectManager};
pub use session::{Session, SessionConfig, SessionManager};
pub use stdio::StdioTransport;
pub use transport::{Transport, TransportMessage};

#[cfg(all(feature = "websocket", not(target_arch = "wasm32")))]
pub use websocket::{WebSocketConfig, WebSocketTransport};

#[cfg(all(feature = "websocket", target_arch = "wasm32"))]
pub use wasm_websocket::{WasmWebSocketConfig, WasmWebSocketTransport};

#[cfg(feature = "http")]
pub use http::{HttpConfig, HttpTransport};

#[cfg(feature = "streamable-http")]
pub use streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
