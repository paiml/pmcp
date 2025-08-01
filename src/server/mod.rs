//! MCP server implementation.

use crate::error::{Error, Result};
use crate::shared::{Protocol, ProtocolOptions, TransportMessage};
use crate::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, GetPromptRequest,
    Implementation, InitializeResult, JSONRPCResponse, ListPromptsRequest, ListPromptsResult,
    ListResourceTemplatesRequest, ListResourceTemplatesResult, ListResourcesRequest,
    ListResourcesResult, ListToolsRequest, ListToolsResult, Notification, ProtocolVersion,
    ReadResourceRequest, Request, RequestId, ServerCapabilities, ServerNotification,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub mod auth;
pub mod batch;
pub mod cancellation;
pub mod elicitation;
#[cfg(feature = "resource-watcher")]
pub mod resource_watcher;
pub mod roots;
pub mod subscriptions;
pub mod transport;

/// Handler for tool execution.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Handle a tool call with the given arguments.
    async fn handle(&self, args: Value, extra: cancellation::RequestHandlerExtra) -> Result<Value>;
}

/// Handler for prompt generation.
#[async_trait]
pub trait PromptHandler: Send + Sync {
    /// Generate a prompt with the given arguments.
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::GetPromptResult>;
}

/// Handler for resource access.
#[async_trait]
pub trait ResourceHandler: Send + Sync {
    /// Read a resource at the given URI.
    async fn read(
        &self,
        uri: &str,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::ReadResourceResult>;

    /// List available resources.
    async fn list(
        &self,
        _cursor: Option<String>,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::ListResourcesResult>;
}

/// Handler for message sampling (LLM operations).
#[async_trait]
pub trait SamplingHandler: Send + Sync {
    /// Create a message using the language model.
    async fn create_message(
        &self,
        params: crate::types::CreateMessageParams,
        extra: cancellation::RequestHandlerExtra,
    ) -> Result<crate::types::CreateMessageResult>;
}

/// MCP server implementation.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::{Server, ServerCapabilities, ToolHandler};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct MyTool;
///
/// #[async_trait]
/// impl ToolHandler for MyTool {
///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
///         Ok(serde_json::json!({"result": "success"}))
///     }
/// }
///
/// # async fn example() -> pmcp::Result<()> {
/// let server = Server::builder()
///     .name("my-server")
///     .version("1.0.0")
///     .tool("my-tool", MyTool)
///     .build()?;
///
/// server.run_stdio().await?;
/// # Ok(())
/// # }
/// ```
#[allow(dead_code)]
pub struct Server {
    info: Implementation,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    prompts: HashMap<String, Arc<dyn PromptHandler>>,
    resources: Option<Arc<dyn ResourceHandler>>,
    sampling: Option<Arc<dyn SamplingHandler>>,
    client_capabilities: Arc<RwLock<Option<ClientCapabilities>>>,
    initialized: Arc<RwLock<bool>>,
    /// Channel for sending notifications
    notification_tx: Option<mpsc::Sender<Notification>>,
    /// Cancellation manager for request cancellation
    cancellation_manager: cancellation::CancellationManager,
    /// Roots manager for directory/URI registration
    roots_manager: Arc<RwLock<roots::RootsManager>>,
    /// Subscription manager for resource subscriptions
    subscription_manager: Arc<RwLock<subscriptions::SubscriptionManager>>,
    /// Elicitation manager for user input requests
    elicitation_manager: Option<Arc<elicitation::ElicitationManager>>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("info", &self.info)
            .field("capabilities", &self.capabilities)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .field("sampling", &self.sampling.is_some())
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl Server {
    /// Send a notification.
    ///
    /// Sends a notification to the connected client. Notifications are one-way
    /// messages that don't expect a response.
    ///
    /// # Arguments
    ///
    /// * `notification` - The server notification to send
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ServerNotification, ProgressNotification, ProgressToken};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Send a progress notification
    /// let progress = ProgressNotification {
    ///     progress_token: ProgressToken::String("task-123".to_string()),
    ///     progress: 50.0,
    ///     message: Some("Processing...".to_string()),
    /// };
    ///
    /// server.send_notification(ServerNotification::Progress(progress)).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_notification(&self, notification: ServerNotification) {
        if let Some(tx) = &self.notification_tx {
            let _ = tx.send(Notification::Server(notification)).await;
        }
    }

    /// Get client capabilities.
    ///
    /// Returns the capabilities that the client declared during initialization.
    /// This can be used to check if the client supports specific features.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Check client capabilities after initialization
    /// if let Some(capabilities) = server.get_client_capabilities().await {
    ///     if capabilities.tools.is_some() {
    ///         println!("Client supports tools");
    ///     }
    ///     if capabilities.resources.is_some() {
    ///         println!("Client supports resources");
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Returns
    ///
    /// - `Some(ClientCapabilities)` if the client has been initialized
    /// - `None` if the client hasn't initialized yet
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Check if the server is initialized.
    ///
    /// Returns true if the initialization handshake with a client has completed.
    /// The server must be initialized before it can process most requests.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("example-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// if server.is_initialized().await {
    ///     println!("Server is ready to handle requests");
    /// } else {
    ///     println!("Waiting for client initialization");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
    /// Create a new server builder.
    ///
    /// Returns a `ServerBuilder` for configuring and constructing a new MCP server.
    /// The builder pattern allows you to set server information, capabilities,
    /// and register handlers before building the final server instance.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct HelloTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for HelloTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         Ok(serde_json::json!({"message": "Hello, World!"}))
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("greeting-server")
    ///     .version("1.0.0")
    ///     .tool("hello", HelloTool{})
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Run the server with stdio transport.
    ///
    /// Starts the server using stdin/stdout for communication.
    /// This is the standard way to run MCP servers as they communicate
    /// via JSON-RPC over stdio.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct EchoTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for EchoTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         Ok(args) // Echo the input
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("echo-server")
    ///     .version("1.0.0")
    ///     .tool("echo", EchoTool{})
    ///     .build()?;
    ///
    /// // This will run indefinitely, handling client requests
    /// server.run_stdio().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The stdio transport fails to initialize
    /// - Communication with the client fails
    /// - The server encounters an unrecoverable error
    pub async fn run_stdio(self) -> Result<()> {
        let transport = crate::shared::StdioTransport::new();
        self.run(transport).await
    }

    /// Run the server with a custom transport.
    ///
    /// Starts the server using a custom transport implementation.
    /// This allows for different communication mechanisms beyond stdio,
    /// such as TCP sockets, `WebSockets`, or other protocols.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport implementation to use for communication
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, StdioTransport, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct CalculatorTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for CalculatorTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         let a = args["a"].as_f64().unwrap_or(0.0);
    ///         let b = args["b"].as_f64().unwrap_or(0.0);
    ///         Ok(serde_json::json!({"result": a + b}))
    ///     }
    /// }
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("calculator-server")
    ///     .version("1.0.0")
    ///     .tool("add", CalculatorTool{})
    ///     .build()?;
    ///
    /// let transport = StdioTransport::new();
    /// server.run(transport).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The transport fails to initialize or operate
    /// - Communication with the client fails
    /// - The server encounters an unrecoverable error
    pub async fn run<T: crate::shared::Transport + 'static>(mut self, transport: T) -> Result<()> {
        let (notification_tx, notification_rx) = mpsc::channel(100);
        self.notification_tx = Some(notification_tx);

        let server = Arc::new(self);
        let transport = Arc::new(RwLock::new(transport));
        let protocol = Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default())));

        Self::spawn_notification_handler(transport.clone(), notification_rx);
        Self::spawn_message_handler(server.clone(), transport.clone(), protocol);

        // Keep the main task alive
        Self::run_main_loop().await
    }

    /// Spawn task to handle outgoing notifications.
    fn spawn_notification_handler(
        transport: Arc<RwLock<impl crate::shared::Transport + 'static>>,
        mut notification_rx: mpsc::Receiver<Notification>,
    ) {
        tokio::spawn(async move {
            while let Some(notification) = notification_rx.recv().await {
                if let Err(e) =
                    Self::send_notification_through_transport(&transport, notification).await
                {
                    Self::log_error(&format!("Failed to send notification: {}", e)).await;
                }
            }
        });
    }

    /// Spawn task to handle incoming messages.
    fn spawn_message_handler(
        server: Arc<Self>,
        transport: Arc<RwLock<impl crate::shared::Transport + 'static>>,
        _protocol: Arc<RwLock<Protocol>>,
    ) {
        tokio::spawn(async move {
            loop {
                let message = match Self::receive_message_from_transport(&transport).await {
                    Ok(msg) => msg,
                    Err(e) => {
                        Self::log_error(&format!("Transport receive error: {}", e)).await;
                        break;
                    },
                };

                if let Err(e) = Self::handle_transport_message(&server, &transport, message).await {
                    Self::log_error(&format!("Message handling error: {}", e)).await;
                    break;
                }
            }
        });
    }

    /// Send a notification through the transport.
    async fn send_notification_through_transport(
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
        notification: Notification,
    ) -> Result<()> {
        let mut t = transport.write().await;
        t.send(TransportMessage::Notification(notification)).await
    }

    /// Receive a message from the transport.
    async fn receive_message_from_transport(
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
    ) -> Result<TransportMessage> {
        let mut t = transport.write().await;
        t.receive().await
    }

    /// Handle a transport message.
    async fn handle_transport_message(
        server: &Arc<Self>,
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
        message: TransportMessage,
    ) -> Result<()> {
        match message {
            TransportMessage::Request { id, request } => {
                Self::handle_request_message(server, transport, id, request).await
            },
            TransportMessage::Response(_) => {
                Self::log_warning("Server received unexpected response message").await;
                Ok(())
            },
            TransportMessage::Notification(_) => {
                Self::log_debug("Server received notification").await;
                Ok(())
            },
        }
    }

    /// Handle a request message.
    async fn handle_request_message(
        server: &Arc<Self>,
        transport: &Arc<RwLock<impl crate::shared::Transport>>,
        id: RequestId,
        request: Request,
    ) -> Result<()> {
        let response = server.handle_request(id, request).await;
        let mut t = transport.write().await;
        t.send(TransportMessage::Response(response)).await
    }

    /// Log an error message.
    async fn log_error(message: &str) {
        crate::log(crate::types::protocol::LogLevel::Error, message, None).await;
    }

    /// Log a warning message.
    async fn log_warning(message: &str) {
        crate::log(crate::types::protocol::LogLevel::Warning, message, None).await;
    }

    /// Log a debug message.
    async fn log_debug(message: &str) {
        crate::log(crate::types::protocol::LogLevel::Debug, message, None).await;
    }

    /// Run the main event loop.
    async fn run_main_loop() -> Result<()> {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse {
        match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };
                // Store client capabilities
                *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
                *self.initialized.write().await = true;

                let result = InitializeResult {
                    protocol_version: ProtocolVersion("2024-11-05".to_string()),
                    capabilities: self.capabilities.clone(),
                    server_info: self.info.clone(),
                    instructions: None,
                };
                JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.clone(),
                    payload: crate::types::jsonrpc::ResponsePayload::Result(
                        serde_json::to_value(result).unwrap(),
                    ),
                }
            },
            Request::Client(boxed_req) => self.handle_client_request(id, *boxed_req).await,
            Request::Server(_) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(
                    crate::types::jsonrpc::JSONRPCError {
                        code: -32601,
                        message: "Server requests not supported by server".to_string(),
                        data: None,
                    },
                ),
            },
        }
    }

    async fn handle_client_request(
        &self,
        id: RequestId,
        request: ClientRequest,
    ) -> JSONRPCResponse {
        let result = self.process_client_request(id.clone(), request).await;
        Self::create_response(id, result)
    }

    /// Process a client request and return the result.
    async fn process_client_request(
        &self,
        request_id: RequestId,
        request: ClientRequest,
    ) -> Result<serde_json::Value> {
        match request {
            ClientRequest::Initialize(_) => {
                // Already handled above
                unreachable!("Initialize should be handled separately")
            },
            ClientRequest::ListTools(req) => self.handle_list_tools(req),
            ClientRequest::CallTool(req) => self.handle_call_tool(request_id, req).await,
            ClientRequest::ListPrompts(req) => self.handle_list_prompts(req),
            ClientRequest::GetPrompt(req) => self.handle_get_prompt(request_id, req).await,
            ClientRequest::ListResources(req) => self.handle_list_resources(request_id, req).await,
            ClientRequest::ReadResource(req) => self.handle_read_resource(request_id, req).await,
            ClientRequest::ListResourceTemplates(req) => {
                Self::handle_list_resource_templates(self, req)
            },
            ClientRequest::Subscribe(_)
            | ClientRequest::Unsubscribe(_)
            | ClientRequest::Complete(_)
            | ClientRequest::SetLoggingLevel { level: _ }
            | ClientRequest::Ping => Ok(serde_json::json!({})),
            ClientRequest::CreateMessage(req) => self.handle_create_message(request_id, req).await,
            ClientRequest::ElicitInputResponse(response) => {
                // Handle elicitation response if we have a manager
                if let Some(elicitation_manager) = &self.elicitation_manager {
                    elicitation_manager.handle_response(response).await?;
                }
                Ok(serde_json::json!({}))
            },
        }
    }

    /// Create a JSON-RPC response from a result.
    fn create_response(id: RequestId, result: Result<serde_json::Value>) -> JSONRPCResponse {
        match result {
            Ok(value) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Result(value),
            },
            Err(e) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: crate::types::jsonrpc::ResponsePayload::Error(
                    crate::types::jsonrpc::JSONRPCError {
                        code: -32603,
                        message: e.to_string(),
                        data: None,
                    },
                ),
            },
        }
    }

    fn handle_list_tools(&self, _req: ListToolsRequest) -> Result<Value> {
        let tools = self
            .tools
            .keys()
            .map(|name| crate::types::ToolInfo {
                name: name.clone(),
                description: None,
                input_schema: serde_json::json!({}),
            })
            .collect::<Vec<_>>();

        Ok(serde_json::to_value(ListToolsResult {
            tools,
            next_cursor: None,
        })?)
    }

    async fn handle_call_tool(&self, request_id: RequestId, req: CallToolRequest) -> Result<Value> {
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Tool '{}' not found", req.name)))?;

        let cancellation_token = self
            .cancellation_manager
            .get_token(&request_id.to_string())
            .await
            .unwrap_or_else(tokio_util::sync::CancellationToken::new);
        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id.to_string(),
            cancellation_token,
        );
        let result = handler.handle(req.arguments, extra).await?;
        Ok(serde_json::to_value(CallToolResult {
            content: vec![crate::types::Content::Text {
                text: result.to_string(),
            }],
            is_error: false,
        })?)
    }

    fn handle_list_prompts(&self, _req: ListPromptsRequest) -> Result<Value> {
        let prompts = self
            .prompts
            .keys()
            .map(|name| crate::types::PromptInfo {
                name: name.clone(),
                description: None,
                arguments: None,
            })
            .collect::<Vec<_>>();

        Ok(serde_json::to_value(ListPromptsResult {
            prompts,
            next_cursor: None,
        })?)
    }

    async fn handle_get_prompt(
        &self,
        request_id: RequestId,
        req: GetPromptRequest,
    ) -> Result<Value> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Prompt '{}' not found", req.name)))?;

        let cancellation_token = self
            .cancellation_manager
            .get_token(&request_id.to_string())
            .await
            .unwrap_or_else(tokio_util::sync::CancellationToken::new);
        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id.to_string(),
            cancellation_token,
        );
        let result = handler.handle(req.arguments, extra).await?;
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_list_resources(
        &self,
        request_id: RequestId,
        req: ListResourcesRequest,
    ) -> Result<Value> {
        if let Some(handler) = &self.resources {
            let cancellation_token = self
                .cancellation_manager
                .get_token(&request_id.to_string())
                .await
                .unwrap_or_else(tokio_util::sync::CancellationToken::new);
            let extra = crate::server::cancellation::RequestHandlerExtra::new(
                request_id.to_string(),
                cancellation_token,
            );
            let result = handler.list(req.cursor, extra).await?;
            Ok(serde_json::to_value(result)?)
        } else {
            Ok(serde_json::to_value(ListResourcesResult {
                resources: vec![],
                next_cursor: None,
            })?)
        }
    }

    async fn handle_read_resource(
        &self,
        request_id: RequestId,
        req: ReadResourceRequest,
    ) -> Result<Value> {
        let handler = self
            .resources
            .as_ref()
            .ok_or_else(|| Error::not_found("No resource handler configured".to_string()))?;

        let cancellation_token = self
            .cancellation_manager
            .get_token(&request_id.to_string())
            .await
            .unwrap_or_else(tokio_util::sync::CancellationToken::new);
        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id.to_string(),
            cancellation_token,
        );
        let result = handler.read(&req.uri, extra).await?;
        Ok(serde_json::to_value(result)?)
    }

    #[allow(clippy::unused_self)]
    fn handle_list_resource_templates(&self, _req: ListResourceTemplatesRequest) -> Result<Value> {
        Ok(serde_json::to_value(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })?)
    }

    async fn handle_create_message(
        &self,
        request_id: RequestId,
        req: crate::types::CreateMessageRequest,
    ) -> Result<Value> {
        let handler = self
            .sampling
            .as_ref()
            .ok_or_else(|| Error::not_found("No sampling handler configured".to_string()))?;

        let cancellation_token = self
            .cancellation_manager
            .get_token(&request_id.to_string())
            .await
            .unwrap_or_else(tokio_util::sync::CancellationToken::new);
        let extra = crate::server::cancellation::RequestHandlerExtra::new(
            request_id.to_string(),
            cancellation_token,
        );
        let result = handler.create_message(req, extra).await?;
        Ok(serde_json::to_value(result)?)
    }

    /// Register a root directory or URI that the server has access to.
    ///
    /// This method allows the server to announce to clients that it has
    /// access to specific file system roots or URIs. This is useful for
    /// resource handlers that need to expose filesystem access or other
    /// URI-based resources.
    ///
    /// # Arguments
    ///
    /// * `uri` - The root URI to register (e.g., `file:///home/user/project`)
    /// * `name` - Optional human-readable name for the root
    ///
    /// # Returns
    ///
    /// An unregister function that can be called to remove the root registration.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Register a project root
    /// let unregister = server.register_root(
    ///     "file:///home/user/project",
    ///     Some("My Project".to_string())
    /// ).await?;
    ///
    /// // Later, unregister the root
    /// unregister();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_root(
        &self,
        uri: impl Into<String>,
        name: Option<String>,
    ) -> Result<impl FnOnce() + Send + 'static> {
        let mut roots_manager = self.roots_manager.write().await;
        if let Some(tx) = &self.notification_tx {
            roots_manager.set_notification_sender({
                let tx = tx.clone();
                move |server_notification| {
                    let _ = tx.try_send(Notification::Server(server_notification));
                }
            });
        }
        roots_manager.register_root(uri.into(), name).await
    }

    /// Get the list of registered roots.
    ///
    /// Returns a list of all currently registered root URIs and their
    /// associated names. Roots are directories or URIs that the server
    /// has announced access to.
    ///
    /// # Returns
    ///
    /// A vector of `Root` objects containing URI and optional name.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Register some roots
    /// server.register_root("file:///home/user/project1", Some("Project 1".to_string())).await?;
    /// server.register_root("file:///home/user/project2", None).await?;
    ///
    /// // Get the list of roots
    /// let roots = server.get_roots().await;
    /// println!("Registered {} roots", roots.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_roots(&self) -> Vec<roots::Root> {
        let roots_manager = self.roots_manager.read().await;
        roots_manager.get_roots().await
    }

    /// Subscribe a client to resource updates.
    ///
    /// This method allows the server to track which clients are interested
    /// in updates to specific resources. When a resource changes, the server
    /// can notify all subscribed clients.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to subscribe to
    /// * `client_id` - Identifier for the subscribing client
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Subscribe client to resource updates
    /// server.subscribe_resource(
    ///     "file:///project/file.txt".to_string(),
    ///     "client-123".to_string()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn subscribe_resource(&self, uri: String, client_id: String) -> Result<()> {
        if uri.is_empty() || client_id.is_empty() {
            return Err(Error::invalid_params("URI and client_id must not be empty"));
        }

        let mut subscription_manager = self.subscription_manager.write().await;
        if let Some(tx) = &self.notification_tx {
            subscription_manager.set_notification_sender({
                let tx = tx.clone();
                move |notification| {
                    let _ = tx.try_send(Notification::Server(notification));
                }
            });
        }

        subscription_manager.subscribe(uri, client_id).await
    }

    /// Cancel a request that is currently being processed.
    ///
    /// This method allows the server to cancel ongoing requests, which is
    /// useful for implementing request timeouts or client-requested cancellations.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to cancel
    /// * `reason` - Optional reason for cancellation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("cancel-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Cancel a request
    /// server.cancel_request(
    ///     "request-123".to_string(),
    ///     Some("User requested cancellation".to_string())
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cancel_request(&self, request_id: String, reason: Option<String>) -> Result<()> {
        if request_id.is_empty() {
            return Err(Error::invalid_params("Request ID must not be empty"));
        }

        self.cancellation_manager
            .cancel_request(request_id, reason)
            .await
    }

    /// Unsubscribe a client from resource updates.
    ///
    /// This method removes a client's subscription to a specific resource,
    /// so they will no longer receive notifications when that resource changes.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to unsubscribe from
    /// * `client_id` - Identifier for the client to unsubscribe
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .build()?;
    ///
    /// // Unsubscribe client from resource updates
    /// server.unsubscribe_resource(
    ///     "file:///project/file.txt".to_string(),
    ///     "client-123".to_string()
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unsubscribe_resource(&self, uri: String, client_id: String) -> Result<()> {
        if uri.is_empty() || client_id.is_empty() {
            return Err(Error::invalid_params("URI and client_id must not be empty"));
        }

        let subscription_manager = self.subscription_manager.read().await;
        subscription_manager.unsubscribe(uri, client_id).await
    }

    /// Notify subscribers that a resource has been updated.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource that was updated
    ///
    /// # Returns
    ///
    /// The number of subscribers that were notified.
    pub async fn notify_resource_updated(&self, uri: String) -> Result<usize> {
        let mut subscription_manager = self.subscription_manager.write().await;
        if let Some(tx) = &self.notification_tx {
            subscription_manager.set_notification_sender({
                let tx = tx.clone();
                move |notification| {
                    let _ = tx.try_send(Notification::Server(notification));
                }
            });
        }
        subscription_manager.notify_resource_updated(uri).await
    }
}

