//! Transport layer abstraction for MCP.
//!
//! This module defines the core `Transport` trait that all transport
//! implementations must satisfy.

use crate::error::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// A message that can be sent/received over a transport.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::TransportMessage;
/// use pmcp::types::{Request, RequestId, JSONRPCResponse, Notification, ProgressNotification, ProgressToken, ClientRequest};
///
/// // Create a request message
/// let request_msg = TransportMessage::Request {
///     id: RequestId::from(1i64),
///     request: Request::Client(Box::new(ClientRequest::Ping)),
/// };
///
/// // Create a response message
/// let response = JSONRPCResponse {
///     jsonrpc: "2.0".to_string(),
///     id: RequestId::from(1i64),
///     payload: pmcp::types::jsonrpc::ResponsePayload::Result(
///         serde_json::json!({"status": "ok"})
///     ),
/// };
/// let response_msg = TransportMessage::Response(response);
///
/// // Create a notification message
/// let notification = Notification::Progress(ProgressNotification {
///     progress_token: ProgressToken::String("task-123".to_string()),
///     progress: 75.0,
///     message: Some("Processing nearly complete".to_string()),
/// });
/// let notification_msg = TransportMessage::Notification(notification);
///
/// // Pattern matching on messages
/// match request_msg {
///     TransportMessage::Request { id, request } => {
///         println!("Received request with ID {:?}", id);
///         match &request {
///             Request::Client(client_req) => {
///                 println!("Client request: {:?}", client_req);
///             }
///             Request::Server(server_req) => {
///                 println!("Server request: {:?}", server_req);
///             }
///         }
///     }
///     TransportMessage::Response(resp) => {
///         println!("Received response for request {:?}", resp.id);
///     }
///     TransportMessage::Notification(notif) => {
///         println!("Received notification");
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum TransportMessage {
    /// Request message with ID
    Request {
        /// Request ID
        id: crate::types::RequestId,
        /// Request payload
        request: crate::types::Request,
    },
    /// Response message
    Response(crate::types::JSONRPCResponse),
    /// Notification message
    Notification(crate::types::Notification),
}

/// Metadata associated with a transport message.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::transport::{MessageMetadata, MessagePriority};
///
/// // Create default metadata
/// let default_meta = MessageMetadata::default();
/// assert!(default_meta.content_type.is_none());
/// assert!(!default_meta.flush);
///
/// // Create metadata with specific settings
/// let meta = MessageMetadata {
///     content_type: Some("application/json".to_string()),
///     priority: Some(MessagePriority::High),
///     flush: true,
/// };
///
/// // Use in transport implementations
/// fn should_flush_immediately(meta: &MessageMetadata) -> bool {
///     meta.flush || matches!(meta.priority, Some(MessagePriority::High))
/// }
///
/// assert!(should_flush_immediately(&meta));
/// ```
#[derive(Debug, Clone, Default)]
pub struct MessageMetadata {
    /// Content type (e.g., "application/json")
    pub content_type: Option<String>,
    /// Message priority
    pub priority: Option<MessagePriority>,
    /// Whether this message should be flushed immediately
    pub flush: bool,
}

/// Message priority levels.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::transport::MessagePriority;
///
/// // Priority levels are ordered
/// assert!(MessagePriority::Low < MessagePriority::Normal);
/// assert!(MessagePriority::Normal < MessagePriority::High);
///
/// // Default is Normal
/// let default_priority = MessagePriority::default();
/// assert_eq!(default_priority, MessagePriority::Normal);
///
/// // Use in message queue prioritization
/// let mut messages = vec![
///     ("msg1", MessagePriority::Low),
///     ("msg2", MessagePriority::High),
///     ("msg3", MessagePriority::Normal),
/// ];
///
/// // Sort by priority (highest first)
/// messages.sort_by_key(|(_, priority)| std::cmp::Reverse(*priority));
/// assert_eq!(messages[0].0, "msg2"); // High priority first
/// assert_eq!(messages[1].0, "msg3"); // Normal priority second
/// assert_eq!(messages[2].0, "msg1"); // Low priority last
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    /// Low priority
    Low,
    /// Normal priority (default)
    Normal,
    /// High priority
    High,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Core transport trait for MCP communication.
///
/// All transport implementations (stdio, WebSocket, HTTP) must implement
/// this trait to be usable with the MCP client/server.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{Transport, TransportMessage};
/// use async_trait::async_trait;
///
/// #[derive(Debug)]
/// struct MyTransport;
///
/// #[async_trait]
/// impl Transport for MyTransport {
///     async fn send(&mut self, message: TransportMessage) -> pmcp::Result<()> {
///         // Send implementation
///         Ok(())
///     }
///
///     async fn receive(&mut self) -> pmcp::Result<TransportMessage> {
///         // Receive implementation  
///         Ok(TransportMessage::Notification(
///             pmcp::types::Notification::Progress(pmcp::types::ProgressNotification {
///                 progress_token: pmcp::types::ProgressToken::String("example".to_string()),
///                 progress: 50.0,
///                 message: Some("Processing...".to_string()),
///             })
///         ))
///     }
///
///     async fn close(&mut self) -> pmcp::Result<()> {
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Transport: Send + Sync + Debug {
    /// Send a message over the transport.
    ///
    /// This method should handle framing and ensure the entire message
    /// is sent atomically.
    async fn send(&mut self, message: TransportMessage) -> Result<()>;

    /// Receive a message from the transport.
    ///
    /// This method should block until a complete message is available.
    /// It should handle any necessary buffering and framing internally.
    async fn receive(&mut self) -> Result<TransportMessage>;

    /// Close the transport.
    ///
    /// After calling this method, the transport should not accept any
    /// more messages for sending or receiving.
    async fn close(&mut self) -> Result<()>;

    /// Check if the transport is still connected.
    ///
    /// Default implementation always returns true.
    fn is_connected(&self) -> bool {
        true
    }

    /// Get the transport type name for debugging.
    fn transport_type(&self) -> &'static str {
        "unknown"
    }
}

/// Options for sending messages.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::transport::{SendOptions, MessagePriority};
/// use std::time::Duration;
///
/// // Default options
/// let default_opts = SendOptions::default();
/// assert!(default_opts.priority.is_none());
/// assert!(!default_opts.flush);
/// assert!(default_opts.timeout.is_none());
///
/// // High priority message with immediate flush
/// let urgent_opts = SendOptions {
///     priority: Some(MessagePriority::High),
///     flush: true,
///     timeout: Some(Duration::from_secs(5)),
/// };
///
/// // Builder pattern for options
/// fn build_send_options(urgent: bool) -> SendOptions {
///     SendOptions {
///         priority: if urgent {
///             Some(MessagePriority::High)
///         } else {
///             Some(MessagePriority::Normal)
///         },
///         flush: urgent,
///         timeout: Some(Duration::from_secs(if urgent { 5 } else { 30 })),
///     }
/// }
///
/// let opts = build_send_options(true);
/// assert_eq!(opts.priority, Some(MessagePriority::High));
/// assert!(opts.flush);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Message priority
    pub priority: Option<MessagePriority>,
    /// Whether to flush immediately after sending
    pub flush: bool,
    /// Timeout for the send operation
    pub timeout: Option<std::time::Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(MessagePriority::Low < MessagePriority::Normal);
        assert!(MessagePriority::Normal < MessagePriority::High);
    }
}
