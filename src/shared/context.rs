//! Request context propagation for distributed tracing and metadata.
//!
//! This module provides context propagation across async boundaries and between
//! client and server components.

use crate::types::RequestId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task_local;

// Task-local storage for request context
task_local! {
    static REQUEST_CONTEXT: Arc<RequestContext>;
}

/// Request context containing metadata and tracing information.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::context::{RequestContext, ClientInfo};
/// use pmcp::types::RequestId;
/// use serde_json::json;
///
/// // Create a new context for a request
/// let context = RequestContext::new(RequestId::from(123i64))
///     .with_user_id("user-456".to_string())
///     .with_session_id("session-789".to_string())
///     .with_metadata("environment".to_string(), json!("production"))
///     .with_baggage("feature-flag".to_string(), "new-ui-enabled".to_string());
///
/// // Add client information
/// let client_info = ClientInfo {
///     client_id: "web-app-v2".to_string(),
///     version: Some("2.1.0".to_string()),
///     ip_address: Some("192.168.1.100".to_string()),
///     user_agent: Some("Mozilla/5.0...".to_string()),
/// };
/// let context = context.with_client_info(client_info);
///
/// // Create child context for nested operations
/// let child_context = context.child();
/// assert_eq!(child_context.trace_id, context.trace_id);
/// assert_eq!(child_context.parent_span_id, Some(context.span_id.clone()));
///
/// // Convert to HTTP headers for propagation
/// let headers = context.to_headers();
/// assert!(headers.contains_key("traceparent"));
/// assert!(headers.contains_key("x-request-id"));
/// assert_eq!(headers.get("x-user-id"), Some(&"user-456".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    /// Unique request ID.
    pub request_id: RequestId,

    /// Trace ID for distributed tracing.
    pub trace_id: String,

    /// Parent span ID.
    pub parent_span_id: Option<String>,

    /// Current span ID.
    pub span_id: String,

    /// Request timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// User ID if authenticated.
    pub user_id: Option<String>,

    /// Session ID.
    pub session_id: Option<String>,

    /// Client information.
    pub client_info: Option<ClientInfo>,

    /// Custom metadata.
    pub metadata: HashMap<String, serde_json::Value>,

    /// Baggage items for propagation.
    pub baggage: HashMap<String, String>,
}

/// Client information in context.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::context::ClientInfo;
///
/// // Create client info for a web application
/// let web_client = ClientInfo {
///     client_id: "web-dashboard".to_string(),
///     version: Some("3.2.1".to_string()),
///     ip_address: Some("10.0.0.50".to_string()),
///     user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string()),
/// };
///
/// // Create client info for a mobile app
/// let mobile_client = ClientInfo {
///     client_id: "ios-app".to_string(),
///     version: Some("1.5.0".to_string()),
///     ip_address: Some("172.16.0.100".to_string()),
///     user_agent: Some("MyApp/1.5.0 (iOS 15.0)".to_string()),
/// };
///
/// // Create minimal client info
/// let minimal_client = ClientInfo {
///     client_id: "cli-tool".to_string(),
///     version: None,
///     ip_address: None,
///     user_agent: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client ID.
    pub client_id: String,

    /// Client version.
    pub version: Option<String>,

    /// Client IP address.
    pub ip_address: Option<String>,

    /// User agent.
    pub user_agent: Option<String>,
}

impl RequestContext {
    /// Create a new request context.
    pub fn new(request_id: RequestId) -> Self {
        Self {
            request_id,
            trace_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: None,
            span_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            user_id: None,
            session_id: None,
            client_info: None,
            metadata: HashMap::new(),
            baggage: HashMap::new(),
        }
    }

    /// Create a child context for nested operations.
    pub fn child(&self) -> Self {
        Self {
            request_id: self.request_id.clone(),
            trace_id: self.trace_id.clone(),
            parent_span_id: Some(self.span_id.clone()),
            span_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            user_id: self.user_id.clone(),
            session_id: self.session_id.clone(),
            client_info: self.client_info.clone(),
            metadata: self.metadata.clone(),
            baggage: self.baggage.clone(),
        }
    }

    /// Add metadata to the context.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Add baggage item for propagation.
    pub fn with_baggage(mut self, key: String, value: String) -> Self {
        self.baggage.insert(key, value);
        self
    }

    /// Set user ID.
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set session ID.
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set client info.
    pub fn with_client_info(mut self, client_info: ClientInfo) -> Self {
        self.client_info = Some(client_info);
        self
    }

