//! WebSocket server transport implementation.

use crate::error::{Error, Result};
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};

/// Configuration for WebSocket server transport.
#[derive(Debug, Clone)]
pub struct WebSocketServerConfig {
    /// Address to bind to
    pub bind_addr: SocketAddr,
    /// Maximum frame size in bytes
    pub max_frame_size: Option<usize>,
    /// Maximum message size in bytes
    pub max_message_size: Option<usize>,
    /// Whether to accept unmasked frames from clients
    pub accept_unmasked_frames: bool,
}

impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9001".parse().expect("Valid default address"),
            max_frame_size: Some(64 * 1024 * 1024),   // 64MB
            max_message_size: Some(64 * 1024 * 1024), // 64MB
            accept_unmasked_frames: false,
        }
    }
}

/// WebSocket server transport that accepts incoming connections.
pub struct WebSocketServerTransport {
    config: WebSocketServerConfig,
    listener: Option<TcpListener>,
    // Channels for communicating with the active connection
    incoming_rx: Arc<Mutex<Option<mpsc::Receiver<TransportMessage>>>>,
    outgoing_tx: Arc<Mutex<Option<mpsc::Sender<TransportMessage>>>>,
}

impl WebSocketServerTransport {
    /// Create a new WebSocket server transport with the given configuration.
    pub fn new(config: WebSocketServerConfig) -> Self {
        Self {
            config,
            listener: None,
            incoming_rx: Arc::new(Mutex::new(None)),
            outgoing_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new WebSocket server transport with default configuration.
    pub fn default_server() -> Self {
        Self::new(WebSocketServerConfig::default())
    }

    /// Bind and start listening for connections.
    pub async fn bind(&mut self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr)
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to bind to {}: {}",
                    self.config.bind_addr, e
                ))
            })?;
        info!("WebSocket server listening on {}", self.config.bind_addr);
        self.listener = Some(listener);
        Ok(())
    }

    /// Accept the next incoming connection and start handling it.
    ///
    /// This will wait for a client to connect, establish the WebSocket handshake,
    /// and spawn background tasks to handle the message flow.
    pub async fn accept(&mut self) -> Result<()> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| Error::internal("Server not bound"))?;

        let (tcp_stream, peer_addr) = listener
            .accept()
            .await
            .map_err(|e| Error::internal(format!("Failed to accept connection: {}", e)))?;
        info!("Accepting WebSocket connection from {}", peer_addr);

        // Accept the WebSocket handshake
        let ws_stream = accept_async(tcp_stream)
            .await
            .map_err(|e| Error::internal(format!("WebSocket handshake failed: {}", e)))?;

        info!("WebSocket connection established with {}", peer_addr);

        // Create channels for message passing
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<TransportMessage>(100);
        let (incoming_tx, incoming_rx) = mpsc::channel::<TransportMessage>(100);

        // Store the channels
        *self.incoming_rx.lock().await = Some(incoming_rx);
        *self.outgoing_tx.lock().await = Some(outgoing_tx);

        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        // Spawn task to handle outgoing messages
        tokio::spawn(async move {
            while let Some(msg) = outgoing_rx.recv().await {
                let json_bytes = match crate::shared::stdio::StdioTransport::serialize_message(&msg)
                {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                        continue;
                    },
                };

                let json = match String::from_utf8(json_bytes) {
                    Ok(json) => json,
                    Err(e) => {
                        error!("Failed to convert to UTF-8: {}", e);
                        continue;
                    },
                };

                if let Err(e) = ws_sink.send(Message::Text(json.into())).await {
                    error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        });

        // Spawn task to handle incoming messages
        tokio::spawn(async move {
            while let Some(result) = ws_stream.next().await {
                match result {
                    Ok(Message::Text(text)) => {
                        match crate::shared::stdio::StdioTransport::parse_message(text.as_bytes()) {
                            Ok(msg) => {
                                if let Err(e) = incoming_tx.send(msg).await {
                                    error!("Failed to queue incoming message: {}", e);
                                    break;
                                }
                            },
                            Err(e) => {
                                error!("Failed to parse message: {}", e);
                            },
                        }
                    },
                    Ok(Message::Binary(_)) => {
                        warn!("Received binary message, ignoring");
                    },
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed by peer");
                        break;
                    },
                    Ok(Message::Ping(_data)) => {
                        // TODO: Handle ping/pong properly
                        warn!("Received ping, automatic pong not yet implemented");
                    },
                    Ok(_) => {
                        // All message types, ignore (including Pong)
                    },
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    },
                }
            }

            // Connection closed, cleanup
            info!("WebSocket connection closed");
        });

        Ok(())
    }
}

