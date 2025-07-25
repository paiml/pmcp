//! JSON-RPC 2.0 protocol types.
//!
//! This module provides the core JSON-RPC types used by MCP.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::fmt;

/// JSON-RPC version constant.
pub const JSONRPC_VERSION: &str = "2.0";

/// A request ID in JSON-RPC.
///
/// Can be either a string or number according to the JSON-RPC spec.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::types::RequestId;
/// use serde_json::json;
///
/// let string_id = RequestId::String("req-123".to_string());
/// let number_id = RequestId::Number(42);
///
/// assert_eq!(json!(string_id), json!("req-123"));
/// assert_eq!(json!(number_id), json!(42));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// String request ID
    String(String),
    /// Numeric request ID
    Number(i64),
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => write!(f, "{}", s),
            Self::Number(n) => write!(f, "{}", n),
        }
    }
}

impl From<String> for RequestId {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for RequestId {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        Self::Number(n)
    }
}

impl From<u64> for RequestId {
    fn from(n: u64) -> Self {
        // Use try_into to safely convert, defaulting to max i64 if overflow
        let num = i64::try_from(n).unwrap_or(i64::MAX);
        Self::Number(num)
    }
}

/// A JSON-RPC request that expects a response.
///
/// # Examples
///
/// ```rust
/// use mcp_sdk::types::{JSONRPCRequest, RequestId};
/// use serde_json::json;
///
/// let request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     id: RequestId::from(1i64),
///     method: "tools/list".to_string(),
///     params: Some(json!({"cursor": null})),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONRPCRequest<P = serde_json::Value> {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Unique request identifier
    pub id: RequestId,
    /// Method name to invoke
    pub method: String,
    /// Optional method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
}

impl<P> JSONRPCRequest<P> {
    /// Create a new JSON-RPC request.
    pub fn new(id: impl Into<RequestId>, method: impl Into<String>, params: Option<P>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    /// Validate that this is a valid JSON-RPC request.
    pub fn validate(&self) -> Result<(), crate::Error> {
        if self.jsonrpc != JSONRPC_VERSION {
            return Err(crate::Error::validation(format!(
                "Invalid JSON-RPC version: expected {}, got {}",
                JSONRPC_VERSION, self.jsonrpc
            )));
        }
        Ok(())
    }
}

/// A JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONRPCResponse<R = serde_json::Value, E = JSONRPCError> {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Request ID this response corresponds to
    pub id: RequestId,
    /// Either result or error must be present
    #[serde(flatten)]
    pub payload: ResponsePayload<R, E>,
}

/// Response payload - either a result or an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsePayload<R, E> {
    /// Successful result
    Result(R),
    /// Error response
    Error(E),
}

impl<R, E> JSONRPCResponse<R, E> {
    /// Create a successful response.
    pub fn success(id: RequestId, result: R) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            payload: ResponsePayload::Result(result),
        }
    }

    /// Create an error response.
    pub fn error(id: RequestId, error: E) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            payload: ResponsePayload::Error(error),
        }
    }

    /// Check if this is a successful response.
    pub fn is_success(&self) -> bool {
        matches!(self.payload, ResponsePayload::Result(_))
    }

    /// Check if this is an error response.
    pub fn is_error(&self) -> bool {
        matches!(self.payload, ResponsePayload::Error(_))
    }

    /// Get the result if this is a successful response.
    pub fn result(&self) -> Option<&R> {
        match &self.payload {
            ResponsePayload::Result(r) => Some(r),
            ResponsePayload::Error(_) => None,
        }
    }

    /// Get the error if this is an error response.
    pub fn get_error(&self) -> Option<&E> {
        match &self.payload {
            ResponsePayload::Error(e) => Some(e),
            ResponsePayload::Result(_) => None,
        }
    }
}

/// A JSON-RPC notification (no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONRPCNotification<P = serde_json::Value> {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Method name
    pub method: String,
    /// Optional parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
}

impl<P> JSONRPCNotification<P> {
    /// Create a new notification.
    pub fn new(method: impl Into<String>, params: Option<P>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONRPCError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Optional additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JSONRPCError {
    /// Create a new JSON-RPC error.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data.
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

impl From<crate::Error> for JSONRPCError {
    fn from(err: crate::Error) -> Self {
        match &err {
            crate::Error::Protocol {
                code,
                message,
                data,
            } => Self {
                code: code.as_i32(),
                message: message.clone(),
                data: data.clone(),
            },
            _ => Self::new(-32603, err.to_string()),
        }
    }
}

/// Raw JSON-RPC message for efficient parsing.
#[derive(Debug, Deserialize)]
pub struct RawMessage {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID (if present)
    #[serde(default)]
    pub id: Option<RequestId>,
    /// Method name (for requests/notifications)
    #[serde(default)]
    pub method: Option<String>,
    /// Raw params
    #[serde(default)]
    pub params: Option<Box<RawValue>>,
    /// Raw result (for responses)
    #[serde(default)]
    pub result: Option<Box<RawValue>>,
    /// Error (for error responses)
    #[serde(default)]
    pub error: Option<JSONRPCError>,
}

impl RawMessage {
    /// Determine the type of this message.
    pub fn message_type(&self) -> MessageType {
        match (&self.id, &self.method, &self.result, &self.error) {
            (Some(_), Some(_), None, None) => MessageType::Request,
            (None, Some(_), None, None) => MessageType::Notification,
            (Some(_), None, Some(_), None) => MessageType::Response,
            (Some(_), None, None, Some(_)) => MessageType::ErrorResponse,
            _ => MessageType::Invalid,
        }
    }
}

/// Type of JSON-RPC message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// Request (has id and method)
    Request,
    /// Notification (has method but no id)
    Notification,
    /// Success response (has id and result)
    Response,
    /// Error response (has id and error)
    ErrorResponse,
    /// Invalid message format
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_id_conversion() {
        assert_eq!(
            RequestId::from("test"),
            RequestId::String("test".to_string())
        );
        assert_eq!(RequestId::from(42i64), RequestId::Number(42));
        assert_eq!(RequestId::from(42u64), RequestId::Number(42));
    }

    #[test]
    fn request_serialization() {
        let request = JSONRPCRequest::new(1i64, "test/method", Some(json!({"key": "value"})));
        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "test/method");
        assert_eq!(json["params"]["key"], "value");
    }

    #[test]
    fn response_success() {
        let response: JSONRPCResponse<serde_json::Value, JSONRPCError> =
            JSONRPCResponse::success(RequestId::from(1i64), json!({"result": true}));
        assert!(response.is_success());
        assert!(!response.is_error());
        assert_eq!(response.result(), Some(&json!({"result": true})));
    }

    #[test]
    fn response_error() {
        let error = JSONRPCError::new(-32600, "Invalid request");
        let response: JSONRPCResponse<serde_json::Value, JSONRPCError> =
            JSONRPCResponse::error(RequestId::from(1i64), error);
        assert!(!response.is_success());
        assert!(response.is_error());
        assert_eq!(response.get_error().unwrap().code, -32600);
    }

    #[test]
    fn notification_serialization() {
        let notification = JSONRPCNotification::new("test/notify", None::<serde_json::Value>);
        let json = serde_json::to_value(&notification).unwrap();

        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "test/notify");
        assert_eq!(json.get("params"), None);
    }
}
