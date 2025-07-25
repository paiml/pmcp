//! MCP client implementation.

use crate::error::Result;
use crate::shared::{Protocol, ProtocolOptions, Transport};
use crate::types::{ClientCapabilities, ServerCapabilities};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod transport;

/// MCP client for connecting to servers.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::{Client, StdioTransport, ClientCapabilities};
///
/// # async fn example() -> pmcp::Result<()> {
/// let transport = StdioTransport::new();
/// let mut client = Client::new(transport);
///
/// // Initialize connection
/// let server_info = client.initialize(ClientCapabilities::default()).await?;
/// println!("Connected to: {}", server_info.server_info.name);
/// # Ok(())
/// # }
/// ```
#[allow(dead_code)]
pub struct Client<T: Transport> {
    transport: Arc<RwLock<T>>,
    protocol: Protocol,
    capabilities: Option<ClientCapabilities>,
    server_capabilities: Option<ServerCapabilities>,
    initialized: bool,
}

impl<T: Transport> std::fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("transport", &"<Arc<RwLock<Transport>>>")
            .field("protocol", &self.protocol)
            .field("capabilities", &self.capabilities)
            .field("server_capabilities", &self.server_capabilities)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl<T: Transport> Client<T> {
    /// Create a new client with the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Protocol::new(ProtocolOptions::default()),
            capabilities: None,
            server_capabilities: None,
            initialized: false,
        }
    }

    /// Create a new client with custom protocol options.
    pub fn with_options(transport: T, options: ProtocolOptions) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Protocol::new(options),
            capabilities: None,
            server_capabilities: None,
            initialized: false,
        }
    }

    /// Initialize the connection with the server.
    #[allow(clippy::unused_async)]
    pub async fn initialize(
        &mut self,
        capabilities: ClientCapabilities,
    ) -> Result<crate::types::InitializeResult> {
        if self.initialized {
            return Err(crate::Error::InvalidState(
                "Client already initialized".into(),
            ));
        }

        self.capabilities = Some(capabilities);
        self.initialized = true;

        unimplemented!("Client initialization not yet implemented")
    }

    /// List available tools.
    #[allow(clippy::unused_async)]
    pub async fn list_tools(
        &self,
        _cursor: Option<String>,
    ) -> Result<crate::types::ListToolsResult> {
        self.ensure_initialized()?;
        unimplemented!("Tool listing not yet implemented")
    }

    /// Check if client is initialized.
    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(crate::Error::InvalidState("Client not initialized".into()))
        }
    }
}

/// Builder for creating clients with custom configuration.
pub struct ClientBuilder<T: Transport> {
    transport: T,
    options: ProtocolOptions,
}

impl<T: Transport> std::fmt::Debug for ClientBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("transport", &"<Transport>")
            .field("options", &self.options)
            .finish()
    }
}

impl<T: Transport> ClientBuilder<T> {
    /// Create a new client builder.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            options: ProtocolOptions::default(),
        }
    }

    /// Set whether to enforce strict capabilities.
    pub fn enforce_strict_capabilities(mut self, enforce: bool) -> Self {
        self.options.enforce_strict_capabilities = enforce;
        self
    }

    /// Set debounced notification methods.
    pub fn debounced_notifications(mut self, methods: Vec<String>) -> Self {
        self.options.debounced_notification_methods = methods;
        self
    }

    /// Build the client.
    pub fn build(self) -> Client<T> {
        Client::with_options(self.transport, self.options)
    }
}
