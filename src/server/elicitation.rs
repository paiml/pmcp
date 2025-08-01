//! User input elicitation support for MCP servers.

use crate::error::{Error, ErrorCode, Result};
use crate::types::elicitation::{ElicitInputRequest, ElicitInputResponse};
use crate::types::protocol::ServerRequest;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

/// Manager for handling input elicitation requests.
pub struct ElicitationManager {
    /// Pending elicitation requests waiting for responses.
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<ElicitInputResponse>>>>,
    /// Channel for sending requests to the client.
    request_tx: Option<mpsc::Sender<ServerRequest>>,
    /// Default timeout for elicitation requests.
    timeout_duration: Duration,
}

impl std::fmt::Debug for ElicitationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElicitationManager")
            .field("has_request_tx", &self.request_tx.is_some())
            .field("timeout_duration", &self.timeout_duration)
            .finish()
    }
}

impl ElicitationManager {
    /// Create a new elicitation manager.
    pub fn new() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            request_tx: None,
            timeout_duration: Duration::from_secs(300), // 5 minutes default
        }
    }

    /// Set the request channel for sending elicitation requests.
    pub fn set_request_channel(&mut self, tx: mpsc::Sender<ServerRequest>) {
        self.request_tx = Some(tx);
    }

    /// Set the timeout duration for elicitation requests.
    pub fn set_timeout(&mut self, duration: Duration) {
        self.timeout_duration = duration;
    }

    /// Request input from the user.
    pub async fn elicit_input(&self, request: ElicitInputRequest) -> Result<ElicitInputResponse> {
        let request_tx = self.request_tx.as_ref().ok_or_else(|| {
            Error::protocol(ErrorCode::INTERNAL_ERROR, "Elicitation not configured")
        })?;

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Store pending request
        {
            let mut pending = self.pending.write().await;
            pending.insert(request.elicitation_id.clone(), tx);
        }

        let elicitation_id = request.elicitation_id.clone();

        // Send elicitation request
        let server_request = ServerRequest::ElicitInput(Box::new(request));
        if let Err(e) = request_tx.send(server_request).await {
            // Remove from pending on send error
            self.pending.write().await.remove(&elicitation_id);
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to send elicitation request: {}", e),
            ));
        }

        debug!("Sent elicitation request: {}", elicitation_id);

        // Wait for response with timeout
        match timeout(self.timeout_duration, rx).await {
            Ok(Ok(response)) => {
                debug!("Received elicitation response: {}", elicitation_id);
                Ok(response)
            },
            Ok(Err(_)) => {
                warn!("Elicitation channel closed: {}", elicitation_id);
                Err(Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    "Elicitation channel closed",
                ))
            },
            Err(_) => {
                warn!("Elicitation timeout: {}", elicitation_id);
                self.pending.write().await.remove(&elicitation_id);
                Err(Error::protocol(
                    ErrorCode::REQUEST_TIMEOUT,
                    "Elicitation request timed out",
                ))
            },
        }
    }

    /// Handle an elicitation response from the client.
    pub async fn handle_response(&self, response: ElicitInputResponse) -> Result<()> {
        let mut pending = self.pending.write().await;

        if let Some(tx) = pending.remove(&response.elicitation_id) {
            if tx.send(response).is_err() {
                warn!("Failed to deliver elicitation response - receiver dropped");
            }
            Ok(())
        } else {
            warn!(
                "Received response for unknown elicitation: {}",
                response.elicitation_id
            );
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Unknown elicitation ID",
            ))
        }
    }

    /// Cancel a pending elicitation request.
    pub async fn cancel(&self, elicitation_id: &str) -> Result<()> {
        let mut pending = self.pending.write().await;

        if let Some(tx) = pending.remove(elicitation_id) {
            // Send cancellation response
            let response = ElicitInputResponse {
                elicitation_id: elicitation_id.to_string(),
                value: None,
                cancelled: true,
                error: Some("Cancelled by server".to_string()),
            };

            if tx.send(response).is_err() {
                debug!("Elicitation already completed: {}", elicitation_id);
            }
            Ok(())
        } else {
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Unknown elicitation ID",
            ))
        }
    }

    /// Cancel all pending elicitation requests.
    pub async fn cancel_all(&self) {
        let mut pending = self.pending.write().await;

        for (id, tx) in pending.drain() {
            let response = ElicitInputResponse {
                elicitation_id: id,
                value: None,
                cancelled: true,
                error: Some("Server shutting down".to_string()),
            };

            let _ = tx.send(response);
        }
    }

    /// Get the number of pending elicitation requests.
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }
}

