//! Protocol implementation for MCP.
//!
//! This module provides the core protocol state machine and request handling.

use crate::error::Result;
use crate::types::{JSONRPCResponse, RequestId};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::oneshot;

/// Progress callback type.
pub type ProgressCallback = Box<dyn Fn(u64, Option<u64>) + Send + Sync>;

/// Protocol options for configuring behavior.
#[derive(Debug, Clone, Default)]
pub struct ProtocolOptions {
    /// Whether to enforce strict capability checking.
    pub enforce_strict_capabilities: bool,
    /// Methods that should be debounced.
    pub debounced_notification_methods: Vec<String>,
}

/// Request options for individual requests.
#[derive(Default)]
pub struct RequestOptions {
    /// Timeout for the request.
    pub timeout: Option<Duration>,
    /// Progress callback.
    pub on_progress: Option<ProgressCallback>,
}

impl std::fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestOptions")
            .field("timeout", &self.timeout)
            .field(
                "on_progress",
                &self.on_progress.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

/// Protocol state machine for handling JSON-RPC communication.
#[derive(Debug)]
pub struct Protocol {
    /// Protocol options.
    options: ProtocolOptions,
    /// Pending requests waiting for responses.
    pending_requests: HashMap<RequestId, oneshot::Sender<JSONRPCResponse>>,
}

impl Protocol {
    /// Create a new protocol instance.
    pub fn new(options: ProtocolOptions) -> Self {
        Self {
            options,
            pending_requests: HashMap::new(),
        }
    }

    /// Get protocol options.
    pub fn options(&self) -> &ProtocolOptions {
        &self.options
    }

    /// Register a pending request.
    pub fn register_request(&mut self, id: RequestId) -> oneshot::Receiver<JSONRPCResponse> {
        let (tx, rx) = oneshot::channel();
        self.pending_requests.insert(id, tx);
        rx
    }

    /// Complete a pending request.
    pub fn complete_request(&mut self, id: &RequestId, response: JSONRPCResponse) -> Result<()> {
        if let Some(tx) = self.pending_requests.remove(id) {
            let _ = tx.send(response);
        }
        Ok(())
    }

    /// Cancel a pending request.
    pub fn cancel_request(&mut self, id: &RequestId) {
        self.pending_requests.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_options() {
        let options = ProtocolOptions {
            enforce_strict_capabilities: true,
            debounced_notification_methods: vec!["test".to_string()],
        };
        assert!(options.enforce_strict_capabilities);
        assert_eq!(options.debounced_notification_methods, vec!["test"]);

        let default_options = ProtocolOptions::default();
        assert!(!default_options.enforce_strict_capabilities);
        assert!(default_options.debounced_notification_methods.is_empty());
    }

    #[test]
    fn test_request_options() {
        let options = RequestOptions {
            timeout: Some(Duration::from_secs(30)),
            on_progress: None,
        };
        assert_eq!(options.timeout, Some(Duration::from_secs(30)));
        assert!(options.on_progress.is_none());

        // Test debug formatting
        let debug_str = format!("{:?}", options);
        assert!(debug_str.contains("timeout: Some"));
    }

    #[test]
    fn test_protocol_creation() {
        let options = ProtocolOptions::default();
        let protocol = Protocol::new(options);
        assert!(!protocol.options().enforce_strict_capabilities);
        assert_eq!(protocol.pending_requests.len(), 0);
    }

    #[tokio::test]
    async fn test_register_and_complete_request() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Register a request
        let id = RequestId::Number(42);
        let mut rx = protocol.register_request(id.clone());
        assert_eq!(protocol.pending_requests.len(), 1);

        // Complete the request
        let response = JSONRPCResponse::success(id.clone(), serde_json::json!("success"));
        protocol.complete_request(&id, response).unwrap();
        assert_eq!(protocol.pending_requests.len(), 0);

        // Verify the receiver got the response
        let received = rx.try_recv().unwrap();
        assert_eq!(received.result(), Some(&serde_json::json!("success")));
    }

    #[test]
    fn test_cancel_request() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Register multiple requests
        let id1 = RequestId::Number(1);
        let id2 = RequestId::String("req-2".to_string());
        let _rx1 = protocol.register_request(id1.clone());
        let _rx2 = protocol.register_request(id2.clone());
        assert_eq!(protocol.pending_requests.len(), 2);

        // Cancel one request
        protocol.cancel_request(&id1);
        assert_eq!(protocol.pending_requests.len(), 1);
        assert!(!protocol.pending_requests.contains_key(&id1));
        assert!(protocol.pending_requests.contains_key(&id2));

        // Cancel non-existent request (should not panic)
        protocol.cancel_request(&RequestId::Number(999));
        assert_eq!(protocol.pending_requests.len(), 1);
    }

    #[tokio::test]
    async fn test_complete_non_existent_request() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Try to complete a request that was never registered
        let id = RequestId::String("non-existent".to_string());
        let response = JSONRPCResponse::success(id.clone(), serde_json::json!("test"));

        // Should not panic
        let result = protocol.complete_request(&id, response);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_pending_requests() {
        let mut protocol = Protocol::new(ProtocolOptions::default());

        // Register multiple requests
        let ids: Vec<_> = (0..5).map(RequestId::Number).collect();
        let _receivers: Vec<_> = ids
            .iter()
            .map(|id| protocol.register_request(id.clone()))
            .collect();
        assert_eq!(protocol.pending_requests.len(), 5);

        // Complete them in reverse order
        for (i, id) in ids.iter().enumerate().rev() {
            let response = JSONRPCResponse::success(id.clone(), serde_json::json!(i));
            protocol.complete_request(id, response).unwrap();
        }

        assert_eq!(protocol.pending_requests.len(), 0);
    }

    #[test]
    fn test_request_options_with_progress() {
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();

        let options = RequestOptions {
            timeout: Some(Duration::from_millis(100)),
            on_progress: Some(Box::new(move |current, total| {
                called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(current, 50);
                assert_eq!(total, Some(100));
            })),
        };

        // Call the progress callback
        if let Some(cb) = &options.on_progress {
            cb(50, Some(100));
        }

        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_protocol_with_enforced_capabilities() {
        let options = ProtocolOptions {
            enforce_strict_capabilities: true,
            debounced_notification_methods: vec![
                "notifications/progress".to_string(),
                "notifications/cancelled".to_string(),
            ],
        };

        let protocol = Protocol::new(options);
        assert!(protocol.options().enforce_strict_capabilities);
        assert_eq!(protocol.options().debounced_notification_methods.len(), 2);
    }
}
