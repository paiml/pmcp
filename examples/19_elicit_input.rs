//! Example showing user input elicitation in tools.

use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::elicitation::{ElicitInput, ElicitationContext, ElicitationManager};
use pmcp::server::{Server, ServerCapabilities, ToolHandler};
use pmcp::types::elicitation::{
    elicit_boolean, elicit_number, elicit_select, elicit_text, InputType, SelectOption,
};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

/// Tool that demonstrates various input elicitation types
struct InteractiveConfigTool {
    elicitation: Arc<ElicitationContext>,
}

#[async_trait]
impl ToolHandler for InteractiveConfigTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        info!("Starting interactive configuration...");

        // Elicit project name
        let name_response = self
            .elicitation
            .elicit_input(
                elicit_text("What is the name of your project?")
                    .description("This will be used as the package name")
                    .required()
                    .min(3.0)
                    .max(50.0)
                    .pattern("^[a-z][a-z0-9-]*$")
                    .build(),
            )
            .await?;

        if name_response.cancelled {
            return Ok(
                json!({"status": "cancelled", "message": "Configuration cancelled by user"}),
            );
        }

        let project_name = name_response
            .value
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "my-project".to_string());

        // Elicit project type
        let type_options = vec![
            SelectOption {
                value: json!("library"),
                label: "Library".to_string(),
                description: Some("A reusable library/package".to_string()),
                disabled: false,
            },
            SelectOption {
                value: json!("application"),
                label: "Application".to_string(),
                description: Some("A standalone application".to_string()),
                disabled: false,
            },
            SelectOption {
                value: json!("cli"),
                label: "CLI Tool".to_string(),
                description: Some("A command-line interface tool".to_string()),
                disabled: false,
            },
        ];

        let type_response = self
            .elicitation
            .elicit_input(
                elicit_select("What type of project is this?", type_options)
                    .default(json!("application"))
                    .build(),
            )
            .await?;

        let project_type = type_response
            .value
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "application".to_string());

        // Elicit version number
        let version_response = self
            .elicitation
            .elicit_input(
                elicit_text("Initial version number?")
                    .description("Semantic version (e.g., 0.1.0)")
                    .default(json!("0.1.0"))
                    .pattern(r"^\d+\.\d+\.\d+(-\w+)?$")
                    .build(),
            )
            .await?;

        let version = version_response
            .value
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "0.1.0".to_string());

        // Elicit whether to include tests
        let tests_response = self
            .elicitation
            .elicit_input(
                elicit_boolean("Include test setup?")
                    .description("This will add a testing framework and example tests")
                    .default(json!(true))
                    .build(),
            )
            .await?;

        let include_tests = tests_response
            .value
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Elicit number of worker threads (for applications)
        let mut config = json!({
            "name": project_name,
            "type": project_type,
            "version": version,
            "include_tests": include_tests,
        });

        if project_type == "application" {
            let threads_response = self
                .elicitation
                .elicit_input(
                    elicit_number("Number of worker threads?")
                        .description("For concurrent processing (1-16)")
                        .default(json!(4))
                        .min(1.0)
                        .max(16.0)
                        .build(),
                )
                .await?;

            let threads = threads_response.value.and_then(|v| v.as_u64()).unwrap_or(4);

            config["worker_threads"] = json!(threads);
        }

        Ok(json!({
            "status": "success",
            "configuration": config,
            "message": format!("Project '{}' configured successfully", project_name)
        }))
    }
}

/// Tool that demonstrates handling cancellation and errors
struct SensitiveDataTool {
    elicitation: Arc<ElicitationContext>,
}

#[async_trait]
impl ToolHandler for SensitiveDataTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("read");

        // Always confirm sensitive operations
        let confirm_response = self
            .elicitation
            .elicit_input(
                elicit_boolean(format!(
                    "Are you sure you want to {} sensitive data?",
                    operation
                ))
                .description("This operation cannot be undone")
                .default(json!(false))
                .build(),
            )
            .await?;

        if confirm_response.cancelled {
            return Ok(json!({
                "status": "cancelled",
                "message": "Operation cancelled by user"
            }));
        }

        let confirmed = confirm_response
            .value
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !confirmed {
            return Ok(json!({
                "status": "aborted",
                "message": "Operation not confirmed"
            }));
        }

        // If confirmed, request credentials
        let password_response = self
            .elicitation
            .elicit_input(
                pmcp::types::elicitation::ElicitInputBuilder::new(
                    InputType::Password,
                    "Enter admin password:",
                )
                .description("Required for sensitive operations")
                .required()
                .build(),
            )
            .await?;

        if password_response.cancelled {
            return Ok(json!({
                "status": "cancelled",
                "message": "Authentication cancelled"
            }));
        }

        // Simulate operation
        Ok(json!({
            "status": "success",
            "operation": operation,
            "message": format!("Sensitive {} operation completed", operation)
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create elicitation manager
    let elicitation_manager = Arc::new(ElicitationManager::new());
    let elicitation_ctx = Arc::new(ElicitationContext::new(elicitation_manager.clone()));

    // Create server
    let server = Server::builder()
        .name("elicit-input-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(Default::default()),
            ..Default::default()
        })
        // Add interactive configuration tool
        .tool("configure_project", InteractiveConfigTool {
            elicitation: elicitation_ctx.clone(),
        })
        // Add sensitive data tool
        .tool("sensitive_operation", SensitiveDataTool {
            elicitation: elicitation_ctx.clone(),
        })
        .build()?;

    info!("Starting server with input elicitation examples...");
    info!("\nAvailable tools:");
    info!("1. configure_project - Interactive project configuration");
    info!("   Demonstrates: text, select, boolean, and number inputs");
    info!("\n2. sensitive_operation - Operations requiring confirmation");
    info!("   Arguments:");
    info!("   - operation: Operation type (read, write, delete)");
    info!("   Demonstrates: boolean confirmation and password input");

    info!("\nInput elicitation features:");
    info!("- Request various types of input from users");
    info!("- Validation rules (min/max, patterns, required)");
    info!("- Default values and descriptions");
    info!("- Cancellation handling");
    info!("- Timeout support");

    info!("\nNote: This example demonstrates the API, but requires");
    info!("a client that supports the elicitation protocol to work fully.");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
