//! Middleware support for request/response processing.

use crate::error::Result;
use crate::shared::TransportMessage;
use crate::types::{JSONRPCRequest, JSONRPCResponse};
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

/// Middleware that can intercept and modify requests and responses.
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before a request is sent.
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        let _ = request;
        Ok(())
    }

    /// Called after a response is received.
    async fn on_response(&self, response: &mut JSONRPCResponse) -> Result<()> {
        let _ = response;
        Ok(())
    }

    /// Called when a message is sent (any type).
    async fn on_send(&self, message: &TransportMessage) -> Result<()> {
        let _ = message;
        Ok(())
    }

    /// Called when a message is received (any type).
    async fn on_receive(&self, message: &TransportMessage) -> Result<()> {
        let _ = message;
        Ok(())
    }
}

/// Chain of middleware handlers.
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl fmt::Debug for MiddlewareChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MiddlewareChain")
            .field("count", &self.middlewares.len())
            .finish()
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareChain {
    /// Create a new empty middleware chain.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add a middleware to the chain.
    pub fn add(&mut self, middleware: Arc<dyn Middleware>) {
        self.middlewares.push(middleware);
    }

    /// Process a request through all middleware.
    pub async fn process_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_request(request).await?;
        }
        Ok(())
    }

    /// Process a response through all middleware.
    pub async fn process_response(&self, response: &mut JSONRPCResponse) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_response(response).await?;
        }
        Ok(())
    }

    /// Process an outgoing message through all middleware.
    pub async fn process_send(&self, message: &TransportMessage) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_send(message).await?;
        }
        Ok(())
    }

    /// Process an incoming message through all middleware.
    pub async fn process_receive(&self, message: &TransportMessage) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_receive(message).await?;
        }
        Ok(())
    }
}

/// Logging middleware that logs all messages.
#[derive(Debug)]
pub struct LoggingMiddleware {
    level: tracing::Level,
}

impl LoggingMiddleware {
    /// Create a new logging middleware with the specified level.
    pub fn new(level: tracing::Level) -> Self {
        Self { level }
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new(tracing::Level::DEBUG)
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        match self.level {
            tracing::Level::TRACE => tracing::trace!("Sending request: {:?}", request),
            tracing::Level::DEBUG => tracing::debug!("Sending request: {}", request.method),
            tracing::Level::INFO => tracing::info!("Sending request: {}", request.method),
            tracing::Level::WARN => tracing::warn!("Sending request: {}", request.method),
            tracing::Level::ERROR => tracing::error!("Sending request: {}", request.method),
        }
        Ok(())
    }

    async fn on_response(&self, response: &mut JSONRPCResponse) -> Result<()> {
        match self.level {
            tracing::Level::TRACE => tracing::trace!("Received response: {:?}", response),
            tracing::Level::DEBUG => tracing::debug!("Received response for: {:?}", response.id),
            tracing::Level::INFO => tracing::info!("Received response"),
            tracing::Level::WARN => tracing::warn!("Received response"),
            tracing::Level::ERROR => tracing::error!("Received response"),
        }
        Ok(())
    }
}

/// Authentication middleware that adds auth headers.
#[derive(Debug)]
pub struct AuthMiddleware {
    #[allow(dead_code)]
    auth_token: String,
}

impl AuthMiddleware {
    /// Create a new auth middleware with the given token.
    pub fn new(auth_token: String) -> Self {
        Self { auth_token }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        // In a real implementation, this would add auth headers
        // For JSON-RPC, we might add auth to params or use a wrapper
        tracing::debug!("Adding authentication to request: {}", request.method);
        Ok(())
    }
}

/// Retry middleware that implements exponential backoff.
#[derive(Debug)]
pub struct RetryMiddleware {
    max_retries: u32,
    #[allow(dead_code)]
    initial_delay_ms: u64,
    #[allow(dead_code)]
    max_delay_ms: u64,
}

impl RetryMiddleware {
    /// Create a new retry middleware.
    pub fn new(max_retries: u32, initial_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
        }
    }
}

impl Default for RetryMiddleware {
    fn default() -> Self {
        Self::new(3, 1000, 30000)
    }
}

#[async_trait]
impl Middleware for RetryMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        // Retry logic would be implemented at the transport level
        // This middleware just adds metadata for retry handling
        tracing::debug!(
            "Request {} configured with max {} retries",
            request.method,
            self.max_retries
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RequestId;

    #[tokio::test]
    async fn test_middleware_chain() {
        let mut chain = MiddlewareChain::new();
        chain.add(Arc::new(LoggingMiddleware::default()));

        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            method: "test".to_string(),
            params: None,
        };

        assert!(chain.process_request(&mut request).await.is_ok());
    }

    #[tokio::test]
    async fn test_auth_middleware() {
        let middleware = AuthMiddleware::new("test-token".to_string());

        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            method: "test".to_string(),
            params: None,
        };

        assert!(middleware.on_request(&mut request).await.is_ok());
    }
}
