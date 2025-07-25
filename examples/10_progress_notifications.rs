//! Example: Progress notifications in MCP
//!
//! This example demonstrates:
//! - Sending progress updates from tools
//! - Handling progress notifications in clients
//! - Progress tokens and tracking
//! - Cancellable operations with progress

use async_trait::async_trait;
use parking_lot::Mutex;
use pmcp::{
    types::protocol::ProgressToken, Client, ClientCapabilities, StdioTransport, ToolHandler,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Generate simple unique IDs for progress tokens
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("id-{}-{}", now.as_secs(), now.subsec_nanos())
}

// Tool that reports progress
// Placeholder progress notification type until Server API is implemented
#[derive(Debug, Clone)]
struct ProgressNotification {
    progress_token: ProgressToken,
    progress: f64,
    message: Option<String>,
}

// Placeholder functions until Server API is implemented
async fn send_progress(_notification: ProgressNotification) -> pmcp::Result<()> {
    // In a real implementation, this would send a progress notification
    Ok(())
}

async fn is_cancelled(_token: &Option<ProgressToken>) -> bool {
    // In a real implementation, this would check if the operation was cancelled
    false
}

async fn cancel_operation(_token: &ProgressToken) -> pmcp::Result<()> {
    // In a real implementation, this would cancel the operation
    Ok(())
}

struct LongRunningTool {
    progress_tracker: Arc<Mutex<HashMap<ProgressToken, f64>>>,
}

#[async_trait]
impl ToolHandler for LongRunningTool {
    async fn handle(&self, arguments: Value) -> pmcp::Result<Value> {
        // Extract progress token from arguments if provided
        let progress_token = arguments
            .get("_progress_token")
            .and_then(|v| v.as_str())
            .map(|s| ProgressToken::String(s.to_string()));
        let total_steps = arguments
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let task_name = arguments
            .get("task_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Processing");

        // Track progress if token provided
        if let Some(token) = &progress_token {
            self.progress_tracker.lock().insert(token.clone(), 0.0);
        }

        println!("🚀 Starting {} with {} steps", task_name, total_steps);

        // Send initial progress
        if let Some(token) = &progress_token {
            send_progress(ProgressNotification {
                progress_token: token.clone(),
                progress: 0.0,
                message: Some(format!("Starting {}", task_name)),
            })
            .await?;
        }

        // Simulate long-running operation
        for i in 0..total_steps {
            // Check for cancellation
            if is_cancelled(&progress_token).await {
                println!("⚠️  Operation cancelled at step {}/{}", i, total_steps);
                return Ok(json!({
                    "status": "cancelled",
                    "completed_steps": i,
                    "total_steps": total_steps,
                }));
            }

            // Do some work
            sleep(Duration::from_millis(500)).await;

            let progress = ((i + 1) as f64 / total_steps as f64) * 100.0;

            // Update progress
            if let Some(token) = &progress_token {
                self.progress_tracker.lock().insert(token.clone(), progress);

                send_progress(ProgressNotification {
                    progress_token: token.clone(),
                    progress,
                    message: Some(format!(
                        "{}: Step {}/{} completed",
                        task_name,
                        i + 1,
                        total_steps
                    )),
                })
                .await?;
            }

            println!(
                "   Step {}/{} completed ({}%)",
                i + 1,
                total_steps,
                progress as u32
            );
        }

        // Send completion
        if let Some(token) = &progress_token {
            send_progress(ProgressNotification {
                progress_token: token.clone(),
                progress: 100.0,
                message: Some(format!("{} completed successfully!", task_name)),
            })
            .await?;

            // Clean up tracking
            self.progress_tracker.lock().remove(token);
        }

        Ok(json!({
            "status": "completed",
            "completed_steps": total_steps,
            "total_steps": total_steps,
            "task_name": task_name,
        }))
    }
}

// Tool that performs parallel operations with progress
struct ParallelProcessingTool;

