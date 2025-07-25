//! Example: Progress notifications in MCP
//!
//! This example demonstrates:
//! - Sending progress updates from tools
//! - Handling progress notifications in clients
//! - Progress tokens and tracking
//! - Cancellable operations with progress

use pmcp::{
    Client, Server, ClientCapabilities, ServerCapabilities,
    StdioTransport, ToolHandler,
    types::{CallToolResult, ProgressNotification, ProgressToken}
};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use std::sync::Arc;
use parking_lot::Mutex;

// Tool that reports progress
struct LongRunningTool {
    progress_tracker: Arc<Mutex<HashMap<ProgressToken, f64>>>,
}

#[async_trait]
impl ToolHandler for LongRunningTool {
    async fn handle_with_progress(
        &self, 
        arguments: Value,
        progress_token: Option<ProgressToken>
    ) -> pmcp::Result<CallToolResult> {
        let total_steps = arguments.get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        
        let task_name = arguments.get("task_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Processing");
        
        // Track progress if token provided
        if let Some(token) = &progress_token {
            self.progress_tracker.lock().insert(token.clone(), 0.0);
        }
        
        println!("🚀 Starting {} with {} steps", task_name, total_steps);
        
        // Send initial progress
        if let Some(token) = &progress_token {
            pmcp::send_progress(ProgressNotification {
                progress_token: token.clone(),
                progress: 0.0,
                message: Some(format!("Starting {}", task_name)),
            }).await?;
        }
        
        // Simulate long-running operation
        for i in 0..total_steps {
            // Check for cancellation
            if pmcp::is_cancelled(&progress_token).await {
                println!("⚠️  Operation cancelled at step {}/{}", i, total_steps);
                return Ok(CallToolResult {
                    content: vec![json!({
                        "status": "cancelled",
                        "completed_steps": i,
                        "total_steps": total_steps,
                    })],
                    is_error: false,
                });
            }
            
            // Do some work
            sleep(Duration::from_millis(500)).await;
            
            let progress = ((i + 1) as f64 / total_steps as f64) * 100.0;
            
            // Update progress
            if let Some(token) = &progress_token {
                self.progress_tracker.lock().insert(token.clone(), progress);
                
                pmcp::send_progress(ProgressNotification {
                    progress_token: token.clone(),
                    progress,
                    message: Some(format!("{}: Step {}/{} completed", task_name, i + 1, total_steps)),
                }).await?;
            }
            
            println!("   Step {}/{} completed ({}%)", i + 1, total_steps, progress as u32);
        }
        
        // Send completion
        if let Some(token) = &progress_token {
            pmcp::send_progress(ProgressNotification {
                progress_token: token.clone(),
                progress: 100.0,
                message: Some(format!("{} completed successfully!", task_name)),
            }).await?;
            
            // Clean up tracking
            self.progress_tracker.lock().remove(token);
        }
        
        Ok(CallToolResult {
            content: vec![json!({
                "status": "completed",
                "completed_steps": total_steps,
                "total_steps": total_steps,
                "task_name": task_name,
            })],
            is_error: false,
        })
    }
    
    async fn handle(&self, arguments: Value) -> pmcp::Result<CallToolResult> {
        // Fallback for non-progress calls
        self.handle_with_progress(arguments, None).await
    }
}

// Tool that performs parallel operations with progress
struct ParallelProcessingTool;

#[async_trait]
impl ToolHandler for ParallelProcessingTool {
    async fn handle_with_progress(
        &self,
        arguments: Value,
        progress_token: Option<ProgressToken>
    ) -> pmcp::Result<CallToolResult> {
        let items = arguments.get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(5);
        
        println!("🔄 Starting parallel processing of {} items", items);
        
        // Create sub-tokens for parallel operations
        let mut handles = vec![];
        
        for i in 0..items {
            let sub_token = progress_token.as_ref().map(|token| {
                ProgressToken::String(format!("{}-item-{}", token, i))
            });
            
            let handle = tokio::spawn(async move {
                // Send initial progress for this item
                if let Some(token) = &sub_token {
                    pmcp::send_progress(ProgressNotification {
                        progress_token: token.clone(),
                        progress: 0.0,
                        message: Some(format!("Starting item {}", i + 1)),
                    }).await.ok();
                }
                
                // Simulate processing with variable time
                let duration = 1000 + (i * 200);
                for step in 0..5 {
                    sleep(Duration::from_millis(duration / 5)).await;
                    
                    if let Some(token) = &sub_token {
                        let progress = ((step + 1) as f64 / 5.0) * 100.0;
                        pmcp::send_progress(ProgressNotification {
                            progress_token: token.clone(),
                            progress,
                            message: Some(format!("Item {}: {}% complete", i + 1, progress as u32)),
                        }).await.ok();
                    }
                }
                
                format!("Item {} processed", i + 1)
            });
            
            handles.push(handle);
        }
        
        // Wait for all items to complete
        let mut results = vec![];
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(format!("Error: {}", e)),
            }
        }
        
        Ok(CallToolResult {
            content: vec![json!({
                "status": "completed",
                "results": results,
                "total_items": items,
            })],
            is_error: false,
        })
    }
    
    async fn handle(&self, arguments: Value) -> pmcp::Result<CallToolResult> {
        self.handle_with_progress(arguments, None).await
    }
}

