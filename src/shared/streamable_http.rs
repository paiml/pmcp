use crate::error::{Error, Result, TransportError};
use crate::shared::reconnect::{ReconnectConfig, ReconnectManager};
use crate::shared::sse::SseParser;
use crate::shared::{Transport, TransportMessage};
use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::{Client, RequestBuilder};
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::Mutex;
use url::Url;

/// Configuration for the StreamableHttpTransport.
#[derive(Debug, Clone)]
pub struct StreamableHttpTransportConfig {
    /// The URL of the MCP server.
    pub url: Url,
    /// Optional extra headers to add to each request.
    pub extra_headers: Option<Vec<(String, String)>>,
    /// An optional `AuthProvider` to use for authentication.
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    /// An optional session ID to use for the connection.
    pub session_id: Option<String>,
    /// An optional `ReconnectConfig` to use for reconnection.
    pub reconnect_config: Option<ReconnectConfig>,
}

/// A streamable HTTP transport for MCP.
#[derive(Debug, Clone)]
pub struct StreamableHttpTransport {
    config: StreamableHttpTransportConfig,
    client: Client,
    receiver: Arc<Mutex<Receiver<TransportMessage>>>,
    sender: Sender<TransportMessage>,
    is_connected: Arc<RwLock<bool>>,
    reconnect_manager: Option<Arc<ReconnectManager>>,
}

impl StreamableHttpTransport {
    /// Creates a new StreamableHttpTransport.
    pub fn new(config: StreamableHttpTransportConfig) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        let reconnect_manager = config
            .reconnect_config
            .clone()
            .map(|c| Arc::new(ReconnectManager::new(c)));
        Self {
            config,
            client: Client::new(),
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_connected: Arc::new(RwLock::new(false)),
            reconnect_manager,
        }
    }

    /// Starts the transport and initiates the connection.
    pub async fn start(&self) -> Result<()> {
        if let Some(reconnect_manager) = &self.reconnect_manager {
            let transport = self.clone();
            reconnect_manager
                .reconnect_with(move || {
                    let transport = transport.clone();
                    async move { transport.connect().await }
                })
                .await?;
        } else {
            self.connect().await?;
        }
        Ok(())
    }

    async fn connect(&self) -> Result<()> {
        // For streamable http, the connection is established when the first message is sent.
        // We can't really "connect" beforehand, so we'll just set the connected flag to true.
        *self.is_connected.write() = true;
        Ok(())
    }

    async fn build_request(&self, method: reqwest::Method, url: Url) -> Result<RequestBuilder> {
        let mut builder = self.client.request(method, url);
        if let Some(headers) = &self.config.extra_headers {
            for (key, value) in headers {
                builder = builder.header(key, value);
            }
        }
        if let Some(auth_provider) = &self.config.auth_provider {
            let token = auth_provider.get_access_token().await?;
            builder = builder.bearer_auth(token);
        }
        if let Some(session_id) = &self.config.session_id {
            builder = builder.header("mcp-session-id", session_id);
        }
        Ok(builder)
    }
}

#[async_trait]
impl Transport for StreamableHttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        let body = serde_json::to_string(&message)
            .map_err(|e| Error::Transport(TransportError::Serialization(e.to_string())))?;

        let builder = self
            .build_request(reqwest::Method::POST, self.config.url.clone())
            .await?;

        let mut response = builder
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        if !response.status().is_success() {
            return Err(Error::Transport(TransportError::Request(format!(
                "Request failed with status: {}",
                response.status()
            ))));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type.contains("text/event-stream") {
            let sender = self.sender.clone();
            let is_connected = self.is_connected.clone();
            tokio::spawn(async move {
                *is_connected.write() = true;
                let mut sse_parser = SseParser::new();
                while let Some(chunk) = response.chunk().await.map_err(|e| {
                    Error::Transport(TransportError::Request(e.to_string()))
                })? {
                    let events = sse_parser.parse(&chunk);
                    for event in events {
                        if event.event == "message" {
                            let msg: TransportMessage = serde_json::from_str(&event.data)
                                .map_err(|e| Error::Transport(TransportError::Deserialization(e.to_string())))?;
                            sender.send(msg).await.map_err(|e| {
                                Error::Transport(TransportError::Send(e.to_string()))
                            })?;
                        }
                    }
                }
                *is_connected.write() = false;
                Ok::<(), Error>(())
            });
        } else {
            let response_bytes = response
                .bytes()
                .await
                .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;
            let message = serde_json::from_slice(&response_bytes)
                .map_err(|e| Error::Transport(TransportError::Deserialization(e.to_string())))?;
            self.sender.send(message).await.map_err(|e| {
                Error::Transport(TransportError::Send(e.to_string()))
            })?;
        }

        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut receiver = self.receiver.lock().await;
        receiver.recv().await.ok_or_else(|| {
            Error::Transport(TransportError::ConnectionClosed)
        })
    }

    async fn close(&mut self) -> Result<()> {
        *self.is_connected.write() = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        *self.is_connected.read()
    }
}

/// A trait for providing authentication tokens.
#[async_trait]
pub trait AuthProvider: Send + Sync + Debug {
    /// Returns an access token.
    async fn get_access_token(&self) -> Result<String>;
}
