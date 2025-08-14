//! Dynamic server management for runtime configuration changes.
//!
//! This module provides functionality to dynamically:
//! - Add/remove tools, prompts, and resources at runtime
//! - Update server capabilities
//! - Manage handler lifecycle
//! - Hot-reload configurations

use crate::error::{Error, ErrorCode, Result};
use crate::server::{PromptHandler, ResourceHandler, SamplingHandler, Server, ToolHandler};
use crate::types::capabilities::SamplingCapabilities;
use crate::types::{PromptInfo, ServerCapabilities, ToolInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Type alias for capability update listeners
type CapabilityListener = Box<dyn Fn(&ServerCapabilities) + Send + Sync>;

/// Dynamic server manager for runtime configuration
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::server::dynamic::DynamicServerManager;
/// use pmcp::server::Server;
/// use std::sync::Arc;
///
/// # async fn example() -> pmcp::Result<()> {
/// let server = Server::builder()
///     .name("dynamic-server")
///     .version("1.0.0")
///     .build()?;
/// let server = Arc::new(server);
///
/// let manager = DynamicServerManager::new(server.clone());
/// // Now you can add/remove tools, prompts, resources at runtime
/// # Ok(())
/// # }
/// ```
pub struct DynamicServerManager {
    /// The server instance
    server: Arc<Server>,

    /// Dynamic tool registry
    dynamic_tools: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,

    /// Dynamic prompt registry
    dynamic_prompts: Arc<RwLock<HashMap<String, Arc<dyn PromptHandler>>>>,

    /// Dynamic resource handler
    dynamic_resources: Arc<RwLock<Option<Arc<dyn ResourceHandler>>>>,

    /// Dynamic sampling handler
    dynamic_sampling: Arc<RwLock<Option<Arc<dyn SamplingHandler>>>>,

    /// Capability update callbacks
    #[allow(dead_code)]
    capability_listeners: Arc<RwLock<Vec<CapabilityListener>>>,
}

impl DynamicServerManager {
    /// Create a new dynamic server manager
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::server::dynamic::DynamicServerManager;
/// use pmcp::server::Server;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let server = Server::builder()
    ///     .name("test-server")
    ///     .version("1.0.0")
    ///     .build()?;
    /// let server_arc = Arc::new(server);
    ///
    /// let manager = DynamicServerManager::new(server_arc);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(server: Arc<Server>) -> Self {
        Self {
            server,
            dynamic_tools: Arc::new(RwLock::new(HashMap::new())),
            dynamic_prompts: Arc::new(RwLock::new(HashMap::new())),
            dynamic_resources: Arc::new(RwLock::new(None)),
            dynamic_sampling: Arc::new(RwLock::new(None)),
            capability_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a tool at runtime
    ///
    /// Adds a tool handler to the dynamic registry for runtime tool availability.
    pub async fn add_tool(
        &self,
        name: impl Into<String>,
        handler: Arc<dyn ToolHandler>,
        _info: ToolInfo,
    ) -> Result<()> {
        let name = name.into();
        info!("Adding dynamic tool: {}", name);

        // Add to dynamic registry
        self.dynamic_tools
            .write()
            .await
            .insert(name.clone(), handler);

        // Update server capabilities to indicate tools are available
        self.update_capabilities(|caps| {
            caps.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(true),
            });
        })
        .await;

        Ok(())
    }

    /// Remove a tool at runtime
    pub async fn remove_tool(&self, name: &str) -> Result<()> {
        info!("Removing dynamic tool: {}", name);

        // Remove from dynamic registry
        if self.dynamic_tools.write().await.remove(name).is_none() {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Tool '{}' not found", name),
            ));
        }

        // Update server capabilities
        self.update_capabilities(|_caps| {
            // Dynamic tools are managed separately, capabilities stay the same
        })
        .await;

        Ok(())
    }

    /// Add a prompt at runtime
    pub async fn add_prompt(
        &self,
        name: impl Into<String>,
        handler: Arc<dyn PromptHandler>,
        _info: PromptInfo,
    ) -> Result<()> {
        let name = name.into();
        info!("Adding dynamic prompt: {}", name);

        // Add to dynamic registry
        self.dynamic_prompts
            .write()
            .await
            .insert(name.clone(), handler);

        // Update server capabilities to indicate prompts are available
        self.update_capabilities(|caps| {
            caps.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(true),
            });
        })
        .await;

        Ok(())
    }

    /// Remove a prompt at runtime
    pub async fn remove_prompt(&self, name: &str) -> Result<()> {
        info!("Removing dynamic prompt: {}", name);

        // Remove from dynamic registry
        if self.dynamic_prompts.write().await.remove(name).is_none() {
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Prompt '{}' not found", name),
            ));
        }

        // Update server capabilities
        self.update_capabilities(|_caps| {
            // Dynamic prompts are managed separately, capabilities stay the same
        })
        .await;

        Ok(())
    }

    /// Set resource handler at runtime
    pub async fn set_resource_handler(&self, handler: Arc<dyn ResourceHandler>) -> Result<()> {
        info!("Setting dynamic resource handler");

        *self.dynamic_resources.write().await = Some(handler);

        // Update server capabilities
        self.update_capabilities(|caps| {
            caps.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            });
        })
        .await;

        Ok(())
    }

    /// Remove resource handler at runtime
    pub async fn remove_resource_handler(&self) -> Result<()> {
        info!("Removing dynamic resource handler");

        *self.dynamic_resources.write().await = None;

        // Update server capabilities
        self.update_capabilities(|caps| {
            caps.resources = None;
        })
        .await;

        Ok(())
    }

    /// Set sampling handler at runtime
    pub async fn set_sampling_handler(&self, handler: Arc<dyn SamplingHandler>) -> Result<()> {
        info!("Setting dynamic sampling handler");

        *self.dynamic_sampling.write().await = Some(handler);

        // Update server capabilities
        self.update_capabilities(|caps| {
            caps.sampling = Some(SamplingCapabilities::default());
        })
        .await;

        Ok(())
    }

    /// Remove sampling handler at runtime
    pub async fn remove_sampling_handler(&self) -> Result<()> {
        info!("Removing dynamic sampling handler");

        *self.dynamic_sampling.write().await = None;

        // Update server capabilities
        self.update_capabilities(|caps| {
            caps.sampling = None;
        })
        .await;

        Ok(())
    }

    /// Register a capability update listener
    pub async fn add_capability_listener<F>(&self, listener: F)
    where
        F: Fn(&ServerCapabilities) + Send + Sync + 'static,
    {
        self.capability_listeners
            .write()
            .await
            .push(Box::new(listener));
    }

    /// Get current dynamic tools
    pub async fn get_dynamic_tools(&self) -> HashMap<String, Arc<dyn ToolHandler>> {
        self.dynamic_tools.read().await.clone()
    }

    /// Get current dynamic prompts
    pub async fn get_dynamic_prompts(&self) -> HashMap<String, Arc<dyn PromptHandler>> {
        self.dynamic_prompts.read().await.clone()
    }

    /// Check if a tool exists (either static or dynamic)
    pub async fn has_tool(&self, name: &str) -> bool {
        self.dynamic_tools.read().await.contains_key(name) || self.server.has_tool(name)
    }

    /// Check if a prompt exists (either static or dynamic)
    pub async fn has_prompt(&self, name: &str) -> bool {
        self.dynamic_prompts.read().await.contains_key(name) || self.server.has_prompt(name)
    }

    /// Reload configuration from a source
    pub async fn reload_configuration(&self, config: DynamicConfig) -> Result<()> {
        info!("Reloading dynamic configuration");

        // Clear existing dynamic handlers
        self.dynamic_tools.write().await.clear();
        self.dynamic_prompts.write().await.clear();

        // Apply new configuration
        for (name, tool) in config.tools {
            self.add_tool(name, tool.handler, tool.info).await?;
        }

        for (name, prompt) in config.prompts {
            self.add_prompt(name, prompt.handler, prompt.info).await?;
        }

        if let Some(resources) = config.resources {
            self.set_resource_handler(resources).await?;
        }

        if let Some(sampling) = config.sampling {
            self.set_sampling_handler(sampling).await?;
        }

        Ok(())
    }

    /// Update server capabilities and notify listeners
    async fn update_capabilities<F>(&self, updater: F)
    where
        F: FnOnce(&mut ServerCapabilities),
    {
        // Note: In a real implementation, we'd need to update the actual
        // server capabilities and send notifications to connected clients
        let mut caps = self.server.capabilities.clone();
        updater(&mut caps);

        // Notify listeners
        let listeners = self.capability_listeners.read().await;
        for listener in listeners.iter() {
            listener(&caps);
        }
    }
}

