//! Core protocol implementation shared between client and server.

use crate::error::{Error, Result};
use crate::types::RequestId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};

/// Progress callback function type.
pub type ProgressCallback = Arc<dyn Fn(f64, Option<String>) + Send + Sync>;

/// Protocol options for initialization.
#[derive(Debug, Clone)]
pub struct ProtocolOptions {
    /// Whether to enforce strict capability checking
    pub enforce_strict_capabilities: bool,
    /// Methods to debounce notifications for
    pub debounced_notification_methods: Vec<String>,
    /// Default request timeout
    pub default_timeout: Duration,
}

impl Default for ProtocolOptions {
    fn default() -> Self {
        Self {
            enforce_strict_capabilities: false,
            debounced_notification_methods: Vec::new(),
            default_timeout: Duration::from_millis(crate::DEFAULT_REQUEST_TIMEOUT_MS),
        }
    }
}

/// Request options for individual requests.
#[derive(Clone, Default)]
pub struct RequestOptions {
    /// Progress callback
    pub on_progress: Option<ProgressCallback>,
    /// Cancellation signal
    pub signal: Option<tokio::sync::watch::Receiver<bool>>,
    /// Request timeout
    pub timeout: Option<Duration>,
    /// Reset timeout on progress
    pub reset_timeout_on_progress: bool,
    /// Maximum total timeout
    pub max_total_timeout: Option<Duration>,
}

impl RequestOptions {
    /// Create new request options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set progress callback.
    pub fn with_progress(
        mut self,
        callback: impl Fn(f64, Option<String>) + Send + Sync + 'static,
    ) -> Self {
        self.on_progress = Some(Arc::new(callback));
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set cancellation signal.
    pub fn with_signal(mut self, signal: tokio::sync::watch::Receiver<bool>) -> Self {
        self.signal = Some(signal);
        self
    }
}

impl std::fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestOptions")
            .field("on_progress", &self.on_progress.is_some())
            .field("signal", &self.signal.is_some())
            .field("timeout", &self.timeout)
            .field("reset_timeout_on_progress", &self.reset_timeout_on_progress)
            .field("max_total_timeout", &self.max_total_timeout)
            .finish()
    }
}

/// Protocol state management.
#[allow(dead_code)]
pub struct Protocol {
    options: ProtocolOptions,
    pending_requests: Arc<RwLock<HashMap<RequestId, PendingRequest>>>,
    request_counter: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Protocol")
            .field("options", &self.options)
            .field("pending_requests_count", &"<Arc<RwLock<HashMap>>>")
            .field(
                "request_counter",
                &self
                    .request_counter
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

#[allow(dead_code)]
struct PendingRequest {
    response_tx: oneshot::Sender<Result<serde_json::Value>>,
    progress_callback: Option<ProgressCallback>,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl Protocol {
    /// Create a new protocol instance.
    pub fn new(options: ProtocolOptions) -> Self {
        Self {
            options,
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            request_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Generate a unique request ID.
    pub fn next_request_id(&self) -> RequestId {
        let id = self
            .request_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        RequestId::from(id)
    }

    /// Register a pending request.
    pub async fn register_request(
        &self,
        id: RequestId,
        options: &RequestOptions,
    ) -> oneshot::Receiver<Result<serde_json::Value>> {
        let (tx, rx) = oneshot::channel();
        let (cancel_tx, _cancel_rx) = oneshot::channel();

        let pending = PendingRequest {
            response_tx: tx,
            progress_callback: options.on_progress.clone(),
            cancel_tx: Some(cancel_tx),
        };

        self.pending_requests
            .write()
            .await
            .insert(id.clone(), pending);

        // Set up cancellation if signal provided
        if let Some(mut signal) = options.signal.clone() {
            let pending_requests = self.pending_requests.clone();
            let request_id = id.clone();

            tokio::spawn(async move {
                if signal.changed().await.is_ok() && *signal.borrow() {
                    // Request was cancelled
                    let mut lock = pending_requests.write().await;
                    if let Some(pending) = lock.remove(&request_id) {
                        let _ = pending.response_tx.send(Err(Error::Cancelled));
                    }
                }
            });
        }

        rx
    }

    /// Handle a response for a pending request.
    pub async fn handle_response(&self, id: RequestId, result: Result<serde_json::Value>) {
        let mut lock = self.pending_requests.write().await;
        if let Some(pending) = lock.remove(&id) {
            let _ = pending.response_tx.send(result);
        }
    }

    /// Handle a progress notification.
    pub async fn handle_progress(&self, id: RequestId, progress: f64, message: Option<String>) {
        if let Some(pending) = self.pending_requests.read().await.get(&id) {
            if let Some(callback) = &pending.progress_callback {
                callback(progress, message);
            }
        }
    }

    /// Clean up a completed request.
    pub async fn cleanup_request(&self, id: &RequestId) {
        self.pending_requests.write().await.remove(id);
    }

    /// Get the number of pending requests.
    pub async fn pending_count(&self) -> usize {
        self.pending_requests.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn protocol_request_ids() {
        let protocol = Protocol::new(ProtocolOptions::default());

        let id1 = protocol.next_request_id();
        let id2 = protocol.next_request_id();

        assert_ne!(id1, id2);
        assert_eq!(id1, RequestId::from(0u64));
        assert_eq!(id2, RequestId::from(1u64));
    }

    #[tokio::test]
    async fn protocol_request_registration() {
        let protocol = Protocol::new(ProtocolOptions::default());
        let id = protocol.next_request_id();

        let rx = protocol
            .register_request(id.clone(), &RequestOptions::default())
            .await;
        assert_eq!(protocol.pending_count().await, 1);

        protocol
            .handle_response(id.clone(), Ok(serde_json::json!({"result": true})))
            .await;
        assert_eq!(protocol.pending_count().await, 0);

        let result = rx.await.unwrap().unwrap();
        assert_eq!(result["result"], true);
    }

    #[tokio::test]
    async fn protocol_request_cancellation() {
        let protocol = Protocol::new(ProtocolOptions::default());
        let id = protocol.next_request_id();

        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        let options = RequestOptions::default().with_signal(cancel_rx);

        let rx = protocol.register_request(id.clone(), &options).await;

        // Cancel the request
        cancel_tx.send(true).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Should receive cancellation error
        match rx.await {
            Ok(Err(Error::Cancelled)) => {},
            other => panic!("Expected cancellation error, got: {:?}", other),
        }
    }
}
