//! Example: Client prompt usage
//!
//! This example demonstrates:
//! - Listing available prompts from a server
//! - Getting prompt details with arguments
//! - Executing prompts with parameters
//! - Handling prompt responses

use pmcp::{Client, ClientCapabilities, StdioTransport};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Client Prompts Example ===\n");

    // Create and initialize client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Initialize with prompt support
    let capabilities = ClientCapabilities {
        prompts: Some(Default::default()),
        ..Default::default()
    };

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // List available prompts
    println!("📋 Listing available prompts:");
    let prompts_result = client.list_prompts(None).await?;

    for prompt in &prompts_result.prompts {
        println!("\n📝 Prompt: {}", prompt.name);
        if let Some(desc) = &prompt.description {
            println!("   Description: {}", desc);
        }

        // Print arguments if available
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

    // Example 1: Code review prompt
    println!("\n\n💻 Using code review prompt:");
    let mut code_review_args = HashMap::new();
    code_review_args.insert(
        "code".to_string(),
        r"
fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
"
        .to_string(),
    );
    code_review_args.insert("language".to_string(), "rust".to_string());
    code_review_args.insert("focus".to_string(), "performance".to_string());

    match client
        .get_prompt("code-review".to_string(), code_review_args)
        .await
    {
        Ok(result) => {
            println!("   ✅ Prompt generated!");
            if let Some(desc) = &result.description {
                println!("   Description: {}", desc);
            }
            println!("   Messages:");
            for (i, msg) in result.messages.iter().enumerate() {
                println!(
                    "   {}. [{}] {}",
                    i + 1,
                    msg.role,
                    match &msg.content {
                        pmcp::types::Content::Text { text } => text,
                        pmcp::types::Content::Image { .. } => "[Image content]",
                        pmcp::types::Content::Resource { .. } => "[Resource content]",
                    }
                );
            }
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example 2: Data analysis prompt
    println!("\n\n📊 Using data analysis prompt:");
    let mut data_args = HashMap::new();
    data_args.insert(
        "data".to_string(),
        "Product,Sales,Revenue\nWidget A,150,4500\nWidget B,200,8000\nWidget C,100,5000"
            .to_string(),
    );
    data_args.insert("data_type".to_string(), "CSV".to_string());
    data_args.insert(
        "question".to_string(),
        "Which product has the highest profit margin?".to_string(),
    );
    data_args.insert("output_format".to_string(), "detailed".to_string());

    match client
        .get_prompt("data-analysis".to_string(), data_args)
        .await
    {
        Ok(result) => {
            println!("   ✅ Prompt generated!");
            println!("   Message count: {}", result.messages.len());
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example 3: Writing assistant prompt
    println!("\n\n✍️  Using writing assistant prompt:");
    let mut writing_args = HashMap::new();
    writing_args.insert(
        "topic".to_string(),
        "The benefits of Rust for systems programming".to_string(),
    );
    writing_args.insert("style".to_string(), "technical".to_string());
    writing_args.insert("length".to_string(), "short".to_string());
    writing_args.insert("audience".to_string(), "developers".to_string());

    match client
        .get_prompt("writing-assistant".to_string(), writing_args)
        .await
    {
        Ok(result) => {
            println!("   ✅ Prompt generated!");
            if let Some(desc) = &result.description {
                println!("   Description: {}", desc);
            }
        },
        Err(e) => {
            println!("   ❌ Error: {}", e);
        },
    }

    // Example 4: Missing required argument
    println!("\n\n⚠️  Testing error handling (missing required argument):");
    let mut incomplete_args = HashMap::new();
    incomplete_args.insert("language".to_string(), "python".to_string());
    // Missing required "code" argument

    match client
        .get_prompt("code-review".to_string(), incomplete_args)
        .await
    {
        Ok(_) => {
            println!("   Unexpected success!");
        },
        Err(e) => {
            println!("   ✅ Error caught: {}", e);
        },
    }

    Ok(())
}
