//! Completable arguments support for MCP.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Represents a completable argument that can provide suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletableArgument {
    /// The argument name.
    pub name: String,

    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether this argument is required.
    #[serde(default)]
    pub required: bool,

    /// Completion provider configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<CompletionConfig>,
}

/// Configuration for argument completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionConfig {
    /// Type of completion provider.
    pub provider: CompletionProvider,

    /// Additional provider-specific configuration.
    #[serde(flatten)]
    pub config: HashMap<String, Value>,
}

/// Types of completion providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompletionProvider {
    /// Static list of values.
    Static,
    /// Dynamic completion from a resource.
    Resource,
    /// Completion from a tool call.
    Tool,
    /// File path completion.
    File,
    /// Custom provider.
    Custom(String),
}

/// Completion request for getting suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionRequest {
    /// The argument being completed.
    pub argument: String,

    /// Current partial value.
    pub partial: String,

    /// Values of other arguments.
    pub context: HashMap<String, String>,
}

/// Completion response with suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionResponse {
    /// List of completion suggestions.
    pub completions: Vec<CompletionItem>,

    /// Whether more completions are available.
    #[serde(default)]
    pub has_more: bool,

    /// Continuation token for pagination.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

/// A single completion suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    /// The value to insert.
    pub value: String,

    /// Display label (if different from value).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Description of the completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Icon or emoji for the completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Additional metadata.
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

/// Trait for types that can provide completions.
#[async_trait::async_trait]
pub trait CompletionProviderTrait: Send + Sync {
    /// Get completions for an argument.
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> crate::error::Result<CompletionResponse>;
}

/// Static completion provider with a fixed list of values.
#[derive(Debug)]
pub struct StaticCompletionProvider {
    /// List of completion items.
    items: Vec<CompletionItem>,
}

impl StaticCompletionProvider {
    /// Create a new static completion provider.
    pub fn new(items: Vec<CompletionItem>) -> Self {
        Self { items }
    }

    /// Create from a list of strings.
    pub fn from_strings(values: Vec<String>) -> Self {
        Self {
            items: values
                .into_iter()
                .map(|value| CompletionItem {
                    value,
                    label: None,
                    description: None,
                    icon: None,
                    metadata: HashMap::new(),
                })
                .collect(),
        }
    }
}

#[async_trait::async_trait]
impl CompletionProviderTrait for StaticCompletionProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> crate::error::Result<CompletionResponse> {
        let completions: Vec<CompletionItem> = self
            .items
            .iter()
            .filter(|item| {
                item.value.starts_with(&request.partial)
                    || item
                        .label
                        .as_ref()
                        .is_some_and(|l| l.starts_with(&request.partial))
            })
            .cloned()
            .collect();

        Ok(CompletionResponse {
            completions,
            has_more: false,
            continuation_token: None,
        })
    }
}

/// File path completion provider.
#[derive(Debug)]
pub struct FileCompletionProvider {
    /// Base directory for relative paths.
    base_dir: Option<std::path::PathBuf>,
    /// File extensions to filter by.
    extensions: Option<Vec<String>>,
}

impl FileCompletionProvider {
    /// Create a new file completion provider.
    pub fn new() -> Self {
        Self {
            base_dir: None,
            extensions: None,
        }
    }

    /// Set the base directory.
    pub fn with_base_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.base_dir = Some(dir);
        self
    }

    /// Set file extensions to filter by.
    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.extensions = Some(extensions);
        self
    }
}

#[async_trait::async_trait]
impl CompletionProviderTrait for FileCompletionProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> crate::error::Result<CompletionResponse> {
        use std::path::Path;

        let partial_path = Path::new(&request.partial);
        let (dir_path, file_prefix) = if request.partial.ends_with(std::path::MAIN_SEPARATOR) {
            (partial_path, "")
        } else {
            (
                partial_path.parent().unwrap_or_else(|| Path::new(".")),
                partial_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(""),
            )
        };

        let search_dir = if dir_path.is_absolute() {
            dir_path.to_path_buf()
        } else if let Some(base) = &self.base_dir {
            base.join(dir_path)
        } else {
            dir_path.to_path_buf()
        };

        let mut completions = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(file_prefix) {
                        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());

                        // Check extension filter for files
                        if !is_dir {
                            if let Some(exts) = &self.extensions {
                                let has_valid_ext = Path::new(name)
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .is_some_and(|e| exts.iter().any(|ext| ext == e));

                                if !has_valid_ext {
                                    continue;
                                }
                            }
                        }

                        let value = if request.partial.is_empty() {
                            name.to_string()
                        } else {
                            dir_path.join(name).to_string_lossy().to_string()
                        };

                        completions.push(CompletionItem {
                            value,
                            label: Some(name.to_string()),
                            description: if is_dir {
                                Some("Directory".to_string())
                            } else {
                                Some("File".to_string())
                            },
                            icon: if is_dir {
                                Some("📁".to_string())
                            } else {
                                Some("📄".to_string())
                            },
                            metadata: HashMap::new(),
                        });
                    }
                }
            }
        }

        // Sort completions
        completions.sort_by(|a, b| a.value.cmp(&b.value));

        Ok(CompletionResponse {
            completions,
            has_more: false,
            continuation_token: None,
        })
    }
}

impl Default for FileCompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating completable arguments.
#[derive(Debug)]
pub struct CompletableBuilder {
    name: String,
    description: Option<String>,
    required: bool,
    completion: Option<CompletionConfig>,
}

impl CompletableBuilder {
    /// Create a new completable argument builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            required: false,
            completion: None,
        }
    }

    /// Set the description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Mark as required.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Add static completions.
    pub fn static_completions(mut self, values: Vec<String>) -> Self {
        self.completion = Some(CompletionConfig {
            provider: CompletionProvider::Static,
            config: {
                let mut config = HashMap::new();
                config.insert("values".to_string(), serde_json::to_value(values).unwrap());
                config
            },
        });
        self
    }

    /// Add file path completions.
    pub fn file_completions(mut self) -> Self {
        self.completion = Some(CompletionConfig {
            provider: CompletionProvider::File,
            config: HashMap::new(),
        });
        self
    }

    /// Build the completable argument.
    pub fn build(self) -> CompletableArgument {
        CompletableArgument {
            name: self.name,
            description: self.description,
            required: self.required,
            completion: self.completion,
        }
    }
}

/// Helper function to create a completable argument.
pub fn completable(name: impl Into<String>) -> CompletableBuilder {
    CompletableBuilder::new(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_completion() {
        let provider = StaticCompletionProvider::from_strings(vec![
            "apple".to_string(),
            "apricot".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ]);

        let request = CompletionRequest {
            argument: "fruit".to_string(),
            partial: "ap".to_string(),
            context: HashMap::new(),
        };

        let response = provider.complete(request).await.unwrap();
        assert_eq!(response.completions.len(), 2);
        assert_eq!(response.completions[0].value, "apple");
        assert_eq!(response.completions[1].value, "apricot");
    }

    #[test]
    fn test_completable_builder() {
        let arg = completable("environment")
            .description("Target environment")
            .required()
            .static_completions(vec![
                "development".to_string(),
                "staging".to_string(),
                "production".to_string(),
            ])
            .build();

        assert_eq!(arg.name, "environment");
        assert_eq!(arg.description, Some("Target environment".to_string()));
        assert!(arg.required);
        assert!(arg.completion.is_some());
    }
}