    /// Get current context from task-local storage.
    pub fn current() -> Option<Arc<Self>> {
        REQUEST_CONTEXT.try_with(|ctx| ctx.clone()).ok()
    }

    /// Run a future with this context.
    pub async fn run<F, R>(self, f: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        REQUEST_CONTEXT.scope(Arc::new(self), f).await
    }

    /// Convert to HTTP headers for propagation.
    pub fn to_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Standard trace context headers
        // Trace ID should be 32 hex chars, Span ID should be 16 hex chars
        let trace_id_hex = self.trace_id.replace('-', "");
        let span_id_hex = self.span_id.replace('-', "")[..16].to_string();
        headers.insert(
            "traceparent".to_string(),
            format!("00-{}-{}-01", trace_id_hex, span_id_hex),
        );

        // Custom headers
        headers.insert("x-request-id".to_string(), self.request_id.to_string());

        if let Some(user_id) = &self.user_id {
            headers.insert("x-user-id".to_string(), user_id.clone());
        }

        if let Some(session_id) = &self.session_id {
            headers.insert("x-session-id".to_string(), session_id.clone());
        }

        // Baggage items
        for (key, value) in &self.baggage {
            headers.insert(format!("baggage-{}", key), value.clone());
        }

        headers
    }

    /// Create from HTTP headers.
    pub fn from_headers(headers: &HashMap<String, String>) -> Option<Self> {
        // Parse traceparent header
        if let Some(traceparent) = headers.get("traceparent") {
            let parts: Vec<&str> = traceparent.split('-').collect();
            if parts.len() >= 4 {
                // Restore UUIDs from hex format
                let trace_id_hex = parts[1];
                let trace_id = if trace_id_hex.len() == 32 {
                    format!(
                        "{}-{}-{}-{}-{}",
                        &trace_id_hex[0..8],
                        &trace_id_hex[8..12],
                        &trace_id_hex[12..16],
                        &trace_id_hex[16..20],
                        &trace_id_hex[20..32]
                    )
                } else {
                    trace_id_hex.to_string()
                };

                let span_id_hex = parts[2];
                let span_id = if span_id_hex.len() == 16 {
                    format!(
                        "{}-{}-{}-{}-{}",
                        &span_id_hex[0..8],
                        &span_id_hex[8..12],
                        &span_id_hex[12..16],
                        "0000",
                        "000000000000"
                    )
                } else {
                    span_id_hex.to_string()
                };

                let request_id = headers
                    .get("x-request-id")
                    .and_then(|id| id.parse::<i64>().ok())
                    .map_or_else(
                        || RequestId::from(uuid::Uuid::new_v4().as_u128() as i64),
                        RequestId::from,
                    );

                let mut context = Self::new(request_id);
                context.trace_id = trace_id;
                context.parent_span_id = Some(span_id);
                context.span_id = uuid::Uuid::new_v4().to_string();

                // Extract other headers
                if let Some(user_id) = headers.get("x-user-id") {
                    context.user_id = Some(user_id.clone());
                }

                if let Some(session_id) = headers.get("x-session-id") {
                    context.session_id = Some(session_id.clone());
                }

                // Extract baggage
                for (key, value) in headers {
                    if key.starts_with("baggage-") {
                        let baggage_key = key.strip_prefix("baggage-").unwrap();
                        context
                            .baggage
                            .insert(baggage_key.to_string(), value.clone());
                    }
                }

                return Some(context);
            }
        }

        None
    }
}

/// Context propagator for middleware integration.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::context::{ContextPropagator, RequestContext};
/// use pmcp::types::RequestId;
/// use std::collections::HashMap;
///
/// // Extract context from incoming HTTP headers
/// let mut headers = HashMap::new();
/// headers.insert("traceparent".to_string(),
///     "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string());
/// headers.insert("x-request-id".to_string(), "12345".to_string());
/// headers.insert("x-user-id".to_string(), "user-789".to_string());
/// headers.insert("baggage-tenant".to_string(), "acme-corp".to_string());
///
/// let context = ContextPropagator::extract(&headers);
/// assert!(context.is_some());
///
/// if let Some(ctx) = context {
///     assert_eq!(ctx.user_id, Some("user-789".to_string()));
///     assert_eq!(ctx.baggage.get("tenant"), Some(&"acme-corp".to_string()));
/// }
///
/// // Inject context into outgoing HTTP headers
/// let context = RequestContext::new(RequestId::from(999i64))
///     .with_user_id("admin".to_string());
/// let outgoing_headers = ContextPropagator::inject(&context);
/// assert!(outgoing_headers.contains_key("traceparent"));
/// assert_eq!(outgoing_headers.get("x-user-id"), Some(&"admin".to_string()));
/// ```
#[derive(Debug)]
pub struct ContextPropagator;