// Client that tracks progress
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();
    
    println!("=== MCP Progress Notifications Example ===\n");
    
    // Create client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);
    
    // Enable progress support
    let capabilities = ClientCapabilities {
        tools: Some(Default::default()),
        progress: Some(Default::default()),
        ..Default::default()
    };
    
    // Set up progress handler
    let progress_tracker = Arc::new(Mutex::new(HashMap::<ProgressToken, f64>::new()));
    let tracker_clone = progress_tracker.clone();
    
    client.on_progress(move |notification| {
        let progress_bar = "█".repeat((notification.progress / 5.0) as usize);
        let empty_bar = "░".repeat(20 - (notification.progress / 5.0) as usize);
        
        println!("📊 Progress [{}{}] {:.1}% - {}",
            progress_bar,
            empty_bar,
            notification.progress,
            notification.message.as_ref().unwrap_or(&"Working...".to_string())
        );
        
        // Track progress
        tracker_clone.lock().insert(notification.progress_token.clone(), notification.progress);
    });
    
    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");
    
    // Example 1: Simple progress tracking
    println!("📋 Example 1: Long-running task with progress\n");
    let progress_token = ProgressToken::String(format!("task-{}", uuid::Uuid::new_v4()));
    
    let result = client.call_tool_with_progress(
        "long_running_task",
        json!({
            "task_name": "Data Processing",
            "steps": 8
        }),
        Some(progress_token.clone())
    ).await?;
    
    println!("\n✅ Result: {}\n", serde_json::to_string_pretty(&result.content)?);
    
    // Example 2: Parallel operations with sub-progress
    println!("📋 Example 2: Parallel processing with sub-progress\n");
    let progress_token = ProgressToken::String(format!("parallel-{}", uuid::Uuid::new_v4()));
    
    let result = client.call_tool_with_progress(
        "parallel_processor",
        json!({
            "items": ["A", "B", "C", "D"]
        }),
        Some(progress_token.clone())
    ).await?;
    
    println!("\n✅ Result: {}\n", serde_json::to_string_pretty(&result.content)?);
    
    // Example 3: Cancellable operation
    println!("📋 Example 3: Cancellable operation\n");
    let progress_token = ProgressToken::String(format!("cancel-{}", uuid::Uuid::new_v4()));
    let cancel_token = progress_token.clone();
    
    // Start operation in background
    let operation = tokio::spawn(async move {
        client.call_tool_with_progress(
            "long_running_task",
            json!({
                "task_name": "Cancellable Task",
                "steps": 20
            }),
            Some(progress_token)
        ).await
    });
    
    // Cancel after 2 seconds
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        println!("\n🛑 Sending cancellation request...");
        pmcp::cancel_operation(&cancel_token).await.ok();
    });
    
    // Wait for operation
    match operation.await {
        Ok(Ok(result)) => {
            println!("\n✅ Operation result: {}", serde_json::to_string_pretty(&result.content)?);
        }
        Ok(Err(e)) => {
            println!("\n❌ Operation error: {}", e);
        }
        Err(e) => {
            println!("\n❌ Task error: {}", e);
        }
    }
    
    // Show final progress state
    println!("\n📈 Final progress state:");
    for (token, progress) in progress_tracker.lock().iter() {
        println!("   {}: {:.1}%", token, progress);
    }
    
    Ok(())
}

use std::collections::HashMap;