/// Builder for creating servers.
pub struct ServerBuilder {
    name: Option<String>,
    version: Option<String>,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    prompts: HashMap<String, Arc<dyn PromptHandler>>,
    resources: Option<Arc<dyn ResourceHandler>>,
    sampling: Option<Arc<dyn SamplingHandler>>,
    /// Cancellation manager for request cancellation
    cancellation_manager: cancellation::CancellationManager,
    /// Roots manager for directory/URI registration
    roots_manager: roots::RootsManager,
}

impl std::fmt::Debug for ServerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerBuilder")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("capabilities", &self.capabilities)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .field("sampling", &self.sampling.is_some())
            .finish()
    }
}

impl ServerBuilder {
    /// Create a new server builder.
    ///
    /// Creates a new `ServerBuilder` with default capabilities and no handlers.
    /// Use the builder methods to configure the server before calling `build()`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::ServerBuilder;
    ///
    /// let builder = ServerBuilder::new();
    /// ```
    ///
    /// This is equivalent to using the default implementation:
    ///
    /// ```rust,no_run
    /// use pmcp::ServerBuilder;
    ///
    /// let builder = ServerBuilder::default();
    /// ```
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            capabilities: ServerCapabilities::default(),
            tools: HashMap::new(),
            prompts: HashMap::new(),
            resources: None,
            sampling: None,
            cancellation_manager: cancellation::CancellationManager::new(),
            roots_manager: roots::RootsManager::new(),
        }
    }

    /// Set the server name.
    ///
    /// The server name identifies this MCP server implementation.
    /// This is required and will be sent to clients during initialization.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the server
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// let server = Server::builder()
    ///     .name("file-manager")
    ///     .version("1.0.0")
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the server version.
    ///
    /// The server version identifies this specific version of the MCP server.
    /// This is required and will be sent to clients during initialization.
    ///
    /// # Arguments
    ///
    /// * `version` - The version string (e.g., "1.0.0", "2.1.3-beta")
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::Server;
    ///
    /// let server = Server::builder()
    ///     .name("data-processor")
    ///     .version("2.1.0")
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set server capabilities.
    ///
    /// Configures the capabilities that this server supports.
    /// Capabilities inform clients about which MCP features are available.
    ///
    /// # Arguments
    ///
    /// * `capabilities` - The server capabilities to advertise
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ServerCapabilities, ToolCapabilities};
    ///
    /// let capabilities = ServerCapabilities {
    ///     tools: Some(ToolCapabilities {
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
    ///
    /// let server = Server::builder()
    ///     .name("advanced-server")
    ///     .version("1.0.0")
    ///     .capabilities(capabilities)
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn capabilities(mut self, capabilities: ServerCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Add a tool handler.
    ///
    /// Registers a tool that clients can call via the tools/call method.
    /// Tools are the primary way servers provide functionality to clients.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool (used by clients to call it)
    /// * `handler` - The handler implementation for this tool
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct FileListTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for FileListTool {
    ///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         let path = args["path"].as_str().unwrap_or(".");
    ///         // List files in path...
    ///         Ok(serde_json::json!({"files": ["file1.txt", "file2.txt"]}))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .tool("list_files", FileListTool{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn tool(mut self, name: impl Into<String>, handler: impl ToolHandler + 'static) -> Self {
        self.tools.insert(name.into(), Arc::new(handler));
        self
    }

    /// Add a prompt handler.
    ///
    /// Registers a prompt that clients can retrieve via the prompts/get method.
    /// Prompts provide templates that clients can use for various tasks.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt (used by clients to retrieve it)
    /// * `handler` - The handler implementation for this prompt
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, PromptHandler, GetPromptResult, PromptMessage, MessageContent};
    /// use async_trait::async_trait;
    /// use std::collections::HashMap;
    ///
    /// struct CodeReviewPrompt;
    ///
    /// #[async_trait]
    /// impl PromptHandler for CodeReviewPrompt {
    ///     async fn handle(&self, args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
    ///         let language = args.get("language").map(|s| s.as_str()).unwrap_or("unknown");
    ///         Ok(GetPromptResult {
    ///             description: Some(format!("Code review prompt for {}", language)),
    ///             messages: vec![PromptMessage {
    ///                 role: pmcp::Role::User,
    ///                 content: pmcp::Content::Text {
    ///                     text: format!("Please review this {} code:", language),
    ///                 },
    ///             }],
    ///         })
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("code-server")
    ///     .version("1.0.0")
    ///     .prompt("code_review", CodeReviewPrompt{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn prompt(
        mut self,
        name: impl Into<String>,
        handler: impl PromptHandler + 'static,
    ) -> Self {
        self.prompts.insert(name.into(), Arc::new(handler));
        self
    }

    /// Set the resource handler.
    ///
    /// Registers a resource handler that provides access to server resources.
    /// Resources allow clients to read files, configurations, or other data.
    ///
    /// # Arguments
    ///
    /// * `handler` - The resource handler implementation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ResourceHandler, ReadResourceResult, ListResourcesResult, ResourceInfo};
    /// use async_trait::async_trait;
    ///
    /// struct FileResourceHandler;
    ///
    /// #[async_trait]
    /// impl ResourceHandler for FileResourceHandler {
    ///     async fn read(&self, uri: &str, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<ReadResourceResult> {
    ///         // Read file content...
    ///         Ok(ReadResourceResult {
    ///             contents: vec![pmcp::Content::Text {
    ///                 text: "File content here".to_string(),
    ///             }],
    ///         })
    ///     }
    ///
    ///     async fn list(&self, _cursor: Option<String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<ListResourcesResult> {
    ///         Ok(ListResourcesResult {
    ///             resources: vec![pmcp::ResourceInfo {
    ///                 uri: "file://example.txt".to_string(),
    ///                 name: "example.txt".to_string(),
    ///                 description: Some("Example file".to_string()),
    ///                 mime_type: Some("text/plain".to_string()),
    ///             }],
    ///             next_cursor: None,
    ///         })
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("file-server")
    ///     .version("1.0.0")
    ///     .resources(FileResourceHandler{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn resources(mut self, handler: impl ResourceHandler + 'static) -> Self {
        self.resources = Some(Arc::new(handler));
        self
    }

    /// Set the sampling handler.
    ///
    /// Registers a sampling handler that provides LLM functionality.
    /// This allows the server to act as a language model provider.
    ///
    /// # Arguments
    ///
    /// * `handler` - The sampling handler implementation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, SamplingHandler, CreateMessageParams, CreateMessageResult};
    /// use async_trait::async_trait;
    ///
    /// struct MockLLM;
    ///
    /// #[async_trait]
    /// impl SamplingHandler for MockLLM {
    ///     async fn create_message(&self, params: CreateMessageParams, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<CreateMessageResult> {
    ///         // Process the messages and generate a response
    ///         Ok(CreateMessageResult {
    ///             content: pmcp::MessageContent::Text {
    ///                 text: "Generated response".to_string(),
    ///             },
    ///             model: "mock-llm-v1".to_string(),
    ///             usage: Some(pmcp::TokenUsage {
    ///                 input_tokens: 10,
    ///                 output_tokens: 5,
    ///                 total_tokens: 15,
    ///             }),
    ///             stop_reason: Some("end_of_text".to_string()),
    ///         })
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("llm-server")
    ///     .version("1.0.0")
    ///     .sampling(MockLLM{})
    ///     .build()?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    pub fn sampling(mut self, handler: impl SamplingHandler + 'static) -> Self {
        self.sampling = Some(Arc::new(handler));
        // Enable sampling capability
        self.capabilities.sampling = Some(crate::types::SamplingCapabilities::default());
        self
    }

    /// Build the server.
    ///
    /// Constructs the final Server instance from the configured builder.
    /// This validates that required fields (name and version) are set.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Server, ToolHandler};
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    ///
    /// struct PingTool;
    ///
    /// #[async_trait]
    /// impl ToolHandler for PingTool {
    ///     async fn handle(&self, _args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
    ///         Ok(serde_json::json!({"response": "pong"}))
    ///     }
    /// }
    ///
    /// let server = Server::builder()
    ///     .name("ping-server")
    ///     .version("1.0.0")
    ///     .tool("ping", PingTool{})
    ///     .build()?;
    ///
    /// // Server is now ready to run
    /// // server.run_stdio().await?;
    /// # Ok::<(), pmcp::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The server name is not set
    /// - The server version is not set
    pub fn build(self) -> Result<Server> {
        let name = self
            .name
            .ok_or_else(|| crate::Error::validation("Server name is required"))?;
        let version = self
            .version
            .ok_or_else(|| crate::Error::validation("Server version is required"))?;

        Ok(Server {
            info: Implementation { name, version },
            capabilities: self.capabilities,
            tools: self.tools,
            prompts: self.prompts,
            resources: self.resources,
            sampling: self.sampling,
            client_capabilities: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            notification_tx: None,
            cancellation_manager: self.cancellation_manager,
            roots_manager: Arc::new(RwLock::new(self.roots_manager)),
            subscription_manager: Arc::new(RwLock::new(subscriptions::SubscriptionManager::new())),
            elicitation_manager: None,
        })
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::ResponsePayload, ClientCapabilities, InitializeRequest, ServerCapabilities,
        ToolCapabilities, TransportMessage,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::time::timeout;

    /// Mock transport for testing
    #[derive(Debug)]
    struct MockTransport {
        messages: Arc<Mutex<Vec<TransportMessage>>>,
        responses: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_requests(requests: Vec<TransportMessage>) -> Self {
            Self {
                messages: Arc::new(Mutex::new(requests)),
                responses: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_request(&self, request: TransportMessage) {
            self.messages.lock().unwrap().push(request);
        }

        #[allow(dead_code)]
        fn get_sent_responses(&self) -> Vec<TransportMessage> {
            self.responses.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.responses.lock().unwrap().push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            let mut messages = self.messages.lock().unwrap();
            messages
                .pop()
                .map_or_else(|| Err(Error::protocol_msg("No more messages")), Ok)
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }

        fn is_connected(&self) -> bool {
            !self.messages.lock().unwrap().is_empty()
        }

        fn transport_type(&self) -> &'static str {
            "mock"
        }
    }

    /// Mock tool handler for testing
    struct MockTool {
        result: Value,
    }

    impl MockTool {
        fn new(result: Value) -> Self {
            Self { result }
        }
    }

    #[async_trait]
    impl ToolHandler for MockTool {
        async fn handle(
            &self,
            _args: Value,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<Value> {
            Ok(self.result.clone())
        }
    }

    /// Mock prompt handler for testing
    struct MockPrompt {
        result: crate::types::GetPromptResult,
    }

    impl MockPrompt {
        fn new(result: crate::types::GetPromptResult) -> Self {
            Self { result }
        }
    }

    #[async_trait]
    impl PromptHandler for MockPrompt {
        async fn handle(
            &self,
            _args: HashMap<String, String>,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::GetPromptResult> {
            Ok(self.result.clone())
        }
    }

    /// Mock resource handler for testing
    struct MockResource {
        resources: Vec<crate::types::ResourceInfo>,
        contents: HashMap<String, crate::types::ReadResourceResult>,
    }

    impl MockResource {
        fn new() -> Self {
            Self {
                resources: Vec::new(),
                contents: HashMap::new(),
            }
        }

        fn with_resource(mut self, uri: String, content: crate::types::ReadResourceResult) -> Self {
            self.contents.insert(uri, content);
            self
        }
    }

    #[async_trait]
    impl ResourceHandler for MockResource {
        async fn read(
            &self,
            uri: &str,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::ReadResourceResult> {
            self.contents
                .get(uri)
                .cloned()
                .ok_or_else(|| Error::not_found(format!("Resource '{}' not found", uri)))
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: crate::server::cancellation::RequestHandlerExtra,
        ) -> Result<crate::types::ListResourcesResult> {
            Ok(crate::types::ListResourcesResult {
                resources: self.resources.clone(),
                next_cursor: None,
            })
        }
    }

    #[test]
    fn test_server_builder() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .build()
            .unwrap();

        assert_eq!(server.info.name, "test-server");
        assert_eq!(server.info.version, "1.0.0");
        assert!(server.tools.contains_key("test-tool"));
    }

    #[test]
    fn test_server_builder_validation() {
        // Missing name
        let result = Server::builder().version("1.0.0").build();
        assert!(result.is_err());

        // Missing version
        let result = Server::builder().name("test-server").build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let init_request = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ClientCapabilities {
                    tools: Some(ToolCapabilities::default()),
                    ..Default::default()
                },
                client_info: Implementation {
                    name: "test-client".to_string(),
                    version: "1.0.0".to_string(),
                },
            }))),
        };

        let transport = MockTransport::with_requests(vec![init_request]);
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .build()
            .unwrap();

        // Test server run for a short time
        let server_handle = tokio::spawn(async move {
            let _ = timeout(std::time::Duration::from_millis(100), server.run(transport)).await;
        });

        // Wait for server to process
        let _ = timeout(std::time::Duration::from_millis(200), server_handle).await;
    }

    #[tokio::test]
    async fn test_server_capabilities() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .build()
            .unwrap();

        assert!(!server.is_initialized().await);
        assert!(server.get_client_capabilities().await.is_none());
    }

    #[tokio::test]
    async fn test_server_notifications() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        // Send notification (should not panic even without transport)
        server
            .send_notification(ServerNotification::ToolsChanged)
            .await;
    }

    #[test]
    fn test_server_builder_with_all_handlers() {
        let prompt_result = crate::types::GetPromptResult {
            description: Some("Test prompt".to_string()),
            messages: vec![],
        };

        let resource_content = crate::types::ReadResourceResult {
            contents: vec![crate::types::Content::Text {
                text: "Hello, world!".to_string(),
            }],
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .prompt("test-prompt", MockPrompt::new(prompt_result))
            .resources(
                MockResource::new().with_resource("test://uri".to_string(), resource_content),
            )
            .build()
            .unwrap();

        assert!(server.tools.contains_key("test-tool"));
        assert!(server.prompts.contains_key("test-prompt"));
        assert!(server.resources.is_some());
    }

    #[tokio::test]
    async fn test_handle_request_initialize() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .capabilities(ServerCapabilities::tools_only())
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        })));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        assert_eq!(response.id, RequestId::from(1i64));
        match response.payload {
            ResponsePayload::Result(_) => {
                assert!(server.is_initialized().await);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_list_tools() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor: None,
        })));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_call_tool() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .tool("test-tool", MockTool::new(json!({"result": "success"})))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "test-tool".to_string(),
            arguments: json!({"input": "test"}),
        })));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(!call_result.is_error);
                assert_eq!(call_result.content.len(), 1);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_call_tool_not_found() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "nonexistent-tool".to_string(),
            arguments: json!({}),
        })));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            },
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_list_prompts() {
        let prompt_result = crate::types::GetPromptResult {
            description: Some("Test prompt".to_string()),
            messages: vec![],
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .prompt("test-prompt", MockPrompt::new(prompt_result))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
            cursor: None,
        })));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let list_result: ListPromptsResult = serde_json::from_value(result).unwrap();
                assert_eq!(list_result.prompts.len(), 1);
                assert_eq!(list_result.prompts[0].name, "test-prompt");
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_get_prompt() {
        let prompt_result = crate::types::GetPromptResult {
            description: Some("Test prompt".to_string()),
            messages: vec![],
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .prompt("test-prompt", MockPrompt::new(prompt_result.clone()))
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name: "test-prompt".to_string(),
            arguments: HashMap::new(),
        })));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let get_result: crate::types::GetPromptResult =
                    serde_json::from_value(result).unwrap();
                assert_eq!(get_result.description, prompt_result.description);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_list_resources() {
        let resource_content = crate::types::ReadResourceResult {
            contents: vec![crate::types::Content::Text {
                text: "Hello, world!".to_string(),
            }],
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .resources(
                MockResource::new().with_resource("test://uri".to_string(), resource_content),
            )
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor: None },
        )));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let resources_result: ListResourcesResult = serde_json::from_value(result).unwrap();
                assert_eq!(resources_result.resources.len(), 0); // MockResource has empty list by default
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_read_resource() {
        let resource_content = crate::types::ReadResourceResult {
            contents: vec![crate::types::Content::Text {
                text: "Hello, world!".to_string(),
            }],
        };

        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .resources(
                MockResource::new()
                    .with_resource("test://uri".to_string(), resource_content.clone()),
            )
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "test://uri".to_string(),
        })));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let read_result: crate::types::ReadResourceResult =
                    serde_json::from_value(result).unwrap();
                assert_eq!(read_result.contents.len(), 1);
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_read_resource_not_found() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .resources(MockResource::new())
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "nonexistent://uri".to_string(),
        })));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            },
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let request = Request::Client(Box::new(ClientRequest::Ping));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(_) => {
                // Success
            },
            ResponsePayload::Error(_) => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_handle_server_request() {
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        let request = Request::Server(Box::new(crate::types::ServerRequest::CreateMessage(
            Box::new(crate::types::protocol::CreateMessageParams {
                messages: vec![],
                model_preferences: None,
                system_prompt: None,
                include_context: crate::types::protocol::IncludeContext::None,
                temperature: None,
                max_tokens: None,
                stop_sequences: None,
                metadata: None,
            }),
        )));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert_eq!(error.code, -32601);
                assert!(error.message.contains("not supported"));
            },
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }
}
