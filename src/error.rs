//! Error types for the MCP SDK.
//!
//! This module provides a comprehensive error type that covers all possible
//! failure modes in the MCP protocol.

use std::fmt;
use thiserror::Error;

/// Result type alias for MCP operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for MCP operations.
#[derive(Error, Debug)]
pub enum Error {
    /// JSON-RPC protocol errors
    #[error("Protocol error: {code} - {message}")]
    Protocol {
        /// Error code as defined in JSON-RPC spec
        code: ErrorCode,
        /// Human-readable error message
        message: String,
        /// Optional additional error data
        data: Option<serde_json::Value>,
    },

    /// Transport-level errors
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Timeout errors
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Capability errors
    #[error("Capability not supported: {0}")]
    UnsupportedCapability(String),

    /// Internal errors
    #[error("Internal error: {0}")]
    Internal(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Cancelled operation
    #[error("Operation cancelled")]
    Cancelled,

    /// Other errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// JSON-RPC error codes as defined in the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Parse error (-32700)
    ParseError = -32700,
    /// Invalid request (-32600)
    InvalidRequest = -32600,
    /// Method not found (-32601)
    MethodNotFound = -32601,
    /// Invalid params (-32602)
    InvalidParams = -32602,
    /// Internal error (-32603)
    InternalError = -32603,
    /// Custom error codes for MCP
    /// Request timeout (-32001)
    RequestTimeout = -32001,
    /// Unsupported capability (-32002)
    UnsupportedCapability = -32002,
    /// Authentication required (-32003)
    AuthenticationRequired = -32003,
    /// Permission denied (-32004)
    PermissionDenied = -32004,
}

impl ErrorCode {
    /// Convert error code to i32 value.
    pub fn as_i32(&self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
            Self::RequestTimeout => -32001,
            Self::UnsupportedCapability => -32002,
            Self::AuthenticationRequired => -32003,
            Self::PermissionDenied => -32004,
        }
    }

    /// Create error code from i32 value.
    pub fn from_i32(code: i32) -> Self {
        match code {
            -32700 => Self::ParseError,
            -32600 => Self::InvalidRequest,
            -32601 => Self::MethodNotFound,
            -32602 => Self::InvalidParams,
            -32603 => Self::InternalError,
            -32001 => Self::RequestTimeout,
            -32002 => Self::UnsupportedCapability,
            -32003 => Self::AuthenticationRequired,
            -32004 => Self::PermissionDenied,
            _ => Self::InternalError,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_i32())
    }
}

/// Transport-specific errors.
#[derive(Error, Debug)]
pub enum TransportError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// WebSocket error (when feature enabled)
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// HTTP error (when feature enabled)
    #[cfg(feature = "http")]
    #[error("HTTP error: {0}")]
    Http(String),
}

impl Error {
    /// Create a protocol error with the given code and message.
    pub fn protocol(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Protocol {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create a protocol error with additional data.
    pub fn protocol_with_data(
        code: ErrorCode,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self::Protocol {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Create a validation error.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Create a not found error.
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound(resource.into())
    }

    /// Check if this is a protocol error with a specific code.
    pub fn is_error_code(&self, code: ErrorCode) -> bool {
        matches!(self, Self::Protocol { code: c, .. } if *c == code)
    }

    /// Get the error code if this is a protocol error.
    pub fn error_code(&self) -> Option<ErrorCode> {
        match self {
            Self::Protocol { code, .. } => Some(*code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_conversion() {
        assert_eq!(ErrorCode::ParseError.as_i32(), -32700);
        assert_eq!(ErrorCode::from_i32(-32700), ErrorCode::ParseError);

        // Server errors now map to InternalError
        assert_eq!(ErrorCode::from_i32(-32050), ErrorCode::InternalError);
    }

    #[test]
    fn error_creation() {
        let err = Error::protocol(ErrorCode::InvalidRequest, "bad request");
        assert!(err.is_error_code(ErrorCode::InvalidRequest));
        assert_eq!(err.error_code(), Some(ErrorCode::InvalidRequest));

        let err = Error::validation("invalid field");
        assert_eq!(err.error_code(), None);
    }
}