impl ContextPropagator {
    /// Extract context from incoming request.
    pub fn extract(headers: &HashMap<String, String>) -> Option<RequestContext> {
        RequestContext::from_headers(headers)
    }

    /// Inject context into outgoing request.
    pub fn inject(context: &RequestContext) -> HashMap<String, String> {
        context.to_headers()
    }
}

/// Macro to run code with request context.
///
/// # Examples
///
/// ```rust
/// use pmcp::{with_context, shared::context::RequestContext};
/// use pmcp::types::RequestId;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a context
/// let context = RequestContext::new(RequestId::from(123i64))
///     .with_user_id("user-456".to_string())
///     .with_metadata("region".to_string(), serde_json::json!("us-east-1"));
///
/// // Run async code with the context
/// let result = with_context!(context, {
///     // This code runs with the context available
///     let current = RequestContext::current().expect("Context should be set");
///     assert_eq!(current.user_id, Some("user-456".to_string()));
///     
///     // Perform some async operation
///     async {
///         // The context is available in nested async blocks too
///         let ctx = RequestContext::current().unwrap();
///         assert_eq!(ctx.request_id, RequestId::from(123i64));
///         42
///     }.await
/// });
///
/// assert_eq!(result, 42);
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! with_context {
    ($ctx:expr, $body:expr) => {
        $ctx.run(async move { $body }).await
    };
}

/// Macro to get current context or create new one.
///
/// # Examples
///
/// ```rust
/// use pmcp::{context_or_new, with_context, shared::context::RequestContext};
/// use pmcp::types::RequestId;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // When no context is set, creates a new one
/// let context = context_or_new!(RequestId::from(999i64));
/// assert_eq!(context.request_id, RequestId::from(999i64));
///
/// // When context is set, uses the existing one
/// let parent_context = RequestContext::new(RequestId::from(123i64))
///     .with_user_id("admin".to_string());
///
/// let result = with_context!(parent_context, {
///     // This uses the existing context
///     let ctx = context_or_new!(RequestId::from(999i64));
///     assert_eq!(ctx.request_id, RequestId::from(123i64)); // Uses parent context ID
///     assert_eq!(ctx.user_id, Some("admin".to_string()));
///     ctx
/// });
///
/// assert_eq!(result.request_id, RequestId::from(123i64));
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! context_or_new {
    ($request_id:expr) => {
        RequestContext::current()
            .map(|ctx| (*ctx).clone())
            .unwrap_or_else(|| RequestContext::new($request_id))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_propagation() {
        let context = RequestContext::new(RequestId::from(123i64))
            .with_user_id("user123".to_string())
            .with_baggage("key1".to_string(), "value1".to_string());

        let result = context
            .clone()
            .run(async {
                let current = RequestContext::current().unwrap();
                assert_eq!(current.user_id, Some("user123".to_string()));
                assert_eq!(current.baggage.get("key1"), Some(&"value1".to_string()));
                42
            })
            .await;

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_child_context() {
        let parent = RequestContext::new(RequestId::from(123i64));
        let child = parent.child();

        assert_eq!(parent.request_id, child.request_id);
        assert_eq!(parent.trace_id, child.trace_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
        assert_ne!(parent.span_id, child.span_id);
    }

    #[tokio::test]
    async fn test_headers_conversion() {
        let context = RequestContext::new(RequestId::from(123i64))
            .with_user_id("user123".to_string())
            .with_session_id("session456".to_string())
            .with_baggage("env".to_string(), "prod".to_string());

        let headers = context.to_headers();

        assert!(headers.contains_key("traceparent"));
        assert_eq!(headers.get("x-request-id"), Some(&"123".to_string()));
        assert_eq!(headers.get("x-user-id"), Some(&"user123".to_string()));
        assert_eq!(headers.get("x-session-id"), Some(&"session456".to_string()));
        assert_eq!(headers.get("baggage-env"), Some(&"prod".to_string()));

        // Test round-trip
        let restored = RequestContext::from_headers(&headers).unwrap();
        assert_eq!(restored.trace_id, context.trace_id);
        assert_eq!(restored.user_id, context.user_id);
        assert_eq!(restored.session_id, context.session_id);
        assert_eq!(restored.baggage.get("env"), Some(&"prod".to_string()));
    }
}
