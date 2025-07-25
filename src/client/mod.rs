//! MCP client implementation.

use crate::error::{Error, Result};
use crate::shared::{Protocol, ProtocolOptions, Transport};
use crate::types::{
    CallToolRequest, CallToolResult, CancelledNotification, ClientCapabilities, ClientNotification,
    ClientRequest, CompleteRequest, CompleteResult, GetPromptRequest, GetPromptResult,
    Implementation, InitializeRequest, InitializeResult, ListPromptsRequest, ListPromptsResult,
    ListResourceTemplatesRequest, ListResourceTemplatesResult, ListResourcesRequest,
    ListResourcesResult, ListToolsRequest, ListToolsResult, LoggingLevel, Notification,
    ProgressNotification, ReadResourceRequest, ReadResourceResult, Request, RequestId,
    ServerCapabilities, SubscribeRequest, UnsubscribeRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use uuid::Uuid;

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
pub struct Client<T: Transport> {
    transport: Arc<RwLock<T>>,
    protocol: Arc<RwLock<Protocol>>,
    capabilities: Option<ClientCapabilities>,
    server_capabilities: Option<ServerCapabilities>,
    server_version: Option<Implementation>,
    instructions: Option<String>,
    initialized: bool,
    info: Implementation,
    /// Channel for handling incoming notifications
    notification_tx: Option<mpsc::Sender<Notification>>,
    /// Active request tracking for cancellation
    active_requests: Arc<RwLock<HashMap<RequestId, oneshot::Sender<()>>>>,
}

impl<T: Transport> std::fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("transport", &"<Arc<RwLock<Transport>>>")
            .field("protocol", &"<Arc<RwLock<Protocol>>>")
            .field("capabilities", &self.capabilities)
            .field("server_capabilities", &self.server_capabilities)
            .field("initialized", &self.initialized)
            .field("info", &self.info)
            .finish()
    }
}

