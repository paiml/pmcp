//! HTTP/SSE transport implementation for MCP.

use crate::error::Result;
use crate::shared::sse_parser::SseParser;
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use url::Url;

/// HTTP transport configuration.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Base URL for HTTP requests
    pub base_url: Url,
    /// SSE endpoint for receiving notifications
    pub sse_endpoint: Option<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Additional headers to include in requests
    pub headers: Vec<(String, String)>,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Maximum idle connections in pool
    pub max_idle_per_host: usize,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".parse().expect("Valid default URL"),
            sse_endpoint: Some("/events".to_string()),
            timeout: Duration::from_secs(30),
            headers: vec![],
            enable_pooling: true,
            max_idle_per_host: 10,
        }
    }
}

/// HTTP/SSE transport implementation.
pub struct HttpTransport {
    config: HttpConfig,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>,
    message_queue: Arc<AsyncMutex<mpsc::Receiver<TransportMessage>>>,
    message_tx: mpsc::Sender<TransportMessage>,
    connected: Arc<RwLock<bool>>,
}

impl std::fmt::Debug for HttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransport")
            .field("config", &self.config)
            .field("connected", &self.connected)
            .finish_non_exhaustive()
    }
}

impl HttpTransport {
    /// Create a new HTTP transport with the given configuration.
    pub fn new(config: HttpConfig) -> Self {
        let connector = hyper_util::client::legacy::connect::HttpConnector::new();
        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(config.max_idle_per_host)
            .build(connector);

        let (tx, rx) = mpsc::channel(100);

        Self {
            config,
            client,
            message_queue: Arc::new(AsyncMutex::new(rx)),
            message_tx: tx,
            connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a new HTTP transport with default configuration.
    pub fn with_url(url: impl Into<Url>) -> Result<Self> {
        Ok(Self::new(HttpConfig {
            base_url: url.into(),
            ..Default::default()
        }))
    }

    /// Connect to SSE endpoint for receiving notifications.
    pub async fn connect_sse(&self) -> Result<()> {
        if let Some(sse_path) = &self.config.sse_endpoint {
            let sse_url = self
                .config
                .base_url
                .join(sse_path)
                .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;
            info!("Connecting to SSE endpoint: {}", sse_url);

            let req = Request::builder()
                .method(Method::GET)
                .uri(sse_url.as_str())
                .header("Accept", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .body(Full::new(Bytes::new()))
                .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;

            let response = self
                .client
                .request(req)
                .await
                .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;

            if response.status() != StatusCode::OK {
                return Err(crate::error::Error::Transport(
                    crate::error::TransportError::InvalidMessage(format!(
                        "SSE connection failed with status: {}",
                        response.status()
                    )),
                ));
            }

            // Spawn SSE reader task
            let message_tx = self.message_tx.clone();
            let connected = self.connected.clone();

            tokio::spawn(async move {
                *connected.write() = true;

                let mut body = response.into_body();
                let mut sse_parser = SseParser::new();

                while let Some(chunk) = body.frame().await {
                    match chunk {
                        Ok(frame) => {
                            if let Some(data) = frame.data_ref() {
                                let text = String::from_utf8_lossy(data);
                                let events = sse_parser.feed(&text);

                                for event in events {
                                    // Process SSE event data as JSON-RPC message
                                    match crate::shared::stdio::StdioTransport::parse_message(
                                        event.data.as_bytes(),
                                    ) {
                                        Ok(msg) => {
                                            if message_tx.send(msg).await.is_err() {
                                                error!("Failed to send SSE message");
                                                break;
                                            }
                                        },
                                        Err(e) => {
                                            error!("Failed to parse SSE message: {}", e);
                                        },
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            error!("SSE stream error: {}", e);
                            break;
                        },
                    }
                }

                *connected.write() = false;
                warn!("SSE connection closed");
            });
        } else {
            // No SSE endpoint configured, mark as connected for request/response only
            *self.connected.write() = true;
        }
        Ok(())
    }

    async fn send_request(&self, message: &TransportMessage) -> Result<()> {
        let json_bytes = crate::shared::stdio::StdioTransport::serialize_message(message)?;
        let json = String::from_utf8(json_bytes).map_err(|e| {
            crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(format!(
                "Invalid UTF-8: {}",
                e
            )))
        })?;

        let req = Request::builder()
            .method(Method::POST)
            .uri(self.config.base_url.as_str())
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json)))
            .map_err(|e| crate::error::TransportError::InvalidMessage(e.to_string()))?;

        let response = timeout(self.config.timeout, self.client.request(req))
            .await
            .map_err(|_| crate::error::Error::Timeout(self.config.timeout.as_secs() * 1000))?
            .map_err(|e| {
                crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(
                    e.to_string(),
                ))
            })?;

        if response.status() != StatusCode::OK {
            return Err(crate::error::Error::Transport(
                crate::error::TransportError::InvalidMessage(format!(
                    "HTTP request failed with status: {}",
                    response.status()
                )),
            ));
        }

        // Process response
        let body_bytes = response
            .collect()
            .await
            .map_err(|e| {
                crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(
                    e.to_string(),
                ))
            })?
            .to_bytes();
        let response_msg = crate::shared::stdio::StdioTransport::parse_message(&body_bytes)?;

        // Send response through message queue
        self.message_tx.send(response_msg).await.map_err(|_| {
            crate::error::Error::Transport(crate::error::TransportError::ConnectionClosed)
        })?;

        Ok(())
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        debug!("Sending HTTP message: {:?}", message);
        self.send_request(&message).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.message_queue.lock().await;
        rx.recv().await.ok_or_else(|| {
            crate::error::Error::Transport(crate::error::TransportError::ConnectionClosed)
        })
    }

    async fn close(&mut self) -> Result<()> {
        *self.connected.write() = false;
        info!("HTTP transport closed");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.connected.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_config_default() {
        let config = HttpConfig::default();
        assert!(config.enable_pooling);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.sse_endpoint, Some("/events".to_string()));
    }

    #[test]
    fn test_http_transport_creation() {
        let config = HttpConfig::default();
        let transport = HttpTransport::new(config);
        assert!(!transport.is_connected());
    }
}
