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
pub mod transport;

/// Handler for tool execution.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Handle a tool call with the given arguments.
    async fn handle(&self, args: Value) -> Result<Value>;
}

/// Handler for prompt generation.
#[async_trait]
pub trait PromptHandler: Send + Sync {
    /// Generate a prompt with the given arguments.
    async fn handle(&self, args: HashMap<String, String>) -> Result<crate::types::GetPromptResult>;
}

/// Handler for resource access.
#[async_trait]
pub trait ResourceHandler: Send + Sync {
    /// Read a resource at the given URI.
    async fn read(&self, uri: &str) -> Result<crate::types::ReadResourceResult>;

    /// List available resources.
    async fn list(&self, _cursor: Option<String>) -> Result<crate::types::ListResourcesResult>;
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
///     async fn handle(&self, args: Value) -> pmcp::Result<Value> {
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
    client_capabilities: Arc<RwLock<Option<ClientCapabilities>>>,
    initialized: Arc<RwLock<bool>>,
    /// Channel for sending notifications
    notification_tx: Option<mpsc::Sender<Notification>>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("info", &self.info)
            .field("capabilities", &self.capabilities)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl Server {
    /// Send a notification.
    pub async fn send_notification(&self, notification: ServerNotification) {
        if let Some(tx) = &self.notification_tx {
            let _ = tx.send(Notification::Server(notification)).await;
        }
    }

    /// Get client capabilities.
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Check if the server is initialized.
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
    /// Create a new server builder.
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Run the server with stdio transport.
    pub async fn run_stdio(self) -> Result<()> {
        let transport = crate::shared::StdioTransport::new();
        self.run(transport).await
    }

    /// Run the server with a custom transport.
    pub async fn run<T: crate::shared::Transport + 'static>(mut self, transport: T) -> Result<()> {
        // Set up notification channel
        let (notification_tx, mut notification_rx) = mpsc::channel(100);
        self.notification_tx = Some(notification_tx);

        let server = Arc::new(self);
        let transport = Arc::new(RwLock::new(transport));
        let protocol = Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default())));

        // Spawn task to send notifications
        let transport_clone = transport.clone();
        tokio::spawn(async move {
            while let Some(notification) = notification_rx.recv().await {
                let mut t = transport_clone.write().await;
                if let Err(e) = t.send(TransportMessage::Notification(notification)).await {
                    crate::log(
                        crate::types::protocol::LogLevel::Error,
                        &format!("Failed to send notification: {}", e),
                        None,
                    )
                    .await;
                }
            }
        });

        // Spawn task to handle incoming messages
        let server_clone = server.clone();
        let transport_clone = transport.clone();
        let _protocol = protocol;

        tokio::spawn(async move {
            loop {
                let message = {
                    let mut t = transport_clone.write().await;
                    match t.receive().await {
                        Ok(msg) => msg,
                        Err(e) => {
                            crate::log(
                                crate::types::protocol::LogLevel::Error,
                                &format!("Transport receive error: {}", e),
                                None,
                            )
                            .await;
                            break;
                        },
                    }
                };

                // Handle different message types
                match message {
                    TransportMessage::Request { id, request } => {
                        // Handle the request
                        let response = server_clone.handle_request(id, request).await;

                        // Send response
                        let mut t = transport_clone.write().await;
                        if let Err(e) = t.send(TransportMessage::Response(response)).await {
                            crate::log(
                                crate::types::protocol::LogLevel::Error,
                                &format!("Failed to send response: {}", e),
                                None,
                            )
                            .await;
                            break;
                        }
                    },
                    TransportMessage::Response(_) => {
                        crate::log(
                            crate::types::protocol::LogLevel::Warning,
                            "Server received unexpected response message",
                            None,
                        )
                        .await;
                    },
                    TransportMessage::Notification(_) => {
                        crate::log(
                            crate::types::protocol::LogLevel::Debug,
                            "Server received notification",
                            None,
                        )
                        .await;
                    },
                }
            }
        });

        // Keep the main task alive
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse {
        match request {
            Request::Client(ClientRequest::Initialize(init_req)) => {
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
            Request::Client(client_req) => self.handle_client_request(id, client_req).await,
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
        let result = match request {
            ClientRequest::Initialize(_) => {
                // Already handled above
                unreachable!("Initialize should be handled separately")
            },
            ClientRequest::ListTools(req) => self.handle_list_tools(req),
            ClientRequest::CallTool(req) => self.handle_call_tool(req).await,
            ClientRequest::ListPrompts(req) => self.handle_list_prompts(req),
            ClientRequest::GetPrompt(req) => self.handle_get_prompt(req).await,
            ClientRequest::ListResources(req) => self.handle_list_resources(req).await,
            ClientRequest::ReadResource(req) => self.handle_read_resource(req).await,
            ClientRequest::ListResourceTemplates(req) => {
                Self::handle_list_resource_templates(self, req)
            },
            ClientRequest::Subscribe(_)
            | ClientRequest::Unsubscribe(_)
            | ClientRequest::Complete(_)
            | ClientRequest::SetLoggingLevel { level: _ }
            | ClientRequest::Ping => Ok(serde_json::json!({})),
        };

        match result {
            Ok(value) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: id.clone(),
                payload: crate::types::jsonrpc::ResponsePayload::Result(value),
            },
            Err(e) => JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: id.clone(),
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

    async fn handle_call_tool(&self, req: CallToolRequest) -> Result<Value> {
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Tool '{}' not found", req.name)))?;

        let result = handler.handle(req.arguments).await?;
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

    async fn handle_get_prompt(&self, req: GetPromptRequest) -> Result<Value> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::not_found(format!("Prompt '{}' not found", req.name)))?;

        let result = handler.handle(req.arguments).await?;
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_list_resources(&self, req: ListResourcesRequest) -> Result<Value> {
        if let Some(handler) = &self.resources {
            let result = handler.list(req.cursor).await?;
            Ok(serde_json::to_value(result)?)
        } else {
            Ok(serde_json::to_value(ListResourcesResult {
                resources: vec![],
                next_cursor: None,
            })?)
        }
    }

    async fn handle_read_resource(&self, req: ReadResourceRequest) -> Result<Value> {
        let handler = self
            .resources
            .as_ref()
            .ok_or_else(|| Error::not_found("No resource handler configured".to_string()))?;

        let result = handler.read(&req.uri).await?;
        Ok(serde_json::to_value(result)?)
    }

    #[allow(clippy::unused_self)]
    fn handle_list_resource_templates(&self, _req: ListResourceTemplatesRequest) -> Result<Value> {
        Ok(serde_json::to_value(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })?)
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
            .finish()
    }
}

impl ServerBuilder {
    /// Create a new server builder.
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            capabilities: ServerCapabilities::default(),
            tools: HashMap::new(),
            prompts: HashMap::new(),
            resources: None,
        }
    }

    /// Set the server name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the server version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set server capabilities.
    pub fn capabilities(mut self, capabilities: ServerCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Add a tool handler.
    pub fn tool(mut self, name: impl Into<String>, handler: impl ToolHandler + 'static) -> Self {
        self.tools.insert(name.into(), Arc::new(handler));
        self
    }

    /// Add a prompt handler.
    pub fn prompt(
        mut self,
        name: impl Into<String>,
        handler: impl PromptHandler + 'static,
    ) -> Self {
        self.prompts.insert(name.into(), Arc::new(handler));
        self
    }

    /// Set the resource handler.
    pub fn resources(mut self, handler: impl ResourceHandler + 'static) -> Self {
        self.resources = Some(Arc::new(handler));
        self
    }

    /// Build the server.
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
            client_capabilities: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            notification_tx: None,
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
        jsonrpc::ResponsePayload,
        ClientCapabilities, InitializeRequest, ServerCapabilities, TransportMessage,
        ToolCapabilities,
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
            messages.pop().map_or_else(|| Err(Error::protocol_msg("No more messages")), Ok)
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
        async fn handle(&self, _args: Value) -> Result<Value> {
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
        async fn handle(&self, _args: HashMap<String, String>) -> Result<crate::types::GetPromptResult> {
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
        async fn read(&self, uri: &str) -> Result<crate::types::ReadResourceResult> {
            self.contents
                .get(uri)
                .cloned()
                .ok_or_else(|| Error::not_found(format!("Resource '{}' not found", uri)))
        }

        async fn list(&self, _cursor: Option<String>) -> Result<crate::types::ListResourcesResult> {
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
        let result = Server::builder()
            .version("1.0.0")
            .build();
        assert!(result.is_err());

        // Missing version
        let result = Server::builder()
            .name("test-server")
            .build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let init_request = TransportMessage::Request {
            id: RequestId::from(1i64),
            request: Request::Client(ClientRequest::Initialize(InitializeRequest {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ClientCapabilities {
                    tools: Some(ToolCapabilities::default()),
                    ..Default::default()
                },
                client_info: Implementation {
                    name: "test-client".to_string(),
                    version: "1.0.0".to_string(),
                },
            })),
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
            .resources(MockResource::new().with_resource("test://uri".to_string(), resource_content))
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

        let request = Request::Client(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        }));

        let response = server.handle_request(RequestId::from(1i64), request).await;
        
        assert_eq!(response.id, RequestId::from(1i64));
        match response.payload {
            ResponsePayload::Result(_) => {
                assert!(server.is_initialized().await);
            }
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

        let request = Request::Client(ClientRequest::ListTools(ListToolsRequest { cursor: None }));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            }
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

        let request = Request::Client(ClientRequest::CallTool(CallToolRequest {
            name: "test-tool".to_string(),
            arguments: json!({"input": "test"}),
        }));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let call_result: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(!call_result.is_error);
                assert_eq!(call_result.content.len(), 1);
            }
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

        let request = Request::Client(ClientRequest::CallTool(CallToolRequest {
            name: "nonexistent-tool".to_string(),
            arguments: json!({}),
        }));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            }
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

        let request = Request::Client(ClientRequest::ListPrompts(ListPromptsRequest { cursor: None }));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let list_result: ListPromptsResult = serde_json::from_value(result).unwrap();
                assert_eq!(list_result.prompts.len(), 1);
                assert_eq!(list_result.prompts[0].name, "test-prompt");
            }
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

        let request = Request::Client(ClientRequest::GetPrompt(GetPromptRequest {
            name: "test-prompt".to_string(),
            arguments: HashMap::new(),
        }));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let get_result: crate::types::GetPromptResult = serde_json::from_value(result).unwrap();
                assert_eq!(get_result.description, prompt_result.description);
            }
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
            .resources(MockResource::new().with_resource("test://uri".to_string(), resource_content))
            .build()
            .unwrap();

        let request = Request::Client(ClientRequest::ListResources(ListResourcesRequest { cursor: None }));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let resources_result: ListResourcesResult = serde_json::from_value(result).unwrap();
                assert_eq!(resources_result.resources.len(), 0); // MockResource has empty list by default
            }
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
            .resources(MockResource::new().with_resource("test://uri".to_string(), resource_content.clone()))
            .build()
            .unwrap();

        let request = Request::Client(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "test://uri".to_string(),
        }));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let read_result: crate::types::ReadResourceResult = serde_json::from_value(result).unwrap();
                assert_eq!(read_result.contents.len(), 1);
            }
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

        let request = Request::Client(ClientRequest::ReadResource(ReadResourceRequest {
            uri: "nonexistent://uri".to_string(),
        }));

        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert!(error.message.contains("not found"));
            }
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

        let request = Request::Client(ClientRequest::Ping);
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Result(_) => {
                // Success
            }
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

        let request = Request::Server(crate::types::ServerRequest::CreateMessage(
            crate::types::protocol::CreateMessageParams {
                messages: vec![],
                model_preferences: None,
                system_prompt: None,
                include_context: crate::types::protocol::IncludeContext::None,
                temperature: None,
                max_tokens: None,
                stop_sequences: None,
                metadata: None,
            }
        ));
        let response = server.handle_request(RequestId::from(1i64), request).await;

        match response.payload {
            ResponsePayload::Error(error) => {
                assert_eq!(error.code, -32601);
                assert!(error.message.contains("not supported"));
            }
            ResponsePayload::Result(_) => panic!("Expected error response"),
        }
    }
}
