//! Shared components used by both client and server.

pub mod batch;
pub mod middleware;
pub mod protocol;
pub mod protocol_helpers;
pub mod stdio;
pub mod transport;
pub mod uri_template;

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(feature = "http")]
pub mod http;

// Re-export commonly used types
pub use batch::{BatchRequest, BatchResponse};
pub use middleware::{
    AuthMiddleware, LoggingMiddleware, Middleware, MiddlewareChain, RetryMiddleware,
};
pub use protocol::{ProgressCallback, Protocol, ProtocolOptions, RequestOptions};
pub use protocol_helpers::{
    create_notification, create_request, parse_notification, parse_request,
};
pub use stdio::StdioTransport;
pub use transport::{Transport, TransportMessage};

#[cfg(feature = "websocket")]
pub use websocket::{WebSocketConfig, WebSocketTransport};

#[cfg(feature = "http")]
pub use http::{HttpConfig, HttpTransport};