impl Default for ElicitationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for tool handlers to elicit input.
#[async_trait::async_trait]
pub trait ElicitInput {
    /// Request input from the user.
    async fn elicit_input(&self, request: ElicitInputRequest) -> Result<ElicitInputResponse>;
}

/// Context that provides elicitation capabilities to tool handlers.
#[derive(Debug)]
pub struct ElicitationContext {
    manager: Arc<ElicitationManager>,
}

impl ElicitationContext {
    /// Create a new elicitation context.
    pub fn new(manager: Arc<ElicitationManager>) -> Self {
        Self { manager }
    }
}

#[async_trait::async_trait]
impl ElicitInput for ElicitationContext {
    async fn elicit_input(&self, request: ElicitInputRequest) -> Result<ElicitInputResponse> {
        self.manager.elicit_input(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::elicitation::{elicit_text, InputType};

    #[tokio::test]
    async fn test_elicitation_manager() {
        let manager = ElicitationManager::new();

        // Should fail without request channel
        let request = elicit_text("Test prompt").build();
        let result = manager.elicit_input(request).await;
        assert!(result.is_err());

        // Set up request channel
        let (tx, mut rx) = mpsc::channel(10);
        let mut manager = manager;
        manager.set_request_channel(tx);
        manager.set_timeout(Duration::from_millis(100)); // Short timeout for test

        // Send request
        let request = elicit_text("Test prompt").build();
        let elicitation_id = request.elicitation_id.clone();

        let handle = tokio::spawn(async move { manager.elicit_input(request).await });

        // Verify request was sent
        let received = rx.recv().await.unwrap();
        match received {
            ServerRequest::ElicitInput(req) => {
                assert_eq!(req.elicitation_id, elicitation_id);
                assert_eq!(req.prompt, "Test prompt");
            },
            _ => panic!("Expected ElicitInput request"),
        }

        // Should timeout without response
        let result = handle.await.unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_elicitation_response_handling() {
        let _manager = Arc::new(ElicitationManager::new());
        let (tx, _rx) = mpsc::channel(10);

        let mut manager_mut = ElicitationManager::new();
        manager_mut.set_request_channel(tx);
        let manager = Arc::new(manager_mut);

        // Create request
        let request = elicit_text("Test prompt").build();
        let elicitation_id = request.elicitation_id.clone();

        // Start elicitation in background
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move { manager_clone.elicit_input(request).await });

        // Give it time to register
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Send response
        let response = ElicitInputResponse {
            elicitation_id: elicitation_id.clone(),
            value: Some(serde_json::json!("User input")),
            cancelled: false,
            error: None,
        };

        manager.handle_response(response).await.unwrap();

        // Check that elicitation completed
        let result = handle.await.unwrap().unwrap();
        assert_eq!(result.value, Some(serde_json::json!("User input")));
        assert!(!result.cancelled);
    }

    #[tokio::test]
    async fn test_elicitation_cancellation() {
        let _manager = Arc::new(ElicitationManager::new());
        let (tx, _rx) = mpsc::channel(10);

        let mut manager_mut = ElicitationManager::new();
        manager_mut.set_request_channel(tx);
        let manager = Arc::new(manager_mut);

        // Create request
        let request = elicit_text("Test prompt").build();
        let elicitation_id = request.elicitation_id.clone();

        // Start elicitation in background
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move { manager_clone.elicit_input(request).await });

        // Give it time to register
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Cancel the request
        manager.cancel(&elicitation_id).await.unwrap();

        // Check that elicitation was cancelled
        let result = handle.await.unwrap().unwrap();
        assert!(result.cancelled);
        assert_eq!(result.error, Some("Cancelled by server".to_string()));
    }
}