#[async_trait]
impl ToolHandler for ParallelProcessingTool {
    async fn handle(&self, arguments: Value) -> pmcp::Result<Value> {
        // Extract progress token from arguments if provided
        let progress_token = arguments
            .get("_progress_token")
            .and_then(|v| v.as_str())
            .map(|s| ProgressToken::String(s.to_string()));
        let items = arguments
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(5);

        println!("🔄 Starting parallel processing of {} items", items);

        // Create sub-tokens for parallel operations
        let mut handles = vec![];

        for i in 0..items {
            let sub_token = progress_token
                .as_ref()
                .map(|token| ProgressToken::String(format!("{:?}-item-{}", token, i)));

            let handle = tokio::spawn(async move {
                // Send initial progress for this item
                if let Some(token) = &sub_token {
                    send_progress(ProgressNotification {
                        progress_token: token.clone(),
                        progress: 0.0,
                        message: Some(format!("Starting item {}", i + 1)),
                    })
                    .await
                    .ok();
                }

                // Simulate processing with variable time
                let duration = 1000 + (i * 200);
                for step in 0..5 {
                    sleep(Duration::from_millis((duration / 5) as u64)).await;

                    if let Some(token) = &sub_token {
                        let progress = ((step + 1) as f64 / 5.0) * 100.0;
                        send_progress(ProgressNotification {
                            progress_token: token.clone(),
                            progress,
                            message: Some(format!("Item {}: {}% complete", i + 1, progress as u32)),
                        })
                        .await
                        .ok();
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

        Ok(json!({
            "status": "completed",
            "results": results,
            "total_items": items,
        }))
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

    client.on_progress(move |notification| {
        let progress_bar = "█".repeat((notification.progress / 5.0) as usize);
        let empty_bar = "░".repeat(20 - (notification.progress / 5.0) as usize);

        println!(
            "📊 Progress [{}{}] {:.1}% - {}",
            progress_bar,
            empty_bar,
            notification.progress,
            notification
                .message
                .as_ref()
                .unwrap_or(&"Working...".to_string())
        );

        // Track progress
        tracker_clone
            .lock()
            .insert(notification.progress_token.clone(), notification.progress);
    });

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // Example 1: Simple progress tracking
    println!("📋 Example 1: Long-running task with progress\n");
    let progress_token = ProgressToken::String(format!("task-{}", generate_id()));

    // Note: call_tool_with_progress not yet implemented, using regular call_tool
    let result = client
        .call_tool(
            "long_running_task".to_string(),
            json!({
                "task_name": "Data Processing",
                "steps": 8,
                "_progress_token": match &progress_token {
                    ProgressToken::String(s) => s.clone(),
                    ProgressToken::Number(n) => n.to_string(),
                }
            }),
        )
        .await?;

    println!(
        "\n✅ Result: {}\n",
        serde_json::to_string_pretty(&result.content)?
    );

    // Example 2: Parallel operations with sub-progress
    println!("📋 Example 2: Parallel processing with sub-progress\n");
    let progress_token = ProgressToken::String(format!("parallel-{}", generate_id()));

    let result = client
        .call_tool(
            "parallel_processor".to_string(),
            json!({
                "items": ["A", "B", "C", "D"],
                "_progress_token": match &progress_token {
                    ProgressToken::String(s) => s.clone(),
                    ProgressToken::Number(n) => n.to_string(),
                }
            }),
        )
        .await?;

    println!(
        "\n✅ Result: {}\n",
        serde_json::to_string_pretty(&result.content)?
    );

    // Example 3: Cancellable operation
    println!("📋 Example 3: Cancellable operation\n");
    let progress_token = ProgressToken::String(format!("cancel-{}", generate_id()));
    let cancel_token = progress_token.clone();

    // Start operation in background
    let client_clone = client.clone();
    let operation = tokio::spawn(async move {
        client_clone
            .call_tool(
                "long_running_task".to_string(),
                json!({
                    "task_name": "Cancellable Task",
                    "steps": 20,
                    "_progress_token": match &progress_token {
                        ProgressToken::String(s) => s.clone(),
                        ProgressToken::Number(n) => n.to_string(),
                    }
                }),
            )
            .await
    });

    // Cancel after 2 seconds
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        println!("\n🛑 Sending cancellation request...");
        cancel_operation(&cancel_token).await.ok();
    });

    // Wait for operation
    match operation.await {
        Ok(Ok(result)) => {
            println!(
                "\n✅ Operation result: {}",
                serde_json::to_string_pretty(&result)?
            );
        },
        Ok(Err(e)) => {
            println!("\n❌ Operation error: {}", e);
        },
        Err(e) => {
            println!("\n❌ Task error: {}", e);
        },
    }

    // Show final progress state
    println!("\n📈 Final progress state:");
    for (token, progress) in progress_tracker.lock().iter() {
        println!("   {}: {:.1}%", token, progress);
    }

    Ok(())
}
