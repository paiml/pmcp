//! Transport layer abstraction for MCP.
//!
//! This module defines the core `Transport` trait that all transport
//! implementations must satisfy.

use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::fmt::Debug;

/// A message that can be sent/received over a transport.
#[derive(Debug, Clone)]
pub struct TransportMessage {
    /// The message payload (typically JSON)
    pub payload: Bytes,
    /// Optional message metadata
    pub metadata: Option<MessageMetadata>,
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
/// use bytes::Bytes;
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
///         Ok(TransportMessage {
///             payload: Bytes::from("{}"),
///             metadata: None,
///         })
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

impl TransportMessage {
    /// Create a new transport message from bytes.
    pub fn new(payload: impl Into<Bytes>) -> Self {
        Self {
            payload: payload.into(),
            metadata: None,
        }
    }

    /// Create a message with metadata.
    pub fn with_metadata(payload: impl Into<Bytes>, metadata: MessageMetadata) -> Self {
        Self {
            payload: payload.into(),
            metadata: Some(metadata),
        }
    }

    /// Set the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(Default::default)
            .content_type = Some(content_type.into());
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.metadata.get_or_insert_with(Default::default).priority = Some(priority);
        self
    }

    /// Mark this message for immediate flushing.
    pub fn flush(mut self) -> Self {
        self.metadata.get_or_insert_with(Default::default).flush = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_builder() {
        let msg = TransportMessage::new(&b"test"[..]);
        assert_eq!(msg.payload.as_ref(), b"test");
        assert!(msg.metadata.is_none());

        let msg = TransportMessage::new(&b"test"[..])
            .with_content_type("application/json")
            .with_priority(MessagePriority::High)
            .flush();

        let metadata = msg.metadata.unwrap();
        assert_eq!(metadata.content_type.as_deref(), Some("application/json"));
        assert_eq!(metadata.priority, Some(MessagePriority::High));
        assert!(metadata.flush);
    }

    #[test]
    fn priority_ordering() {
        assert!(MessagePriority::Low < MessagePriority::Normal);
        assert!(MessagePriority::Normal < MessagePriority::High);
    }
}