impl std::fmt::Debug for DynamicServerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicServerManager")
            .field("server", &"Arc<Server>")
            .field("dynamic_tools", &"Arc<RwLock<HashMap<...>>>")
            .field("dynamic_prompts", &"Arc<RwLock<HashMap<...>>>")
            .field("dynamic_resources", &"Arc<RwLock<Option<...>>>")
            .field("dynamic_sampling", &"Arc<RwLock<Option<...>>>")
            .field("capability_listeners", &"Arc<RwLock<Vec<...>>>")
            .finish()
    }
}

/// Configuration for dynamic server updates
#[derive(Default)]
pub struct DynamicConfig {
    /// Dynamic tools to add
    pub tools: HashMap<String, DynamicTool>,

    /// Dynamic prompts to add
    pub prompts: HashMap<String, DynamicPrompt>,

    /// Dynamic resource handler
    pub resources: Option<Arc<dyn ResourceHandler>>,

    /// Dynamic sampling handler
    pub sampling: Option<Arc<dyn SamplingHandler>>,
}

impl std::fmt::Debug for DynamicConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicConfig")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("prompts", &self.prompts.keys().collect::<Vec<_>>())
            .field("resources", &self.resources.is_some())
            .field("sampling", &self.sampling.is_some())
            .finish()
    }
}

