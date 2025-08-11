use crate::error::{Error, Result, TransportError};
use crate::shared::http_constants::*;
use crate::shared::reconnect::{ReconnectConfig, ReconnectManager};
use crate::shared::sse::{SseEvent, SseParser};
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
#[derive(Clone)]
pub struct StreamableHttpTransportConfig {
    pub url: Url,
    pub extra_headers: Vec<(String, String)>,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub reconnect_config: Option<ReconnectConfig>,
    pub on_resumption_token: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl Debug for StreamableHttpTransportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransportConfig")
            .field("url", &self.url)
            .field("extra_headers", &self.extra_headers)
            .field("auth_provider", &self.auth_provider.is_some())
            .field("reconnect_config", &self.reconnect_config)
            .field("on_resumption_token", &self.on_resumption_token.is_some())
            .finish()
    }
}

/// A streamable HTTP transport for MCP.
#[derive(Clone)]
pub struct StreamableHttpTransport {
    config: Arc<StreamableHttpTransportConfig>,
    client: Client,
    receiver: Arc<Mutex<Receiver<TransportMessage>>>,
    sender: Sender<TransportMessage>,
    is_connected: Arc<RwLock<bool>>,
    reconnect_manager: Option<Arc<ReconnectManager>>,
    session_id: Arc<RwLock<Option<String>>>,
    protocol_version: Arc<RwLock<Option<String>>>,
}

impl Debug for StreamableHttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpTransport")
            .field("config", &self.config)
            .field("is_connected", &self.is_connected)
            .field("session_id", &self.session_id)
            .field("protocol_version", &self.protocol_version)
            .finish()
    }
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
            config: Arc::new(config),
            client: Client::new(),
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_connected: Arc::new(RwLock::new(false)),
            reconnect_manager,
            session_id: Arc::new(RwLock::new(None)),
            protocol_version: Arc::new(RwLock::new(None)),
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

        // Add extra headers from config
        for (key, value) in &self.config.extra_headers {
            builder = builder.header(key, value);
        }

        // Add auth header if provider is present
        if let Some(auth_provider) = &self.config.auth_provider {
            let token = auth_provider.get_access_token().await?;
            builder = builder.bearer_auth(token);
        }

        // Add session ID header if we have one
        if let Some(session_id) = self.session_id.read().as_ref() {
            builder = builder.header(MCP_SESSION_ID, session_id);
        }

        // Add protocol version header if we have one
        if let Some(protocol_version) = self.protocol_version.read().as_ref() {
            builder = builder.header(MCP_PROTOCOL_VERSION, protocol_version);
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
            .header(CONTENT_TYPE, APPLICATION_JSON)
            .header(ACCEPT, ACCEPT_STREAMABLE)
            .body(body)
            .send()
            .await
            .map_err(|e| Error::Transport(TransportError::Request(e.to_string())))?;

        // Update session ID from response header
        if let Some(session_id) = response.headers().get(MCP_SESSION_ID) {
            if let Ok(session_id_str) = session_id.to_str() {
                *self.session_id.write() = Some(session_id_str.to_string());
            }
        }

        // Update protocol version from response header
        if let Some(protocol_version) = response.headers().get(MCP_PROTOCOL_VERSION) {
            if let Ok(protocol_version_str) = protocol_version.to_str() {
                *self.protocol_version.write() = Some(protocol_version_str.to_string());
            }
        }

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