impl<T: Transport> Client<T> {
    /// Create a new client with the given transport.
    pub fn new(transport: T) -> Self {
        Self::with_info(
            transport,
            Implementation {
                name: "pmcp-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        )
    }

    /// Create a new client with custom info.
    pub fn with_info(transport: T, client_info: Implementation) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new client with custom protocol options.
    pub fn with_options(
        transport: T,
        client_info: Implementation,
        options: ProtocolOptions,
    ) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(options))),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize the connection with the server.
    pub async fn initialize(
        &mut self,
        capabilities: ClientCapabilities,
    ) -> Result<InitializeResult> {
        if self.initialized {
            return Err(Error::InvalidState("Client already initialized".into()));
        }

        self.capabilities = Some(capabilities.clone());

        // Send initialize request
        let request = Request::Client(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::types::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            client_info: self.info.clone(),
        }));

        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        // Parse initialize result
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                if let Ok(init_result) = serde_json::from_value::<InitializeResult>(result) {
                    // Validate protocol version
                    if !crate::types::SUPPORTED_PROTOCOL_VERSIONS
                        .contains(&init_result.protocol_version.as_str())
                    {
                        return Err(Error::protocol_msg(format!(
                            "Server protocol version {} not supported",
                            init_result.protocol_version
                        )));
                    }

                    self.server_capabilities = Some(init_result.capabilities.clone());
                    self.server_version = Some(init_result.server_info.clone());
                    self.instructions.clone_from(&init_result.instructions);
                    self.initialized = true;

                    // Send initialized notification
                    self.send_notification(Notification::Client(ClientNotification::Initialized))
                        .await?;

                    Ok(init_result)
                } else {
                    Err(Error::parse("Invalid initialize result format"))
                }
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get server capabilities after initialization.
    pub fn get_server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    /// Get server version information after initialization.
    pub fn get_server_version(&self) -> Option<&Implementation> {
        self.server_version.as_ref()
    }

    /// Get server instructions after initialization.
    pub fn get_instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Send a ping to the server.
    pub async fn ping(&self) -> Result<()> {
        self.ensure_initialized()?;
        let request = Request::Client(ClientRequest::Ping);
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Set the logging level on the server.
    pub async fn set_logging_level(&self, level: LoggingLevel) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("logging", "logging/setLevel")?;

        let request = Request::Client(ClientRequest::SetLoggingLevel { level });
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available tools.
    pub async fn list_tools(&self, cursor: Option<String>) -> Result<ListToolsResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/list")?;

        let request = Request::Client(ClientRequest::ListTools(ListToolsRequest { cursor }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Call a tool.
    pub async fn call_tool(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(ClientRequest::CallTool(CallToolRequest { name, arguments }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available prompts.
    pub async fn list_prompts(&self, cursor: Option<String>) -> Result<ListPromptsResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/list")?;

        let request = Request::Client(ClientRequest::ListPrompts(ListPromptsRequest { cursor }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get a prompt.
    pub async fn get_prompt(
        &self,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<GetPromptResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/get")?;

        let request = Request::Client(ClientRequest::GetPrompt(GetPromptRequest {
            name,
            arguments,
        }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available resources.
    pub async fn list_resources(&self, cursor: Option<String>) -> Result<ListResourcesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/list")?;

        let request = Request::Client(ClientRequest::ListResources(ListResourcesRequest {
            cursor,
        }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List resource templates.
    pub async fn list_resource_templates(
        &self,
        cursor: Option<String>,
    ) -> Result<ListResourceTemplatesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/templates/list")?;

        let request = Request::Client(ClientRequest::ListResourceTemplates(
            ListResourceTemplatesRequest { cursor },
        ));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Read a resource.
    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/read")?;

        let request = Request::Client(ClientRequest::ReadResource(ReadResourceRequest { uri }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Subscribe to resource updates.
    pub async fn subscribe_resource(&self, uri: String) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/subscribe")?;

        // Check if server supports subscriptions
        if let Some(resources) = &self
            .server_capabilities
            .as_ref()
            .and_then(|c| c.resources.as_ref())
        {
            if !resources.subscribe.unwrap_or(false) {
                return Err(Error::capability(
                    "Server does not support resource subscriptions",
                ));
            }
        }

        let request = Request::Client(ClientRequest::Subscribe(SubscribeRequest { uri }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Unsubscribe from resource updates.
    pub async fn unsubscribe_resource(&self, uri: String) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/unsubscribe")?;

        let request = Request::Client(ClientRequest::Unsubscribe(UnsubscribeRequest { uri }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Request completion from the server.
    pub async fn complete(&self, params: CompleteRequest) -> Result<CompleteResult> {
        self.ensure_initialized()?;
        self.assert_capability("completions", "completion/complete")?;

        let request = Request::Client(ClientRequest::Complete(params));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Send roots list changed notification.
    pub async fn send_roots_list_changed(&self) -> Result<()> {
        self.ensure_initialized()?;
        if let Some(roots) = &self.capabilities.as_ref().and_then(|c| c.roots.as_ref()) {
            if roots.list_changed {
                // OK, we support it
            } else {
                return Err(Error::capability(
                    "Client does not support roots list changed notifications",
                ));
            }
        }

        self.send_notification(Notification::Client(ClientNotification::RootsListChanged))
            .await
    }

    /// Cancel a request.
    pub async fn cancel_request(&self, request_id: &RequestId) -> Result<()> {
        // Send cancellation notification
        self.send_notification(Notification::Cancelled(CancelledNotification {
            request_id: request_id.clone(),
            reason: Some("User requested cancellation".to_string()),
        }))
        .await?;

        // Cancel any local tracking
        let sender = self.active_requests.write().await.remove(request_id);
        if let Some(sender) = sender {
            let _ = sender.send(());
        }

        Ok(())
    }

    /// Send a progress notification.
    pub async fn send_progress(&self, progress: ProgressNotification) -> Result<()> {
        self.send_notification(Notification::Progress(progress))
            .await
    }

    /// Check if client is initialized.
    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(Error::InvalidState("Client not initialized".into()))
        }
    }

    /// Assert that the server has a specific capability.
    fn assert_capability(&self, capability: &str, method: &str) -> Result<()> {
        let has_capability = match capability {
            "tools" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.tools.is_some()),
            "prompts" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.prompts.is_some()),
            "resources" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.resources.is_some()),
            "logging" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.logging.is_some()),
            "completions" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.completions.is_some()),
            _ => false,
        };

        if has_capability {
            Ok(())
        } else {
            Err(Error::capability(format!(
                "Server does not support {} (required for {})",
                capability, method
            )))
        }
    }

    /// Send a request and wait for response.
    async fn send_request(
        &self,
        request_id: RequestId,
        request: Request,
    ) -> Result<crate::types::JSONRPCResponse> {
        // Track request for cancellation
        let (cancel_tx, _cancel_rx) = oneshot::channel();
        self.active_requests
            .write()
            .await
            .insert(request_id.clone(), cancel_tx);

        // Send request through transport
        let message = crate::types::TransportMessage::Request {
            id: request_id.clone(),
            request,
        };

        self.transport.write().await.send(message).await?;

        // Wait for response (this would be implemented with proper response routing)
        // For now, receive next message and assume it's our response
        let response_message = self.transport.write().await.receive().await?;

        // Remove from active requests
        self.active_requests.write().await.remove(&request_id);

        match response_message {
            crate::types::TransportMessage::Response(response) => Ok(response),
            _ => Err(Error::protocol_msg(
                "Expected response, got different message type",
            )),
        }
    }

    /// Send a notification.
    async fn send_notification(&self, notification: Notification) -> Result<()> {
        let message = crate::types::TransportMessage::Notification(notification);
        self.transport.write().await.send(message).await
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
        Client::with_options(
            self.transport,
            Implementation {
                name: "pmcp-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            self.options,
        )
    }
}

impl<T: Transport> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            protocol: self.protocol.clone(),
            capabilities: self.capabilities.clone(),
            server_capabilities: self.server_capabilities.clone(),
            server_version: self.server_version.clone(),
            instructions: self.instructions.clone(),
            initialized: self.initialized,
            info: self.info.clone(),
            notification_tx: self.notification_tx.clone(),
            active_requests: self.active_requests.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::{JSONRPCError, ResponsePayload},
        JSONRPCResponse, ProgressNotification,
        ProgressToken, TransportMessage,
        ToolCapabilities, ResourceCapabilities,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// Mock transport for testing
    #[derive(Debug)]
    struct MockTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_responses(responses: Vec<TransportMessage>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_response(&self, response: TransportMessage) {
            self.responses.lock().unwrap().push(response);
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.sent_messages.lock().unwrap().push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| Error::protocol_msg("No more responses"))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_client_creation() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        assert!(!client.initialized);
        assert_eq!(client.info.name, "pmcp-client");
    }

    #[test]
    fn test_client_with_info() {
        let transport = MockTransport::new();
        let info = Implementation {
            name: "test-client".to_string(),
            version: "1.0.0".to_string(),
        };
        let client = Client::with_info(transport, info);
        assert_eq!(client.info.name, "test-client");
        assert_eq!(client.info.version, "1.0.0");
    }

    #[test]
    fn test_client_builder() {
        let transport = MockTransport::new();
        let client = ClientBuilder::new(transport)
            .enforce_strict_capabilities(true)
            .debounced_notifications(vec!["test".to_string()])
            .build();
        assert!(tokio::runtime::Runtime::new().unwrap().block_on(client.protocol.read()).options().enforce_strict_capabilities);
    }

    #[tokio::test]
    async fn test_client_initialization() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        let caps = ClientCapabilities {
            tools: Some(ToolCapabilities { list_changed: Some(true) }),
            ..Default::default()
        };

        let result = client.initialize(caps).await;
        assert!(result.is_ok());
        assert!(client.initialized);
        assert_eq!(client.server_version.as_ref().unwrap().name, "test-server");
    }

    #[tokio::test]
    async fn test_ping() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let ping_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({})),
        });

        let transport = MockTransport::with_responses(vec![ping_response, init_response]);
        let mut client = Client::new(transport);
        
        // Initialize first
        let _ = client.initialize(ClientCapabilities::default()).await;
        
        // Ping
        let result = client.ping().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let tools_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "tools": [{
                    "name": "test-tool",
                    "description": "Test tool",
                    "inputSchema": {}
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![tools_response, init_response]);
        let mut client = Client::new(transport);
        
        // Initialize with tools capability
        let _ = client.initialize(ClientCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        }).await;
        
        // List tools
        let result = client.list_tools(None).await;
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "test-tool");
    }

    #[tokio::test]
    async fn test_error_response() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let error_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Error(JSONRPCError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        });

        let transport = MockTransport::with_responses(vec![error_response, init_response]);
        let mut client = Client::new(transport);
        
        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;
        
        // Try to list tools - should get error
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[tokio::test]
    async fn test_uninitialized_error() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        
        // Try to call method without initialization
        let result = client.ping().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_capability_enforcement() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    // No tools capability
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);
        
        // Initialize without tools capability
        let _ = client.initialize(ClientCapabilities::default()).await;
        
        // Try to list tools - should fail
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[tokio::test]
    async fn test_send_progress() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);
        
        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;
        
        // Send progress
        let progress = ProgressNotification {
            progress_token: ProgressToken::String("test".to_string()),
            progress: 50.0,
            message: Some("Halfway done".to_string()),
        };
        
        let result = client.send_progress(progress).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "completions": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let complete_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "completion": {
                    "values": ["test1", "test2"]
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![complete_response, init_response]);
        let mut client = Client::new(transport);
        
        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;
        
        // Complete
        let result = client.complete(CompleteRequest {
            r#ref: crate::types::CompletionReference::Resource {
                uri: "test://test".to_string(),
            },
            argument: crate::types::CompletionArgument {
                name: "test".to_string(),
                value: "t".to_string(),
            },
        }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_resource() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "resources": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let read_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "contents": [{
                    "type": "text",
                    "text": "Hello, world!"
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![read_response, init_response]);
        let mut client = Client::new(transport);
        
        // Initialize
        let _ = client.initialize(ClientCapabilities {
            resources: Some(ResourceCapabilities::default()),
            ..Default::default()
        }).await;
        
        // Read resource
        let result = client.read_resource("test://test".to_string()).await;
        if let Err(e) = &result {
            eprintln!("Read resource error: {:?}", e);
        }
        assert!(result.is_ok());
        let contents = result.unwrap();
        assert_eq!(contents.contents.len(), 1);
    }
}
