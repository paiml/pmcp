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

/// JSON-RPC error code for custom errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCode(pub i32);

impl ErrorCode {
    /// Parse error (-32700)
    pub const PARSE_ERROR: Self = Self(-32700);
    /// Invalid request (-32600)
    pub const INVALID_REQUEST: Self = Self(-32600);
    /// Method not found (-32601)
    pub const METHOD_NOT_FOUND: Self = Self(-32601);
    /// Invalid params (-32602)
    pub const INVALID_PARAMS: Self = Self(-32602);
    /// Internal error (-32603)
    pub const INTERNAL_ERROR: Self = Self(-32603);
    /// Request timeout (-32001)
    pub const REQUEST_TIMEOUT: Self = Self(-32001);
    /// Unsupported capability (-32002)
    pub const UNSUPPORTED_CAPABILITY: Self = Self(-32002);
    /// Authentication required (-32003)
    pub const AUTHENTICATION_REQUIRED: Self = Self(-32003);
    /// Permission denied (-32004)
    pub const PERMISSION_DENIED: Self = Self(-32004);

    /// Create a custom error code.
    pub const fn other(code: i32) -> Self {
        Self(code)
    }

    /// Convert error code to i32 value.
    pub fn as_i32(&self) -> i32 {
        self.0
    }

    /// Create error code from i32 value.
    pub fn from_i32(code: i32) -> Self {
        match code {
            -32700 => Self::PARSE_ERROR,
            -32600 => Self::INVALID_REQUEST,
            -32601 => Self::METHOD_NOT_FOUND,
            -32602 => Self::INVALID_PARAMS,
            -32603 => Self::INTERNAL_ERROR,
            -32001 => Self::REQUEST_TIMEOUT,
            -32002 => Self::UNSUPPORTED_CAPABILITY,
            -32003 => Self::AUTHENTICATION_REQUIRED,
            -32004 => Self::PERMISSION_DENIED,
            // Map server errors to internal error
            -32099..=-32000 => Self::INTERNAL_ERROR,
            other => Self::other(other),
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
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

    /// Create a protocol error with just a message (defaults to `InternalError` code).
    pub fn protocol_msg(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INTERNAL_ERROR,
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

    /// Create a parse error.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::PARSE_ERROR,
            message: message.into(),
            data: None,
        }
    }

    /// Create an invalid request error.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_REQUEST,
            message: message.into(),
            data: None,
        }
    }

    /// Create a method not found error.
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    /// Create an invalid params error.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_PARAMS,
            message: message.into(),
            data: None,
        }
    }

    /// Create an authentication error.
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create a capability error.
    pub fn capability(message: impl Into<String>) -> Self {
        Self::UnsupportedCapability(message.into())
    }

    /// Create a resource not found error.
    pub fn resource_not_found(uri: impl Into<String>) -> Self {
        Self::NotFound(format!("Resource not found: {}", uri.into()))
    }

    /// Create from JSON-RPC error.
    pub fn from_jsonrpc_error(error: crate::types::JSONRPCError) -> Self {
        Self::Protocol {
            code: ErrorCode::from_i32(error.code),
            message: error.message,
            data: error.data,
        }
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
        assert_eq!(ErrorCode::PARSE_ERROR.as_i32(), -32700);
        assert_eq!(ErrorCode::from_i32(-32700), ErrorCode::PARSE_ERROR);

        // Server errors now map to InternalError
        assert_eq!(ErrorCode::from_i32(-32050), ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn error_creation() {
        let err = Error::protocol(ErrorCode::INVALID_REQUEST, "bad request");
        assert!(err.is_error_code(ErrorCode::INVALID_REQUEST));
        assert_eq!(err.error_code(), Some(ErrorCode::INVALID_REQUEST));

        let err = Error::validation("invalid field");
        assert_eq!(err.error_code(), None);
    }
}
