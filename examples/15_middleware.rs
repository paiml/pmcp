//! Example of using middleware for request/response processing.

use async_trait::async_trait;
use pmcp::shared::TransportMessage;
use pmcp::types::{JSONRPCRequest, JSONRPCResponse};
use pmcp::{
    Client, ClientCapabilities, LoggingMiddleware, Middleware, MiddlewareChain, StdioTransport,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, Level};

/// Custom middleware that tracks request timing
struct TimingMiddleware {
    request_count: AtomicU64,
    start_times: dashmap::DashMap<String, Instant>,
}

impl TimingMiddleware {
    fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            start_times: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl Middleware for TimingMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> pmcp::Result<()> {
        let count = self.request_count.fetch_add(1, Ordering::SeqCst);
        info!("Request #{}: {}", count + 1, request.method);

        // Track start time
        self.start_times
            .insert(request.id.to_string(), Instant::now());

        Ok(())
    }

    async fn on_response(&self, response: &mut JSONRPCResponse) -> pmcp::Result<()> {
        // Calculate elapsed time
        if let Some((_, start)) = self.start_times.remove(&response.id.to_string()) {
            let elapsed = start.elapsed();
            info!("Response for {} took {:?}", response.id, elapsed);
        }

        Ok(())
    }
}

/// Custom middleware that adds metadata
struct MetadataMiddleware {
    client_id: String,
}

#[async_trait]
impl Middleware for MetadataMiddleware {
    async fn on_send(&self, _message: &TransportMessage) -> pmcp::Result<()> {
        info!("Client {} sending message", self.client_id);
        Ok(())
    }

    async fn on_receive(&self, _message: &TransportMessage) -> pmcp::Result<()> {
        info!("Client {} received message", self.client_id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Create middleware chain
    let mut middleware = MiddlewareChain::new();

    // Add logging middleware
    middleware.add(Arc::new(LoggingMiddleware::new(Level::DEBUG)));

    // Add timing middleware
    middleware.add(Arc::new(TimingMiddleware::new()));

    // Add metadata middleware
    middleware.add(Arc::new(MetadataMiddleware {
        client_id: "example-client".to_string(),
    }));

    info!("Creating client with middleware");

    // In a real implementation, you would integrate middleware with the transport
    // For this example, we'll just show the middleware setup
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // The middleware would process all requests and responses
    let capabilities = ClientCapabilities::default();

    info!("Initializing connection (middleware will track this)");
    match client.initialize(capabilities).await {
        Ok(server_info) => {
            info!(
                "Connected to: {} v{}",
                server_info.server_info.name, server_info.server_info.version
            );
        },
        Err(e) => {
            info!("Connection failed (expected in example): {}", e);
        },
    }

    Ok(())
}
