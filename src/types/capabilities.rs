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
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create a full set of client capabilities.
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
    pub fn supports_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Check if the client supports prompts.
    pub fn supports_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    /// Check if the client supports resources.
    pub fn supports_resources(&self) -> bool {
        self.resources.is_some()
    }

    /// Check if the client supports sampling.
    pub fn supports_sampling(&self) -> bool {
        self.sampling.is_some()
    }
}

impl ServerCapabilities {
    /// Create a minimal set of server capabilities.
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create capabilities for a tool server.
    pub fn tools_only() -> Self {
        Self {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a prompt server.
    pub fn prompts_only() -> Self {
        Self {
            prompts: Some(PromptCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a resource server.
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
    pub fn provides_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Check if the server provides prompts.
    pub fn provides_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    /// Check if the server provides resources.
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
