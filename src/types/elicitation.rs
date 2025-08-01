//! User input elicitation support for MCP.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Input elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitInputRequest {
    /// Unique identifier for this elicitation request.
    pub elicitation_id: String,

    /// Type of input being requested.
    pub input_type: InputType,

    /// Prompt message for the user.
    pub prompt: String,

    /// Additional context or instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Default value if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,

    /// Validation rules for the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<InputValidation>,

    /// Additional metadata.
    #[serde(flatten)]
    pub metadata: HashMap<String, Value>,
}

/// Types of input that can be requested.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputType {
    /// Single line text input.
    Text,
    /// Multi-line text input.
    Textarea,
    /// Boolean yes/no input.
    Boolean,
    /// Numeric input.
    Number,
    /// Single selection from options.
    Select,
    /// Multiple selection from options.
    MultiSelect,
    /// File path selection.
    FilePath,
    /// Directory path selection.
    DirectoryPath,
    /// Password or sensitive input.
    Password,
    /// Date input.
    Date,
    /// Time input.
    Time,
    /// Date and time input.
    DateTime,
    /// Color picker.
    Color,
    /// URL input.
    Url,
    /// Email input.
    Email,
}

/// Input validation rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputValidation {
    /// Whether the input is required.
    #[serde(default)]
    pub required: bool,

    /// Minimum value (for numbers) or length (for strings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum value (for numbers) or length (for strings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    /// Regular expression pattern for validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// List of allowed values (for select/multiselect).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<SelectOption>>,

    /// Custom validation message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Option for select/multiselect inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectOption {
    /// Option value.
    pub value: Value,

    /// Display label.
    pub label: String,

    /// Option description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether this option is disabled.
    #[serde(default)]
    pub disabled: bool,
}

/// Response to an input elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitInputResponse {
    /// The elicitation ID this response is for.
    pub elicitation_id: String,

    /// The user's input value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,

    /// Whether the user cancelled the input.
    #[serde(default)]
    pub cancelled: bool,

    /// Error message if validation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Builder for creating input elicitation requests.
#[derive(Debug)]
pub struct ElicitInputBuilder {
    elicitation_id: String,
    input_type: InputType,
    prompt: String,
    description: Option<String>,
    default: Option<Value>,
    validation: Option<InputValidation>,
    metadata: HashMap<String, Value>,
}

impl ElicitInputBuilder {
    /// Create a new elicitation builder.
    pub fn new(input_type: InputType, prompt: impl Into<String>) -> Self {
        Self {
            elicitation_id: uuid::Uuid::new_v4().to_string(),
            input_type,
            prompt: prompt.into(),
            description: None,
            default: None,
            validation: None,
            metadata: HashMap::new(),
        }
    }

    /// Set a custom elicitation ID.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.elicitation_id = id.into();
        self
    }

    /// Set the description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the default value.
    pub fn default(mut self, value: impl Into<Value>) -> Self {
        self.default = Some(value.into());
        self
    }

    /// Mark as required.
    pub fn required(mut self) -> Self {
        if self.validation.is_none() {
            self.validation = Some(InputValidation {
                required: true,
                min: None,
                max: None,
                pattern: None,
                options: None,
                message: None,
            });
        } else if let Some(validation) = &mut self.validation {
            validation.required = true;
        }
        self
    }

    /// Set minimum value or length.
    pub fn min(mut self, min: f64) -> Self {
        if self.validation.is_none() {
            self.validation = Some(InputValidation {
                required: false,
                min: Some(min),
                max: None,
                pattern: None,
                options: None,
                message: None,
            });
        } else if let Some(validation) = &mut self.validation {
            validation.min = Some(min);
        }
        self
    }

    /// Set maximum value or length.
    pub fn max(mut self, max: f64) -> Self {
        if self.validation.is_none() {
            self.validation = Some(InputValidation {
                required: false,
                min: None,
                max: Some(max),
                pattern: None,
                options: None,
                message: None,
            });
        } else if let Some(validation) = &mut self.validation {
            validation.max = Some(max);
        }
        self
    }

    /// Set validation pattern.
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        if self.validation.is_none() {
            self.validation = Some(InputValidation {
                required: false,
                min: None,
                max: None,
                pattern: Some(pattern.into()),
                options: None,
                message: None,
            });
        } else if let Some(validation) = &mut self.validation {
            validation.pattern = Some(pattern.into());
        }
        self
    }

    /// Set options for select/multiselect.
    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        if self.validation.is_none() {
            self.validation = Some(InputValidation {
                required: false,
                min: None,
                max: None,
                pattern: None,
                options: Some(options),
                message: None,
            });
        } else if let Some(validation) = &mut self.validation {
            validation.options = Some(options);
        }
        self
    }

    /// Add metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the elicitation request.
    pub fn build(self) -> ElicitInputRequest {
        ElicitInputRequest {
            elicitation_id: self.elicitation_id,
            input_type: self.input_type,
            prompt: self.prompt,
            description: self.description,
            default: self.default,
            validation: self.validation,
            metadata: self.metadata,
        }
    }
}

