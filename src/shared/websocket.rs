//! WebSocket transport implementation for MCP.

use crate::error::Result;
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};
use url::Url;

/// WebSocket transport configuration.
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// WebSocket URL to connect to
    pub url: Url,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Initial reconnection delay (doubles on each attempt)
    pub reconnect_delay: Duration,
    /// Maximum reconnection delay
    pub max_reconnect_delay: Duration,
    /// Maximum number of reconnection attempts (None = infinite)
    pub max_reconnect_attempts: Option<u32>,
    /// Ping interval for keepalive
    pub ping_interval: Option<Duration>,
    /// Request timeout
    pub request_timeout: Duration,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8080".parse().expect("Valid default URL"),
            auto_reconnect: true,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
            max_reconnect_attempts: None,
            ping_interval: Some(Duration::from_secs(30)),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// WebSocket transport implementation.
pub struct WebSocketTransport {
    config: WebSocketConfig,
    state: Arc<RwLock<ConnectionState>>,
    message_tx: mpsc::Sender<TransportMessage>,
    message_rx: Arc<AsyncMutex<mpsc::Receiver<TransportMessage>>>,
}

#[derive(Debug)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Closing,
}

impl std::fmt::Debug for WebSocketTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketTransport")
            .field("config", &self.config)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl WebSocketTransport {
    /// Create a new WebSocket transport with the given configuration.
    pub fn new(config: WebSocketConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            message_tx: tx,
            message_rx: Arc::new(AsyncMutex::new(rx)),
        }
    }

    /// Create a new WebSocket transport with default configuration.
    pub fn with_url(url: impl Into<Url>) -> Result<Self> {
        Ok(Self::new(WebSocketConfig {
            url: url.into(),
            ..Default::default()
        }))
    }

    /// Connect to the WebSocket server.
    pub async fn connect(&self) -> Result<()> {
        self.connect_with_retry().await
    }

    async fn connect_with_retry(&self) -> Result<()> {
        let mut attempts = 0;
        let mut delay = self.config.reconnect_delay;

        loop {
            match self.connect_once().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if let Some(max) = self.config.max_reconnect_attempts {
                        if attempts >= max {
                            error!("Max reconnection attempts reached");
                            return Err(e);
                        }
                    }

                    warn!(
                        "Connection attempt {} failed: {}. Retrying in {:?}",
                        attempts, e, delay
                    );

                    sleep(delay).await;
                    delay = (delay * 2).min(self.config.max_reconnect_delay);
                },
            }
        }
    }

    async fn connect_once(&self) -> Result<()> {
        {
            let mut state = self.state.write();
            *state = ConnectionState::Connecting;
        }

        info!("Connecting to WebSocket at {}", self.config.url);

        let (ws_stream, _) = connect_async(self.config.url.as_str())
            .await
            .map_err(crate::error::TransportError::from)?;
        let (sink, stream) = ws_stream.split();

        {
            let mut state = self.state.write();
            *state = ConnectionState::Connected;
        }

        info!("WebSocket connected");

        // Spawn reader task
        let message_tx = self.message_tx.clone();
        let _reader_handle = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(_json) => {
                                // Parse the JSON value into a TransportMessage
                                match crate::shared::stdio::StdioTransport::parse_message(
                                    text.as_bytes(),
                                ) {
                                    Ok(msg) => {
                                        if message_tx.send(msg).await.is_err() {
                                            error!("Failed to send message to channel");
                                            break;
                                        }
                                    },
                                    Err(e) => {
                                        error!("Failed to parse message: {}", e);
                                    },
                                }
                            },
                            Err(e) => {
                                error!("Failed to parse WebSocket message: {}", e);
                            },
                        }
                    },
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed by remote");
                        break;
                    },
                    Ok(Message::Ping(data)) => {
                        debug!("Received ping: {:?}", data);
                    },
                    Ok(Message::Pong(_)) => {
                        debug!("Received pong");
                    },
                    Ok(Message::Binary(_)) => {
                        warn!("Received unexpected binary message");
                    },
                    Ok(Message::Frame(_)) => {
                        warn!("Received unexpected frame message");
                    },
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    },
                }
            }
        });

        // Spawn writer task
        let (write_tx, mut write_rx) = mpsc::channel::<Message>(100);
        let _writer_handle = tokio::spawn(async move {
            let mut sink = sink;
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = sink.send(msg).await {
                    error!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        });

        // Spawn ping task if configured
        if let Some(ping_interval) = self.config.ping_interval {
            tokio::spawn(async move {
                let mut ticker = interval(ping_interval);
                loop {
                    ticker.tick().await;
                    if write_tx.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Store the writer channel for sending messages
        // This is a simplified approach - in production you'd want better state management

        Ok(())
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        let json_bytes = crate::shared::stdio::StdioTransport::serialize_message(&message)?;
        let json = String::from_utf8(json_bytes).map_err(|e| {
            crate::error::Error::Transport(crate::error::TransportError::InvalidMessage(format!(
                "Invalid UTF-8: {}",
                e
            )))
        })?;

        // In a real implementation, we'd need to store the write channel
        // This is simplified for the example
        match &*self.state.read() {
            ConnectionState::Connected => {
                debug!("Sending WebSocket message: {}", json);
                Ok(())
            },
            _ => Err(crate::error::Error::Transport(
                crate::error::TransportError::ConnectionClosed,
            )),
        }
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.message_rx.lock().await;
        rx.recv().await.ok_or_else(|| {
            crate::error::Error::Transport(crate::error::TransportError::ConnectionClosed)
        })
    }

    async fn close(&mut self) -> Result<()> {
        {
            let mut state = self.state.write();
            *state = ConnectionState::Closing;
        }

        info!("Closing WebSocket connection");

        {
            let mut state = self.state.write();
            *state = ConnectionState::Disconnected;
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        matches!(&*self.state.read(), ConnectionState::Connected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert!(config.auto_reconnect);
        assert_eq!(config.reconnect_delay, Duration::from_secs(1));
        assert_eq!(config.max_reconnect_delay, Duration::from_secs(60));
        assert!(config.max_reconnect_attempts.is_none());
        assert_eq!(config.ping_interval, Some(Duration::from_secs(30)));
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.url.as_str(), "ws://localhost:8080/");
    }

    #[test]
    fn test_websocket_config_custom() {
        let config = WebSocketConfig {
            url: "wss://example.com:9000/ws".parse().unwrap(),
            auto_reconnect: false,
            reconnect_delay: Duration::from_secs(2),
            max_reconnect_delay: Duration::from_secs(120),
            max_reconnect_attempts: Some(5),
            ping_interval: None,
            request_timeout: Duration::from_secs(60),
        };
        assert_eq!(config.url.as_str(), "wss://example.com:9000/ws");
        assert!(!config.auto_reconnect);
        assert_eq!(config.reconnect_delay, Duration::from_secs(2));
        assert_eq!(config.max_reconnect_attempts, Some(5));
        assert!(config.ping_interval.is_none());
    }

    #[test]
    fn test_websocket_transport_creation() {
        let config = WebSocketConfig::default();
        let transport = WebSocketTransport::new(config);
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_websocket_transport_with_url() {
        let transport = WebSocketTransport::with_url("ws://test.example.com:3000".parse::<Url>().unwrap()).unwrap();
        assert!(!transport.is_connected());
        assert_eq!(transport.config.url.as_str(), "ws://test.example.com:3000/");
    }

    #[test]
    fn test_websocket_config_clone() {
        let config = WebSocketConfig::default();
        let cloned = config.clone();
        assert_eq!(config.url, cloned.url);
        assert_eq!(config.auto_reconnect, cloned.auto_reconnect);
        assert_eq!(config.reconnect_delay, cloned.reconnect_delay);
    }

    #[test]
    fn test_websocket_transport_debug() {
        let config = WebSocketConfig::default();
        let transport = WebSocketTransport::new(config);
        let debug_str = format!("{:?}", transport);
        assert!(debug_str.contains("WebSocketTransport"));
        assert!(debug_str.contains("config"));
        assert!(debug_str.contains("state"));
    }

    #[test]
    fn test_connection_state_transitions() {
        let state = Arc::new(RwLock::new(ConnectionState::Disconnected));
        
        // Initial state
        assert!(matches!(&*state.read(), ConnectionState::Disconnected));
        
        // Transition to connecting
        {
            let mut s = state.write();
            *s = ConnectionState::Connecting;
        }
        assert!(matches!(&*state.read(), ConnectionState::Connecting));
        
        // Transition to connected
        {
            let mut s = state.write();
            *s = ConnectionState::Connected;
        }
        assert!(matches!(&*state.read(), ConnectionState::Connected));
        
        // Transition to closing
        {
            let mut s = state.write();
            *s = ConnectionState::Closing;
        }
        assert!(matches!(&*state.read(), ConnectionState::Closing));
        
        // Back to disconnected
        {
            let mut s = state.write();
            *s = ConnectionState::Disconnected;
        }
        assert!(matches!(&*state.read(), ConnectionState::Disconnected));
    }

    #[tokio::test]
    async fn test_websocket_close() {
        let config = WebSocketConfig::default();
        let mut transport = WebSocketTransport::new(config);
        
        // Set to connected state
        {
            let mut state = transport.state.write();
            *state = ConnectionState::Connected;
        }
        assert!(transport.is_connected());
        
        // Close should transition through states
        transport.close().await.unwrap();
        assert!(!transport.is_connected());
        assert!(matches!(&*transport.state.read(), ConnectionState::Disconnected));
    }

    #[tokio::test]
    async fn test_send_when_not_connected() {
        use crate::types::{ClientRequest, Request, RequestId};
        
        let config = WebSocketConfig::default();
        let mut transport = WebSocketTransport::new(config);
        
        let message = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };
        
        // Should fail when not connected
        let result = transport.send(message).await;
        assert!(result.is_err());
        if let Err(crate::error::Error::Transport(e)) = result {
            assert!(matches!(e, crate::error::TransportError::ConnectionClosed));
        }
    }

    #[tokio::test]
    async fn test_send_when_connected() {
        use crate::types::{ClientRequest, Request, RequestId};
        
        let config = WebSocketConfig::default();
        let mut transport = WebSocketTransport::new(config);
        
        // Set to connected state
        {
            let mut state = transport.state.write();
            *state = ConnectionState::Connected;
        }
        
        let message = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };
        
        // Should succeed when connected (though simplified implementation just returns Ok)
        let result = transport.send(message).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_receive_when_channel_closed() {
        let config = WebSocketConfig::default();
        let transport = WebSocketTransport::new(config);
        
        // Create a new receiver that's already closed
        let (_, rx) = mpsc::channel::<TransportMessage>(1);
        let mut transport = WebSocketTransport {
            config: transport.config,
            state: transport.state,
            message_tx: transport.message_tx,
            message_rx: Arc::new(AsyncMutex::new(rx)),
        };
        
        // Receive should error with ConnectionClosed
        let result = transport.receive().await;
        assert!(result.is_err());
        if let Err(crate::error::Error::Transport(e)) = result {
            assert!(matches!(e, crate::error::TransportError::ConnectionClosed));
        }
    }

    #[test]
    fn test_websocket_config_with_finite_retries() {
        let config = WebSocketConfig {
            url: "ws://localhost:8080".parse().unwrap(),
            auto_reconnect: true,
            max_reconnect_attempts: Some(3),
            ..Default::default()
        };
        assert_eq!(config.max_reconnect_attempts, Some(3));
    }

    #[test]
    fn test_websocket_config_with_no_ping() {
        let config = WebSocketConfig {
            url: "ws://localhost:8080".parse().unwrap(),
            ping_interval: None,
            ..Default::default()
        };
        assert!(config.ping_interval.is_none());
    }
}
