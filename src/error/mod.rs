//! Error types for the MCP SDK.
//!
//! This module provides a comprehensive error type that covers all possible
//! failure modes in the MCP protocol.

pub mod recovery;

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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::error::ErrorCode;
    ///
    /// // Create application-specific error codes
    /// const RATE_LIMIT_ERROR: ErrorCode = ErrorCode::other(-32010);
    /// const QUOTA_EXCEEDED: ErrorCode = ErrorCode::other(-32011);
    /// const FEATURE_DISABLED: ErrorCode = ErrorCode::other(-32012);
    ///
    /// // Use in error handling
    /// # use pmcp::Error;
    /// fn check_rate_limit(count: u32) -> Result<(), Error> {
    ///     if count > 100 {
    ///         return Err(Error::protocol(
    ///             RATE_LIMIT_ERROR,
    ///             "Rate limit exceeded: max 100 requests per minute"
    ///         ));
    ///     }
    ///     Ok(())
    /// }
    ///
    /// // Server-specific error codes (reserved range: -32099 to -32000)
    /// const SERVER_MAINTENANCE: ErrorCode = ErrorCode::other(-32050);
    /// const DATABASE_ERROR: ErrorCode = ErrorCode::other(-32051);
    /// ```
    pub const fn other(code: i32) -> Self {
        Self(code)
    }

    /// Convert error code to i32 value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::error::ErrorCode;
    ///
    /// // Standard error codes
    /// assert_eq!(ErrorCode::PARSE_ERROR.as_i32(), -32700);
    /// assert_eq!(ErrorCode::INVALID_REQUEST.as_i32(), -32600);
    /// assert_eq!(ErrorCode::METHOD_NOT_FOUND.as_i32(), -32601);
    /// assert_eq!(ErrorCode::INVALID_PARAMS.as_i32(), -32602);
    /// assert_eq!(ErrorCode::INTERNAL_ERROR.as_i32(), -32603);
    ///
    /// // MCP-specific error codes
    /// assert_eq!(ErrorCode::REQUEST_TIMEOUT.as_i32(), -32001);
    /// assert_eq!(ErrorCode::AUTHENTICATION_REQUIRED.as_i32(), -32003);
    ///
    /// // Custom error codes
    /// let custom = ErrorCode::other(-32099);
    /// assert_eq!(custom.as_i32(), -32099);
    ///
    /// // Use in JSON serialization
    /// # use serde_json::json;
    /// let error_json = json!({
    ///     "code": ErrorCode::INVALID_PARAMS.as_i32(),
    ///     "message": "Invalid parameters"
    /// });
    /// ```
    pub fn as_i32(&self) -> i32 {
        self.0
    }

    /// Create error code from i32 value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::error::ErrorCode;
    ///
    /// // Standard JSON-RPC errors are recognized
    /// assert_eq!(ErrorCode::from_i32(-32700), ErrorCode::PARSE_ERROR);
    /// assert_eq!(ErrorCode::from_i32(-32601), ErrorCode::METHOD_NOT_FOUND);
    ///
    /// // MCP-specific errors are recognized
    /// assert_eq!(ErrorCode::from_i32(-32001), ErrorCode::REQUEST_TIMEOUT);
    /// assert_eq!(ErrorCode::from_i32(-32003), ErrorCode::AUTHENTICATION_REQUIRED);
    ///
    /// // Server error range maps to INTERNAL_ERROR
    /// assert_eq!(ErrorCode::from_i32(-32050), ErrorCode::INTERNAL_ERROR);
    /// assert_eq!(ErrorCode::from_i32(-32099), ErrorCode::INTERNAL_ERROR);
    ///
    /// // Unknown codes create custom error
    /// let custom = ErrorCode::from_i32(-40000);
    /// assert_eq!(custom.as_i32(), -40000);
    ///
    /// // Use in error parsing
    /// # use serde_json::json;
    /// let error_data = json!({"code": -32602});
    /// let code = ErrorCode::from_i32(error_data["code"].as_i64().unwrap() as i32);
    /// assert_eq!(code, ErrorCode::INVALID_PARAMS);
    /// ```
    pub fn from_i32(code: i32) -> Self {
        // First check JSON-RPC standard errors
        if let Some(error_code) = Self::standard_jsonrpc_error(code) {
            return error_code;
        }

        // Then check MCP-specific errors
        if let Some(error_code) = Self::mcp_specific_error(code) {
            return error_code;
        }

        // Handle server error range
        if Self::is_server_error_range(code) {
            return Self::INTERNAL_ERROR;
        }

        // Default to other
        Self::other(code)
    }

    /// Map standard JSON-RPC error codes.
    fn standard_jsonrpc_error(code: i32) -> Option<Self> {
        match code {
            -32700 => Some(Self::PARSE_ERROR),
            -32600 => Some(Self::INVALID_REQUEST),
            -32601 => Some(Self::METHOD_NOT_FOUND),
            -32602 => Some(Self::INVALID_PARAMS),
            -32603 => Some(Self::INTERNAL_ERROR),
            _ => None,
        }
    }

    /// Map MCP-specific error codes.
    fn mcp_specific_error(code: i32) -> Option<Self> {
        match code {
            -32001 => Some(Self::REQUEST_TIMEOUT),
            -32002 => Some(Self::UNSUPPORTED_CAPABILITY),
            -32003 => Some(Self::AUTHENTICATION_REQUIRED),
            -32004 => Some(Self::PERMISSION_DENIED),
            _ => None,
        }
    }

    /// Check if code is in the server error range.
    fn is_server_error_range(code: i32) -> bool {
        matches!(code, -32099..=-32000)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Implement Hash for `ErrorCode` to use in `HashMap`
impl std::hash::Hash for ErrorCode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Transport-specific errors.
#[derive(Error, Debug)]
pub enum TransportError {
    /// IO error
    #[error("IO error: {0}")]
    Io(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Request error
    #[error("Request error: {0}")]
    Request(String),

    /// Send error
    #[error("Send error: {0}")]
    Send(String),

    /// WebSocket error (when feature enabled)
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// HTTP error (when feature enabled)
    #[cfg(feature = "http")]
    #[error("HTTP error: {0}")]
    Http(String),
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl Error {
    /// Create a protocol error with the given code and message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Error, error::ErrorCode};
    ///
    /// // Create a standard JSON-RPC error
    /// let err = Error::protocol(ErrorCode::INVALID_REQUEST, "Missing required field");
    /// assert!(err.is_error_code(ErrorCode::INVALID_REQUEST));
    ///
    /// // Create a custom error code
    /// let custom_err = Error::protocol(ErrorCode::other(-32050), "Custom server error");
    /// assert_eq!(custom_err.error_code().unwrap().as_i32(), -32050);
    ///
    /// // Use in error handling
    /// fn validate_request(data: &str) -> Result<(), Error> {
    ///     if data.is_empty() {
    ///         return Err(Error::protocol(
    ///             ErrorCode::INVALID_REQUEST,
    ///             "Request data cannot be empty"
    ///         ));
    ///     }
    ///     Ok(())
    /// }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Error, error::ErrorCode};
    /// use serde_json::json;
    ///
    /// // Create error with structured data
    /// let err = Error::protocol_with_data(
    ///     ErrorCode::INVALID_PARAMS,
    ///     "Invalid parameter type",
    ///     json!({
    ///         "expected": "string",
    ///         "actual": "number",
    ///         "field": "name"
    ///     })
    /// );
    ///
    /// // Create validation error with details
    /// let validation_err = Error::protocol_with_data(
    ///     ErrorCode::INVALID_PARAMS,
    ///     "Validation failed",
    ///     json!({
    ///         "errors": [
    ///             {"field": "age", "message": "Must be positive"},
    ///             {"field": "email", "message": "Invalid format"}
    ///         ]
    ///     })
    /// );
    ///
    /// // Access error details
    /// match &validation_err {
    ///     Error::Protocol { data: Some(details), .. } => {
    ///         println!("Error details: {}", details);
    ///     }
    ///     _ => {}
    /// }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Simple validation error
    /// let err = Error::validation("Email address is required");
    ///
    /// // Use in validation functions
    /// fn validate_email(email: &str) -> Result<(), Error> {
    ///     if email.is_empty() {
    ///         return Err(Error::validation("Email cannot be empty"));
    ///     }
    ///     if !email.contains('@') {
    ///         return Err(Error::validation("Invalid email format"));
    ///     }
    ///     Ok(())
    /// }
    ///
    /// // Validation with formatted messages
    /// fn validate_range(value: i32, min: i32, max: i32) -> Result<(), Error> {
    ///     if value < min || value > max {
    ///         return Err(Error::validation(
    ///             format!("Value {} must be between {} and {}", value, min, max)
    ///         ));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    /// Create an internal error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Simple internal error
    /// let err = Error::internal("Unexpected state");
    ///
    /// // Use for unexpected conditions
    /// fn process_data(data: &[u8]) -> Result<String, Error> {
    ///     match std::str::from_utf8(data) {
    ///         Ok(s) => Ok(s.to_string()),
    ///         Err(_) => Err(Error::internal("Failed to decode UTF-8 data")),
    ///     }
    /// }
    ///
    /// // Wrap system errors
    /// fn read_config() -> Result<String, Error> {
    ///     std::fs::read_to_string("config.json")
    ///         .map_err(|e| Error::internal(format!("Failed to read config: {}", e)))
    /// }
    /// ```
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Create a not found error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    /// use std::collections::HashMap;
    ///
    /// // Simple not found error
    /// let err = Error::not_found("user:123");
    ///
    /// // Use in lookup functions
    /// fn find_user(id: &str, users: &HashMap<String, String>) -> Result<String, Error> {
    ///     users.get(id)
    ///         .cloned()
    ///         .ok_or_else(|| Error::not_found(format!("User with ID '{}'", id)))
    /// }
    ///
    /// // Resource paths
    /// fn load_resource(path: &str) -> Result<Vec<u8>, Error> {
    ///     if !std::path::Path::new(path).exists() {
    ///         return Err(Error::not_found(format!("File '{}'", path)));
    ///     }
    ///     // Load file...
    ///     Ok(vec![])
    /// }
    /// ```
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound(resource.into())
    }

    /// Create a parse error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // JSON parsing error
    /// let err = Error::parse("Invalid JSON syntax at line 5");
    /// assert!(err.is_error_code(pmcp::error::ErrorCode::PARSE_ERROR));
    ///
    /// // Use in message parsing
    /// fn parse_message(json: &str) -> Result<serde_json::Value, Error> {
    ///     serde_json::from_str(json)
    ///         .map_err(|e| Error::parse(format!("Failed to parse JSON: {}", e)))
    /// }
    ///
    /// // Protocol frame parsing
    /// fn parse_frame(data: &[u8]) -> Result<String, Error> {
    ///     std::str::from_utf8(data)
    ///         .map(|s| s.to_string())
    ///         .map_err(|_| Error::parse("Invalid UTF-8 in message frame"))
    /// }
    /// ```
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::PARSE_ERROR,
            message: message.into(),
            data: None,
        }
    }

    /// Create an invalid request error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Missing required fields
    /// let err = Error::invalid_request("Missing required field: 'method'");
    /// assert!(err.is_error_code(pmcp::error::ErrorCode::INVALID_REQUEST));
    ///
    /// // Invalid request structure
    /// fn validate_jsonrpc_request(req: &serde_json::Value) -> Result<(), Error> {
    ///     if !req.is_object() {
    ///         return Err(Error::invalid_request("Request must be a JSON object"));
    ///     }
    ///     if req.get("jsonrpc") != Some(&serde_json::Value::String("2.0".to_string())) {
    ///         return Err(Error::invalid_request("Invalid jsonrpc version"));
    ///     }
    ///     Ok(())
    /// }
    ///
    /// // Protocol violations
    /// fn check_request_id(id: Option<&serde_json::Value>) -> Result<(), Error> {
    ///     match id {
    ///         Some(v) if v.is_string() || v.is_number() => Ok(()),
    ///         None => Ok(()), // notifications don't need ID
    ///         _ => Err(Error::invalid_request("Request ID must be string or number")),
    ///     }
    /// }
    /// ```
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_REQUEST,
            message: message.into(),
            data: None,
        }
    }

    /// Create a method not found error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Unknown method
    /// let err = Error::method_not_found("unknown/method");
    /// assert!(err.is_error_code(pmcp::error::ErrorCode::METHOD_NOT_FOUND));
    ///
    /// // Use in method dispatch
    /// fn dispatch_method(method: &str) -> Result<String, Error> {
    ///     match method {
    ///         "initialize" => Ok("initializing...".to_string()),
    ///         "ping" => Ok("pong".to_string()),
    ///         "tools/list" => Ok("[]".to_string()),
    ///         unknown => Err(Error::method_not_found(unknown)),
    ///     }
    /// }
    ///
    /// // Check supported methods
    /// fn validate_method(method: &str, supported: &[&str]) -> Result<(), Error> {
    ///     if supported.contains(&method) {
    ///         Ok(())
    ///     } else {
    ///         Err(Error::method_not_found(format!("'{}' (supported: {:?})", method, supported)))
    ///     }
    /// }
    /// ```
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    /// Create an invalid params error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Missing required parameter
    /// let err = Error::invalid_params("Missing required parameter: 'name'");
    /// assert!(err.is_error_code(pmcp::error::ErrorCode::INVALID_PARAMS));
    ///
    /// // Parameter validation
    /// fn validate_tool_call(name: Option<&str>, args: &serde_json::Value) -> Result<(), Error> {
    ///     let name = name.ok_or_else(|| Error::invalid_params("Tool name is required"))?;
    ///     
    ///     if name.is_empty() {
    ///         return Err(Error::invalid_params("Tool name cannot be empty"));
    ///     }
    ///     
    ///     if !args.is_object() {
    ///         return Err(Error::invalid_params("Tool arguments must be an object"));
    ///     }
    ///     
    ///     Ok(())
    /// }
    ///
    /// // Type validation
    /// fn validate_range(params: &serde_json::Value) -> Result<(i32, i32), Error> {
    ///     let min = params.get("min")
    ///         .and_then(|v| v.as_i64())
    ///         .ok_or_else(|| Error::invalid_params("'min' must be a number"))?;
    ///     let max = params.get("max")
    ///         .and_then(|v| v.as_i64())
    ///         .ok_or_else(|| Error::invalid_params("'max' must be a number"))?;
    ///         
    ///     if min >= max {
    ///         return Err(Error::invalid_params("'min' must be less than 'max'"));
    ///     }
    ///     
    ///     Ok((min as i32, max as i32))
    /// }
    /// ```
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_PARAMS,
            message: message.into(),
            data: None,
        }
    }

    /// Create an authentication error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Basic authentication error
    /// let err = Error::authentication("Invalid API key");
    ///
    /// // Use in auth validation
    /// fn validate_token(token: &str) -> Result<(), Error> {
    ///     if token.is_empty() {
    ///         return Err(Error::authentication("Token is required"));
    ///     }
    ///     if token.len() < 32 {
    ///         return Err(Error::authentication("Token must be at least 32 characters"));
    ///     }
    ///     if !token.starts_with("Bearer ") {
    ///         return Err(Error::authentication("Token must use Bearer scheme"));
    ///     }
    ///     Ok(())
    /// }
    ///
    /// // OAuth errors
    /// fn handle_oauth_callback(code: Option<&str>) -> Result<String, Error> {
    ///     match code {
    ///         Some(c) if !c.is_empty() => Ok(c.to_string()),
    ///         _ => Err(Error::authentication("OAuth authorization code missing")),
    ///     }
    /// }
    /// ```
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create a capability error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // Unsupported capability
    /// let err = Error::capability("Server does not support tools capability");
    ///
    /// // Feature check
    /// fn check_sampling_support(server_caps: &pmcp::types::ServerCapabilities) -> Result<(), Error> {
    ///     if server_caps.sampling.is_none() {
    ///         return Err(Error::capability("Sampling not supported by server"));
    ///     }
    ///     Ok(())
    /// }
    ///
    /// // Protocol version mismatch
    /// fn validate_protocol_version(version: &str) -> Result<(), Error> {
    ///     match version {
    ///         "2025-06-18" => Ok(()),
    ///         other => Err(Error::capability(format!("Protocol version '{}' not supported", other))),
    ///     }
    /// }
    ///
    /// // Resource capability check
    /// fn require_resources(has_resources: bool) -> Result<(), Error> {
    ///     if !has_resources {
    ///         Err(Error::capability("This operation requires resources capability"))
    ///     } else {
    ///         Ok(())
    ///     }
    /// }
    /// ```
    pub fn capability(message: impl Into<String>) -> Self {
        Self::UnsupportedCapability(message.into())
    }

    /// Create a resource not found error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // File resource not found
    /// let err = Error::resource_not_found("file:///path/to/missing.txt");
    ///
    /// // Use in resource handlers
    /// fn read_file_resource(uri: &str) -> Result<Vec<u8>, Error> {
    ///     let path = uri.strip_prefix("file://").unwrap_or(uri);
    ///     std::fs::read(path)
    ///         .map_err(|_| Error::resource_not_found(uri))
    /// }
    ///
    /// // Database resource
    /// fn get_database_record(uri: &str) -> Result<String, Error> {
    ///     // Extract ID from URI like "db://users/123"
    ///     if let Some(id) = uri.strip_prefix("db://users/") {
    ///         if id == "999" {
    ///             return Err(Error::resource_not_found(uri));
    ///         }
    ///         Ok(format!("User {}", id))
    ///     } else {
    ///         Err(Error::resource_not_found(uri))
    ///     }
    /// }
    ///
    /// // API resource
    /// fn fetch_api_resource(uri: &str) -> Result<serde_json::Value, Error> {
    ///     if !uri.starts_with("https://api.example.com/") {
    ///         return Err(Error::resource_not_found(format!("Invalid API URI: {}", uri)));
    ///     }
    ///     // Mock response
    ///     Ok(serde_json::json!({"data": "found"}))
    /// }
    /// ```
    pub fn resource_not_found(uri: impl Into<String>) -> Self {
        Self::NotFound(format!("Resource not found: {}", uri.into()))
    }

    /// Create a cancelled operation error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::Error;
    ///
    /// // User-initiated cancellation
    /// let err = Error::cancelled("User cancelled the operation");
    ///
    /// // Timeout cancellation
    /// fn check_cancellation_token(cancelled: bool) -> Result<String, Error> {
    ///     if cancelled {
    ///         return Err(Error::cancelled("Operation timed out"));
    ///     }
    ///     Ok("completed".to_string())
    ///     }
    ///
    /// // Tool execution cancellation
    /// fn run_long_tool(should_cancel: bool) -> Result<serde_json::Value, Error> {
    ///     if should_cancel {
    ///         return Err(Error::cancelled("Tool execution was cancelled by client"));
    ///     }
    ///     Ok(serde_json::json!({"result": "success"}))
    /// }
    ///
    /// // Progress notification cancellation
    /// fn send_progress_update(cancelled: bool, progress: f64) -> Result<(), Error> {
    ///     if cancelled {
    ///         return Err(Error::cancelled(format!("Progress cancelled at {}%", progress * 100.0)));
    ///     }
    ///     println!("Progress: {}%", progress * 100.0);
    ///     Ok(())
    /// }
    /// ```
    pub fn cancelled(message: impl Into<String>) -> Self {
        // For now, treat cancellation as a validation error with specific message
        Self::Validation(format!("Operation cancelled: {}", message.into()))
    }

    /// Create from JSON-RPC error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Error, types::JSONRPCError};
    /// use serde_json::json;
    ///
    /// // Convert from JSON-RPC error
    /// let jsonrpc_error = JSONRPCError {
    ///     code: -32602,
    ///     message: "Invalid parameters".to_string(),
    ///     data: Some(json!({"field": "name", "reason": "too short"})),
    /// };
    ///
    /// let err = Error::from_jsonrpc_error(jsonrpc_error);
    /// assert!(err.is_error_code(pmcp::error::ErrorCode::INVALID_PARAMS));
    ///
    /// // Parse from JSON response
    /// let error_response = json!({
    ///     "jsonrpc": "2.0",
    ///     "error": {
    ///         "code": -32601,
    ///         "message": "Method not found: foo"
    ///     },
    ///     "id": 1
    /// });
    ///
    /// if let Ok(jsonrpc_err) = serde_json::from_value::<JSONRPCError>(error_response["error"].clone()) {
    ///     let err = Error::from_jsonrpc_error(jsonrpc_err);
    ///     assert!(err.is_error_code(pmcp::error::ErrorCode::METHOD_NOT_FOUND));
    /// }
    /// ```
    pub fn from_jsonrpc_error(error: crate::types::JSONRPCError) -> Self {
        Self::Protocol {
            code: ErrorCode::from_i32(error.code),
            message: error.message,
            data: error.data,
        }
    }

    /// Check if this is a protocol error with a specific code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Error, error::ErrorCode};
    ///
    /// // Check specific error codes
    /// let err = Error::protocol(ErrorCode::INVALID_REQUEST, "Bad request");
    /// assert!(err.is_error_code(ErrorCode::INVALID_REQUEST));
    /// assert!(!err.is_error_code(ErrorCode::METHOD_NOT_FOUND));
    ///
    /// // Non-protocol errors return false
    /// let validation_err = Error::validation("Invalid field");
    /// assert!(!validation_err.is_error_code(ErrorCode::INVALID_REQUEST));
    ///
    /// // Use in error handling
    /// fn handle_error(err: &Error) {
    ///     if err.is_error_code(ErrorCode::AUTHENTICATION_REQUIRED) {
    ///         println!("Please authenticate first");
    ///     } else if err.is_error_code(ErrorCode::METHOD_NOT_FOUND) {
    ///         println!("Unknown method called");
    ///     } else {
    ///         println!("Error: {}", err);
    ///     }
    /// }
    /// ```
    pub fn is_error_code(&self, code: ErrorCode) -> bool {
        matches!(self, Self::Protocol { code: c, .. } if *c == code)
    }

    /// Get the error code if this is a protocol error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Error, error::ErrorCode};
    ///
    /// // Protocol errors have error codes
    /// let protocol_err = Error::protocol(ErrorCode::INVALID_PARAMS, "Missing field");
    /// assert_eq!(protocol_err.error_code(), Some(ErrorCode::INVALID_PARAMS));
    ///
    /// // Other error types return None
    /// let internal_err = Error::internal("System failure");
    /// assert_eq!(internal_err.error_code(), None);
    ///
    /// let auth_err = Error::authentication("Invalid token");
    /// assert_eq!(auth_err.error_code(), None);
    ///
    /// // Use for error categorization
    /// fn categorize_error(err: &Error) -> &'static str {
    ///     match err.error_code() {
    ///         Some(code) => match code.as_i32() {
    ///             -32700 => "Parse error",
    ///             -32600 => "Invalid request",
    ///             -32601 => "Method not found",
    ///             -32602 => "Invalid parameters",
    ///             -32603 => "Internal error",
    ///             _ => "Protocol error",
    ///         },
    ///         None => "Application error",
    ///     }
    /// }
    /// ```
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
