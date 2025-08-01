//! Capability definitions for MCP clients and servers.
//!
//! This module defines the capability structures that clients and servers
//! use to advertise their supported features.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Client capabilities advertised during initialization.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::ClientCapabilities;
///
/// let capabilities = ClientCapabilities {
///     experimental: Some([("custom-feature".to_string(), serde_json::json!(true))]
///         .into_iter()
///         .collect()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Tool calling capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,

    /// Prompt capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptCapabilities>,

    /// Resource capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,

    /// Logging capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapabilities>,

    /// Sampling capabilities (for LLM providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,

    /// Roots capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapabilities>,

    /// Experimental capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Server capabilities advertised during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Tool providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,

    /// Prompt providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptCapabilities>,

    /// Resource providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,

    /// Logging capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapabilities>,

    /// Completion capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<CompletionCapabilities>,

    /// Sampling capabilities (for LLM providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,

    /// Experimental capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Tool-related capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Prompt-related capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resource-related capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCapabilities {
    /// Whether resource subscriptions are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,

    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Logging capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingCapabilities {
    /// Supported log levels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
}

/// Sampling capabilities for LLM operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCapabilities {
    /// Supported model families/providers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
}

/// Roots capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapabilities {
    /// Whether list changed notifications are supported
    #[serde(default)]
    pub list_changed: bool,
}

/// Completion capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionCapabilities {
    /// Placeholder for completion capability options
    #[serde(skip)]
    _reserved: (),
}

impl ClientCapabilities {
    /// Create a minimal set of client capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ClientCapabilities;
    ///
    /// // Create minimal capabilities (no features advertised)
    /// let capabilities = ClientCapabilities::minimal();
    /// assert!(!capabilities.supports_tools());
    /// assert!(!capabilities.supports_prompts());
    /// assert!(!capabilities.supports_resources());
    /// assert!(!capabilities.supports_sampling());
    ///
    /// // Use in client initialization
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// let server_info = client.initialize(ClientCapabilities::minimal()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create a full set of client capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ClientCapabilities;
    ///
    /// // Create full capabilities (all features supported)
    /// let capabilities = ClientCapabilities::full();
    /// assert!(capabilities.supports_tools());
    /// assert!(capabilities.supports_prompts());
    /// assert!(capabilities.supports_resources());
    /// assert!(capabilities.supports_sampling());
    ///
    /// // Inspect specific capabilities
    /// assert!(capabilities.tools.unwrap().list_changed.unwrap());
    /// assert!(capabilities.resources.unwrap().subscribe.unwrap());
    ///
    /// // Use in client that needs all features
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// let server_info = client.initialize(ClientCapabilities::full()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn full() -> Self {
        Self {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            prompts: Some(PromptCapabilities {
                list_changed: Some(true),
            }),
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            logging: Some(LoggingCapabilities {
                levels: Some(vec![
                    "debug".to_string(),
                    "info".to_string(),
                    "warning".to_string(),
                    "error".to_string(),
                ]),
            }),
            sampling: Some(SamplingCapabilities::default()),
            roots: Some(RootsCapabilities { list_changed: true }),
            experimental: None,
        }
    }

    /// Check if the client supports tools.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::ToolCapabilities};
    ///
    /// // Minimal capabilities don't support tools
    /// let minimal = ClientCapabilities::minimal();
    /// assert!(!minimal.supports_tools());
    ///
    /// // Full capabilities support tools
    /// let full = ClientCapabilities::full();
    /// assert!(full.supports_tools());
    ///
    /// // Custom capabilities with only tools
    /// let tools_only = ClientCapabilities {
    ///     tools: Some(ToolCapabilities {
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(tools_only.supports_tools());
    ///
    /// // Use to conditionally enable features
    /// fn setup_client(caps: &ClientCapabilities) {
    ///     if caps.supports_tools() {
    ///         println!("Client can call tools");
    ///     }
    /// }
    /// ```
    pub fn supports_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Check if the client supports prompts.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::PromptCapabilities};
    ///
    /// // Check prompt support
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_prompts());
    ///
    /// // Build capabilities with just prompts
    /// let prompts_only = ClientCapabilities {
    ///     prompts: Some(PromptCapabilities {
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(prompts_only.supports_prompts());
    /// assert!(!prompts_only.supports_tools());
    ///
    /// // Conditional logic based on prompt support
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let caps = ClientCapabilities::full();
    /// if caps.supports_prompts() {
    ///     // Client can use prompts
    ///     println!("This client supports prompts");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn supports_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    /// Check if the client supports resources.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::ResourceCapabilities};
    ///
    /// // Check resource support with subscriptions
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_resources());
    ///
    /// // Build capabilities with advanced resource features
    /// let advanced_resources = ClientCapabilities {
    ///     resources: Some(ResourceCapabilities {
    ///         subscribe: Some(true),
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(advanced_resources.supports_resources());
    ///
    /// // Check specific resource capabilities
    /// if let Some(resource_caps) = &advanced_resources.resources {
    ///     assert!(resource_caps.subscribe.unwrap_or(false));
    ///     println!("Client can subscribe to resource changes");
    /// }
    /// ```
    pub fn supports_resources(&self) -> bool {
        self.resources.is_some()
    }

    /// Check if the client supports sampling.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::SamplingCapabilities};
    ///
    /// // Check sampling support for LLM operations
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_sampling());
    ///
    /// // Build LLM client capabilities
    /// let llm_client = ClientCapabilities {
    ///     sampling: Some(SamplingCapabilities {
    ///         models: Some(vec![
    ///             "gpt-4".to_string(),
    ///             "claude-3".to_string(),
    ///             "llama-2".to_string(),
    ///         ]),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(llm_client.supports_sampling());
    ///
    /// // List supported models
    /// if let Some(sampling) = &llm_client.sampling {
    ///     if let Some(models) = &sampling.models {
    ///         println!("Supported models: {:?}", models);
    ///     }
    /// }
    /// ```
    pub fn supports_sampling(&self) -> bool {
        self.sampling.is_some()
    }
}

