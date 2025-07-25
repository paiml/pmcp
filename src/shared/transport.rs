//! Transport layer abstraction for MCP.
//!
//! This module defines the core `Transport` trait that all transport
//! implementations must satisfy.

use crate::error::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// A message that can be sent/received over a transport.
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
