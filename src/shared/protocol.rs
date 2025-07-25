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