#[async_trait]
impl Transport for WebSocketServerTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        let tx_guard = self.outgoing_tx.lock().await;
        let tx = tx_guard
            .as_ref()
            .ok_or_else(|| Error::internal("No active connection"))?;

        let result = tx
            .send(message)
            .await
            .map_err(|_| Error::internal("Failed to send message"));
        drop(tx_guard);
        result?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx_guard = self.incoming_rx.lock().await;
        let rx = rx_guard
            .as_mut()
            .ok_or_else(|| Error::internal("No active connection"))?;

        let result = rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("Connection closed"));
        drop(rx_guard);
        result
    }

    async fn close(&mut self) -> Result<()> {
        // Clear the channels to signal closure
        *self.incoming_rx.lock().await = None;
        *self.outgoing_tx.lock().await = None;

        info!("WebSocket server transport closed");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        // Check if we have active channels
        futures::executor::block_on(async { self.outgoing_tx.lock().await.is_some() })
    }

    fn transport_type(&self) -> &'static str {
        "websocket-server"
    }
}

impl std::fmt::Debug for WebSocketServerTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketServerTransport")
            .field("config", &self.config)
            .field("listener", &self.listener.is_some())
            .field("has_active_connection", &self.is_connected())
            .finish()
    }
}

/// Builder for WebSocket server transport.
#[derive(Debug)]
pub struct WebSocketServerBuilder {
    config: WebSocketServerConfig,
}

impl WebSocketServerBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            config: WebSocketServerConfig::default(),
        }
    }

    /// Set the bind address.
    pub fn bind_addr(mut self, addr: SocketAddr) -> Self {
        self.config.bind_addr = addr;
        self
    }

    /// Set the maximum frame size.
    pub fn max_frame_size(mut self, size: usize) -> Self {
        self.config.max_frame_size = Some(size);
        self
    }

    /// Set the maximum message size.
    pub fn max_message_size(mut self, size: usize) -> Self {
        self.config.max_message_size = Some(size);
        self
    }

    /// Set whether to accept unmasked frames.
    pub fn accept_unmasked_frames(mut self, accept: bool) -> Self {
        self.config.accept_unmasked_frames = accept;
        self
    }

    /// Build the transport.
    pub fn build(self) -> WebSocketServerTransport {
        WebSocketServerTransport::new(self.config)
    }
}

impl Default for WebSocketServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = WebSocketServerConfig::default();
        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:9001");
        assert_eq!(config.max_frame_size, Some(64 * 1024 * 1024));
        assert!(!config.accept_unmasked_frames);
    }

    #[test]
    fn test_builder() {
        let transport = WebSocketServerBuilder::new()
            .bind_addr("127.0.0.1:9002".parse().unwrap())
            .max_frame_size(1024 * 1024)
            .max_message_size(2 * 1024 * 1024)
            .accept_unmasked_frames(true)
            .build();

        assert_eq!(transport.config.bind_addr.to_string(), "127.0.0.1:9002");
        assert_eq!(transport.config.max_frame_size, Some(1024 * 1024));
        assert_eq!(transport.config.max_message_size, Some(2 * 1024 * 1024));
        assert!(transport.config.accept_unmasked_frames);
    }
}