/// Helper function to create a text input elicitation.
pub fn elicit_text(prompt: impl Into<String>) -> ElicitInputBuilder {
    ElicitInputBuilder::new(InputType::Text, prompt)
}

/// Helper function to create a boolean input elicitation.
pub fn elicit_boolean(prompt: impl Into<String>) -> ElicitInputBuilder {
    ElicitInputBuilder::new(InputType::Boolean, prompt)
}

/// Helper function to create a select input elicitation.
pub fn elicit_select(prompt: impl Into<String>, options: Vec<SelectOption>) -> ElicitInputBuilder {
    ElicitInputBuilder::new(InputType::Select, prompt).options(options)
}

/// Helper function to create a number input elicitation.
pub fn elicit_number(prompt: impl Into<String>) -> ElicitInputBuilder {
    ElicitInputBuilder::new(InputType::Number, prompt)
}

/// Helper function to create a file path input elicitation.
pub fn elicit_file(prompt: impl Into<String>) -> ElicitInputBuilder {
    ElicitInputBuilder::new(InputType::FilePath, prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_elicit_text() {
        let request = elicit_text("Enter your name")
            .description("Please provide your full name")
            .required()
            .min(2.0)
            .max(100.0)
            .build();

        assert_eq!(request.input_type, InputType::Text);
        assert_eq!(request.prompt, "Enter your name");
        assert_eq!(
            request.description,
            Some("Please provide your full name".to_string())
        );
        assert!(request.validation.as_ref().unwrap().required);
        assert_eq!(request.validation.as_ref().unwrap().min, Some(2.0));
        assert_eq!(request.validation.as_ref().unwrap().max, Some(100.0));
    }

    #[test]
    fn test_elicit_select() {
        let options = vec![
            SelectOption {
                value: json!("small"),
                label: "Small".to_string(),
                description: Some("Suitable for personal use".to_string()),
                disabled: false,
            },
            SelectOption {
                value: json!("medium"),
                label: "Medium".to_string(),
                description: Some("Good for small teams".to_string()),
                disabled: false,
            },
            SelectOption {
                value: json!("large"),
                label: "Large".to_string(),
                description: Some("For enterprise use".to_string()),
                disabled: false,
            },
        ];

        let request = elicit_select("Choose a size", options.clone())
            .default(json!("medium"))
            .build();

        assert_eq!(request.input_type, InputType::Select);
        assert_eq!(request.prompt, "Choose a size");
        assert_eq!(request.default, Some(json!("medium")));
        assert_eq!(
            request
                .validation
                .as_ref()
                .unwrap()
                .options
                .as_ref()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn test_serialization() {
        let request = elicit_boolean("Enable feature?")
            .default(json!(true))
            .description("This will enable the experimental feature")
            .build();

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["inputType"], "boolean");
        assert_eq!(json["prompt"], "Enable feature?");
        assert_eq!(json["default"], true);
        assert_eq!(
            json["description"],
            "This will enable the experimental feature"
        );

        // Test deserialization
        let deserialized: ElicitInputRequest = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.prompt, request.prompt);
    }
}