/// Dynamic tool configuration
pub struct DynamicTool {
    /// Tool handler
    pub handler: Arc<dyn ToolHandler>,

    /// Tool information
    pub info: ToolInfo,
}

impl std::fmt::Debug for DynamicTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicTool")
            .field("handler", &"Arc<dyn ToolHandler>")
            .field("info", &self.info)
            .finish()
    }
}

/// Dynamic prompt configuration
pub struct DynamicPrompt {
    /// Prompt handler
    pub handler: Arc<dyn PromptHandler>,

    /// Prompt information
    pub info: PromptInfo,
}

impl std::fmt::Debug for DynamicPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicPrompt")
            .field("handler", &"Arc<dyn PromptHandler>")
            .field("info", &self.info)
            .finish()
    }
}

/// Builder for dynamic configuration
#[derive(Debug)]
pub struct DynamicConfigBuilder {
    config: DynamicConfig,
}

impl DynamicConfigBuilder {
    /// Create a new dynamic configuration builder
    pub fn new() -> Self {
        Self {
            config: DynamicConfig::default(),
        }
    }

    /// Add a tool to the configuration
    pub fn tool(
        mut self,
        name: impl Into<String>,
        handler: Arc<dyn ToolHandler>,
        info: ToolInfo,
    ) -> Self {
        self.config
            .tools
            .insert(name.into(), DynamicTool { handler, info });
        self
    }

    /// Add a prompt to the configuration
    pub fn prompt(
        mut self,
        name: impl Into<String>,
        handler: Arc<dyn PromptHandler>,
        info: PromptInfo,
    ) -> Self {
        self.config
            .prompts
            .insert(name.into(), DynamicPrompt { handler, info });
        self
    }

    /// Set the resource handler
    pub fn resources(mut self, handler: Arc<dyn ResourceHandler>) -> Self {
        self.config.resources = Some(handler);
        self
    }

    /// Set the sampling handler
    pub fn sampling(mut self, handler: Arc<dyn SamplingHandler>) -> Self {
        self.config.sampling = Some(handler);
        self
    }

    /// Build the configuration
    pub fn build(self) -> DynamicConfig {
        self.config
    }
}

impl Default for DynamicConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::ServerBuilder;
    use crate::types::GetPromptResult;
    use async_trait::async_trait;
    use serde_json::json;

    struct TestTool;

    #[async_trait]
    impl ToolHandler for TestTool {
        async fn handle(
            &self,
            _args: serde_json::Value,
            _extra: crate::RequestHandlerExtra,
        ) -> Result<serde_json::Value> {
            Ok(json!({"result": "test"}))
        }
    }

    struct TestPrompt;

    #[async_trait]
    impl PromptHandler for TestPrompt {
        async fn handle(
            &self,
            _args: HashMap<String, String>,
            _extra: crate::RequestHandlerExtra,
        ) -> Result<GetPromptResult> {
            Ok(GetPromptResult {
                description: Some("Test prompt".to_string()),
                messages: vec![],
            })
        }
    }

    #[tokio::test]
    async fn test_dynamic_tool_management() {
        let server = Arc::new(
            ServerBuilder::new()
                .name("test")
                .version("1.0.0")
                .build()
                .unwrap(),
        );

        let manager = DynamicServerManager::new(server);

        // Add a tool
        let tool_info = ToolInfo {
            name: "dynamic_test".to_string(),
            description: Some("Dynamic test tool".to_string()),
            input_schema: json!({}),
        };

        manager
            .add_tool("dynamic_test", Arc::new(TestTool), tool_info.clone())
            .await
            .unwrap();

        assert!(manager.has_tool("dynamic_test").await);

        // Remove the tool
        manager.remove_tool("dynamic_test").await.unwrap();

        assert!(!manager.has_tool("dynamic_test").await);
    }

    #[tokio::test]
    async fn test_dynamic_configuration() {
        let server = Arc::new(
            ServerBuilder::new()
                .name("test")
                .version("1.0.0")
                .build()
                .unwrap(),
        );

        let manager = DynamicServerManager::new(server);

        // Build a configuration
        let config = DynamicConfigBuilder::new()
            .tool(
                "tool1",
                Arc::new(TestTool),
                ToolInfo {
                    name: "tool1".to_string(),
                    description: Some("Tool 1".to_string()),
                    input_schema: json!({}),
                },
            )
            .prompt(
                "prompt1",
                Arc::new(TestPrompt),
                PromptInfo {
                    name: "prompt1".to_string(),
                    description: Some("Prompt 1".to_string()),
                    arguments: None,
                },
            )
            .build();

        // Reload configuration
        manager.reload_configuration(config).await.unwrap();

        assert!(manager.has_tool("tool1").await);
        assert!(manager.has_prompt("prompt1").await);
    }
}
