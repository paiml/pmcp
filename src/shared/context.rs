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
#[macro_export]
macro_rules! with_context {
    ($ctx:expr, $body:expr) => {
        $ctx.run(async move { $body }).await
    };
}

/// Macro to get current context or create new one.
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