impl ServerCapabilities {
    /// Create a minimal set of server capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create minimal server with no advertised features
    /// let capabilities = ServerCapabilities::minimal();
    /// assert!(!capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in server that implements custom protocol extensions
    /// # use pmcp::Server;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("minimal-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::minimal())
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create capabilities for a tool server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides tools
    /// let capabilities = ServerCapabilities::tools_only();
    /// assert!(capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in a tool-focused server
    /// # use pmcp::{Server, ToolHandler};
    /// # use async_trait::async_trait;
    /// # struct CalculatorTool;
    /// # #[async_trait]
    /// # impl ToolHandler for CalculatorTool {
    /// #     async fn handle(&self, args: serde_json::Value, _extra: pmcp::RequestHandlerExtra) -> Result<serde_json::Value, pmcp::Error> {
    /// #         Ok(serde_json::json!({"result": 42}))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("calculator-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::tools_only())
    ///     .tool("calculate", CalculatorTool)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn tools_only() -> Self {
        Self {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a prompt server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides prompts
    /// let capabilities = ServerCapabilities::prompts_only();
    /// assert!(!capabilities.provides_tools());
    /// assert!(capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in a prompt template server
    /// # use pmcp::{Server, PromptHandler};
    /// # use async_trait::async_trait;
    /// # use pmcp::types::protocol::{GetPromptResult, PromptMessage, Role, Content};
    /// # struct GreetingPrompt;
    /// # #[async_trait]
    /// # impl PromptHandler for GreetingPrompt {
    /// #     async fn handle(&self, args: std::collections::HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> Result<GetPromptResult, pmcp::Error> {
    /// #         Ok(GetPromptResult {
    /// #             description: Some("Greeting prompt".to_string()),
    /// #             messages: vec![PromptMessage {
    /// #                 role: Role::System,
    /// #                 content: Content::Text { text: "Hello!".to_string() },
    /// #             }],
    /// #         })
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("prompt-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::prompts_only())
    ///     .prompt("greeting", GreetingPrompt)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn prompts_only() -> Self {
        Self {
            prompts: Some(PromptCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a resource server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides resources
    /// let capabilities = ServerCapabilities::resources_only();
    /// assert!(!capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(capabilities.provides_resources());
    ///
    /// // Check subscription support
    /// let resource_caps = capabilities.resources.unwrap();
    /// assert!(resource_caps.subscribe.unwrap());
    /// assert!(resource_caps.list_changed.unwrap());
    ///
    /// // Use in a file system resource server
    /// # use pmcp::{Server, ResourceHandler};
    /// # use async_trait::async_trait;
    /// # use pmcp::types::protocol::{ReadResourceResult, ListResourcesResult, ResourceInfo, Content};
    /// # struct FileResource;
    /// # #[async_trait]
    /// # impl ResourceHandler for FileResource {
    /// #     async fn read(&self, uri: &str, _extra: pmcp::RequestHandlerExtra) -> Result<ReadResourceResult, pmcp::Error> {
    /// #         Ok(ReadResourceResult {
    /// #             contents: vec![Content::Text { text: "File contents".to_string() }],
    /// #         })
    /// #     }
    /// #     async fn list(&self, _path: Option<String>, _extra: pmcp::RequestHandlerExtra) -> Result<ListResourcesResult, pmcp::Error> {
    /// #         Ok(ListResourcesResult {
    /// #             resources: vec![],
    /// #             next_cursor: None,
    /// #         })
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("filesystem-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::resources_only())
    ///     .resources(FileResource)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn resources_only() -> Self {
        Self {
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Check if the server provides tools.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check different server configurations
    /// let tool_server = ServerCapabilities::tools_only();
    /// assert!(tool_server.provides_tools());
    ///
    /// let minimal_server = ServerCapabilities::minimal();
    /// assert!(!minimal_server.provides_tools());
    ///
    /// // Use in server logic
    /// fn validate_server(caps: &ServerCapabilities) {
    ///     if caps.provides_tools() {
    ///         println!("Server can handle tool calls");
    ///     } else {
    ///         println!("Server does not provide tools");
    ///     }
    /// }
    ///
    /// // Combine multiple capabilities
    /// use pmcp::types::capabilities::{ToolCapabilities, PromptCapabilities};
    /// let multi_server = ServerCapabilities {
    ///     tools: Some(ToolCapabilities::default()),
    ///     prompts: Some(PromptCapabilities::default()),
    ///     ..Default::default()
    /// };
    /// assert!(multi_server.provides_tools());
    /// assert!(multi_server.provides_prompts());
    /// ```
    pub fn provides_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Check if the server provides prompts.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check prompt server
    /// let prompt_server = ServerCapabilities::prompts_only();
    /// assert!(prompt_server.provides_prompts());
    /// assert!(!prompt_server.provides_tools());
    ///
    /// // Use in client code to check server features
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let transport = StdioTransport::new();
    /// # let mut client = Client::new(transport);
    /// # let server_info = client.initialize(pmcp::ClientCapabilities::default()).await?;
    /// if server_info.capabilities.provides_prompts() {
    ///     // Server supports prompts, we can list them
    ///     let prompts = client.list_prompts(None).await?;
    ///     println!("Available prompts: {}", prompts.prompts.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn provides_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    /// Check if the server provides resources.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check resource server capabilities
    /// let resource_server = ServerCapabilities::resources_only();
    /// assert!(resource_server.provides_resources());
    ///
    /// // Check if subscriptions are supported
    /// if resource_server.provides_resources() {
    ///     if let Some(res_caps) = &resource_server.resources {
    ///         if res_caps.subscribe.unwrap_or(false) {
    ///             println!("Server supports resource subscriptions");
    ///         }
    ///     }
    /// }
    ///
    /// // Build a full-featured server
    /// use pmcp::types::capabilities::*;
    /// let full_server = ServerCapabilities {
    ///     tools: Some(ToolCapabilities::default()),
    ///     prompts: Some(PromptCapabilities::default()),
    ///     resources: Some(ResourceCapabilities {
    ///         subscribe: Some(true),
    ///         list_changed: Some(true),
    ///     }),
    ///     ..Default::default()
    /// };
    /// assert!(full_server.provides_tools());
    /// assert!(full_server.provides_prompts());
    /// assert!(full_server.provides_resources());
    /// ```
    pub fn provides_resources(&self) -> bool {
        self.resources.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_capabilities_helpers() {
        let minimal = ClientCapabilities::minimal();
        assert!(!minimal.supports_tools());
        assert!(!minimal.supports_prompts());
        assert!(!minimal.supports_resources());
        assert!(!minimal.supports_sampling());

        let full = ClientCapabilities::full();
        assert!(full.supports_tools());
        assert!(full.supports_prompts());
        assert!(full.supports_resources());
        assert!(full.supports_sampling());
    }

    #[test]
    fn server_capabilities_helpers() {
        let tools_only = ServerCapabilities::tools_only();
        assert!(tools_only.provides_tools());
        assert!(!tools_only.provides_prompts());
        assert!(!tools_only.provides_resources());

        let prompts_only = ServerCapabilities::prompts_only();
        assert!(!prompts_only.provides_tools());
        assert!(prompts_only.provides_prompts());
        assert!(!prompts_only.provides_resources());
    }

    #[test]
    fn capabilities_serialization() {
        let caps = ClientCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["tools"]["listChanged"], true);
        assert!(json.get("prompts").is_none());
    }
}
