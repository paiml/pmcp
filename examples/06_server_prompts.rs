//! Example: Server with prompt support
//!
//! This example demonstrates:
//! - Creating a server that provides prompts
//! - Implementing prompt handlers
//! - Dynamic prompt generation with arguments
//! - Prompt templates and formatting

use async_trait::async_trait;
use pmcp::{
    types::{GetPromptResult, PromptArgument, PromptInfo, PromptMessage, Role},
    PromptHandler, Server, ServerCapabilities,
};
use std::collections::HashMap;

// Code review prompt handler
struct CodeReviewPrompt;

#[async_trait]
impl PromptHandler for CodeReviewPrompt {
    async fn handle(&self, args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
        let language = args
            .get("language")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let code = args
            .get("code")
            .ok_or_else(|| pmcp::Error::validation("code argument is required"))?;
        let focus = args.get("focus").map(|s| s.as_str()).unwrap_or("general");

        let mut messages = vec![];

        // System message
        messages.push(PromptMessage {
            role: Role::System,
            content: pmcp::types::MessageContent::Text {
                text: format!(
                    "You are an expert {} code reviewer. Focus on {} aspects of the code. \
                     Provide constructive feedback with specific suggestions for improvement.",
                    language, focus
                ),
            },
        });

        // User message with the code
        messages.push(PromptMessage {
            role: Role::User,
            content: pmcp::types::MessageContent::Text {
                text: format!(
                    "Please review this {} code:\n\n```{}\n{}\n```",
                    language, language, code
                ),
            },
        });

        Ok(GetPromptResult {
            messages,
            description: Some(format!(
                "Code review for {} code focusing on {}",
                language, focus
            )),
        })
    }
}

// Data analysis prompt handler
struct DataAnalysisPrompt;

#[async_trait]
impl PromptHandler for DataAnalysisPrompt {
    async fn handle(&self, args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
        let data_type = args.get("data_type").map(|s| s.as_str()).unwrap_or("CSV");
        let data = args
            .get("data")
            .ok_or_else(|| pmcp::Error::validation("data argument is required"))?;
        let question = args.get("question").map(|s| s.as_str());
        let output_format = args
            .get("output_format")
            .map(|s| s.as_str())
            .unwrap_or("summary");

        let mut messages = vec![];

        // System message
        messages.push(PromptMessage {
            role: Role::System,
            content: pmcp::types::MessageContent::Text {
                text: format!(
                    "You are a data analyst expert. Analyze the provided {} data and \
                     present your findings in {} format. Be precise and data-driven.",
                    data_type, output_format
                ),
            },
        });

        // User message with the data
        let mut user_text = format!("Here is the {} data to analyze:\n\n{}", data_type, data);

        if let Some(q) = question {
            user_text.push_str(&format!("\n\nSpecific question: {}", q));
        }

        messages.push(PromptMessage {
            role: Role::User,
            content: pmcp::types::MessageContent::Text { text: user_text },
        });

        Ok(GetPromptResult {
            messages,
            description: Some(format!("Data analysis of {} data", data_type)),
        })
    }
}

// Writing assistant prompt handler
struct WritingAssistantPrompt;

#[async_trait]
impl PromptHandler for WritingAssistantPrompt {
    async fn handle(&self, args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
        let style = args
            .get("style")
            .map(|s| s.as_str())
            .unwrap_or("professional");
        let topic = args
            .get("topic")
            .ok_or_else(|| pmcp::Error::validation("topic argument is required"))?;
        let length = args.get("length").map(|s| s.as_str()).unwrap_or("medium");
        let audience = args
            .get("audience")
            .map(|s| s.as_str())
            .unwrap_or("general");

        let mut messages = vec![];

        // System message
        messages.push(PromptMessage {
            role: Role::System,
            content: pmcp::types::MessageContent::Text {
                text: format!(
                    "You are a skilled writing assistant. Write in a {} style for a {} audience. \
                     The content should be {} in length. Ensure clarity, engagement, and appropriate tone.",
                    style, audience, length
                ),
            },
        });

        // User message
        messages.push(PromptMessage {
            role: Role::User,
            content: pmcp::types::MessageContent::Text {
                text: format!("Write about: {}", topic),
            },
        });

        Ok(GetPromptResult {
            messages,
            description: Some(format!(
                "Writing assistance for '{}' in {} style",
                topic, style
            )),
        })
    }
}

// List available prompts
fn get_available_prompts() -> Vec<PromptInfo> {
    vec![
        PromptInfo {
            name: "code-review".to_string(),
            description: Some("Review code with expert feedback".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "code".to_string(),
                    description: Some("The code to review".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "language".to_string(),
                    description: Some(
                        "Programming language (e.g., rust, python, javascript)".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "focus".to_string(),
                    description: Some(
                        "Review focus: general, performance, security, style".to_string(),
                    ),
                    required: false,
                },
            ]),
        },
        PromptInfo {
            name: "data-analysis".to_string(),
            description: Some("Analyze data and provide insights".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "data".to_string(),
                    description: Some("The data to analyze".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "data_type".to_string(),
                    description: Some("Type of data: CSV, JSON, text".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "question".to_string(),
                    description: Some("Specific question about the data".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "output_format".to_string(),
                    description: Some(
                        "Output format: summary, detailed, visualization".to_string(),
                    ),
                    required: false,
                },
            ]),
        },
        PromptInfo {
            name: "writing-assistant".to_string(),
            description: Some("Generate written content on any topic".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "topic".to_string(),
                    description: Some("The topic to write about".to_string()),
                    required: true,
                },
                PromptArgument {
                    name: "style".to_string(),
                    description: Some(
                        "Writing style: professional, casual, academic, creative".to_string(),
                    ),
                    required: false,
                },
                PromptArgument {
                    name: "length".to_string(),
                    description: Some("Content length: short, medium, long".to_string()),
                    required: false,
                },
                PromptArgument {
                    name: "audience".to_string(),
                    description: Some(
                        "Target audience: general, technical, children, experts".to_string(),
                    ),
                    required: false,
                },
            ]),
        },
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Server Prompts Example ===");
    println!("Starting server with prompt templates...\n");

    // Build server with prompt support
    let server = Server::builder()
        .name("prompt-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::prompts_only())
        .prompt("code-review", CodeReviewPrompt)
        .prompt("data-analysis", DataAnalysisPrompt)
        .prompt("writing-assistant", WritingAssistantPrompt)
        .build()?;

    println!("Server ready! Available prompts:");
    for prompt in get_available_prompts() {
        println!(
            "\n📝 {}: {}",
            prompt.name,
            prompt.description.as_ref().unwrap_or(&"".to_string())
        );
        if let Some(args) = &prompt.arguments {
            println!("   Arguments:");
            for arg in args {
                let required = if arg.required { " (required)" } else { "" };
                println!(
                    "     - {}{}: {}",
                    arg.name,
                    required,
                    arg.description.as_ref().unwrap_or(&"".to_string())
                );
            }
        }
    }
    println!("\nListening on stdio...");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
