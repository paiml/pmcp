//! MCP server implementation.

use crate::error::Result;
use crate::types::{Implementation, ServerCapabilities};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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
/// use mcp_sdk::{Server, ServerCapabilities, ToolHandler};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct MyTool;
///
/// #[async_trait]
/// impl ToolHandler for MyTool {
///     async fn handle(&self, args: Value) -> mcp_sdk::Result<Value> {
///         Ok(serde_json::json!({"result": "success"}))
///     }
/// }
///
/// # async fn example() -> mcp_sdk::Result<()> {
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
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("info", &self.info)
            .field("capabilities", &self.capabilities)
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .finish()
    }
}

impl Server {
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
    #[allow(clippy::unused_async)]
    pub async fn run<T: crate::shared::Transport>(self, _transport: T) -> Result<()> {
        unimplemented!("Server run loop not yet implemented")
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
        })
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
