//! Example: Logging in MCP
//!
//! This example demonstrates:
//! - Server logging with different levels
//! - Client log message handling
//! - Structured logging with metadata
//! - Log filtering and processing

use pmcp::{
    Client, Server, ClientCapabilities, ServerCapabilities, 
    StdioTransport, ToolHandler,
    types::{LogLevel, CallToolResult}
};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

// Tool that demonstrates logging
struct LoggingTool;

#[async_trait]
impl ToolHandler for LoggingTool {
    async fn handle(&self, _arguments: Value) -> pmcp::Result<CallToolResult> {
        // Log at different levels
        pmcp::log(LogLevel::Debug, "Starting tool execution", None).await;
        
        // Simulate some work with progress logging
        for i in 1..=3 {
            pmcp::log(
                LogLevel::Info, 
                &format!("Processing step {}/3", i),
                Some(json!({
                    "step": i,
                    "total": 3,
                    "progress": format!("{}%", i * 33)
                }))
            ).await;
            
            sleep(Duration::from_millis(500)).await;
        }
        
        // Log a warning
        pmcp::log(
            LogLevel::Warning,
            "Resource usage is high",
            Some(json!({
                "cpu": "85%",
                "memory": "92%",
                "action": "consider scaling"
            }))
        ).await;
        
        // Log completion
        pmcp::log(LogLevel::Info, "Tool execution completed", None).await;
        
        Ok(CallToolResult {
            content: vec![json!({
                "status": "completed",
                "steps_processed": 3
            })],
            is_error: false,
        })
    }
}

// Server that logs lifecycle events
async fn run_logging_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  Starting logging server...\n");
    
    let server = Server::builder()
        .name("logging-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(Default::default()),
            logging: Some(Default::default()),
            ..Default::default()
        })
        .tool("process_with_logs", LoggingTool)
        .build()?;
    
    // Log server startup
    pmcp::log(
        LogLevel::Info,
        "Server initialized and ready",
        Some(json!({
            "name": "logging-server",
            "version": "1.0.0",
            "pid": std::process::id()
        }))
    ).await;
    
    // Run server
    server.run_stdio().await?;
    
    Ok(())
}

// Client that receives and processes log messages
async fn run_logging_client() -> Result<(), Box<dyn std::error::Error>> {
    println!("💻 Starting logging client...\n");
    
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);
    
    // Enable logging support
    let capabilities = ClientCapabilities {
        logging: Some(Default::default()),
        tools: Some(Default::default()),
        ..Default::default()
    };
    
    // Set up log handler
    client.on_log_message(|level, message, data| {
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
        let level_icon = match level {
            LogLevel::Debug => "🐛",
            LogLevel::Info => "ℹ️ ",
            LogLevel::Warning => "⚠️ ",
            LogLevel::Error => "❌",
            LogLevel::Alert => "🚨",
            LogLevel::Critical => "💥",
            LogLevel::Emergency => "🆘",
            LogLevel::Notice => "📢",
        };
        
        print!("[{}] {} {}: {}", timestamp, level_icon, level, message);
        
        if let Some(data) = data {
            println!(" | {}", serde_json::to_string(&data).unwrap_or_default());
        } else {
            println!();
        }
    });
    
    // Initialize connection
    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");
    
    // Call tool that generates logs
    println!("📞 Calling tool that generates logs:\n");
    let result = client.call_tool("process_with_logs", json!({})).await?;
    
    println!("\n✅ Tool result: {}", serde_json::to_string_pretty(&result.content)?);
    
    // Demonstrate client-side logging
    println!("\n📝 Client-side logging examples:\n");
    
    // Log with different levels
    client.log(LogLevel::Debug, "Debugging connection state", None).await?;
    client.log(LogLevel::Info, "Processing completed", Some(json!({
        "items_processed": 42,
        "duration_ms": 1337
    }))).await?;
    client.log(LogLevel::Warning, "Cache miss rate high", Some(json!({
        "miss_rate": "45%",
        "threshold": "20%"
    }))).await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();
    
    println!("=== MCP Logging Example ===");
    println!("This example demonstrates logging in both server and client.\n");
    
    // In a real application, you would run either the server or client
    // For this example, we'll show the client side
    run_logging_client().await?;
    
    println!("\n📌 Note: To see server-side logging, run the server example separately.");
    
    Ok(())
}