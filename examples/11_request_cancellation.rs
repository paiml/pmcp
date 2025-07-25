//! Example: Request cancellation in MCP
//!
//! NOTE: This example requires the Server API to be fully implemented.
//! Request cancellation handlers are not yet available in the current SDK.
//!
//! This example demonstrates:
//! - Cancelling in-flight requests
//! - Handling cancellation in tools
//! - Graceful shutdown on cancellation
//! - Cancellation tokens and propagation

use async_trait::async_trait;
use parking_lot::Mutex;
use pmcp::{
    types::{CallToolResult, CancelledNotification, RequestId},
    Client, ClientCapabilities, StdioTransport, ToolHandler,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout, Duration};

// Tool that can be cancelled
struct CancellableTool {
    active_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<()>>>>,
}

impl CancellableTool {
    fn new() -> Self {
        Self {
            active_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ToolHandler for CancellableTool {
    async fn handle_with_cancellation(
        &self,
        arguments: Value,
        request_id: Option<RequestId>,
    ) -> pmcp::Result<CallToolResult> {
        let duration_secs = arguments
            .get("duration")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);

        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Register cancellation handler
        if let Some(id) = &request_id {
            self.active_requests.lock().insert(id.clone(), cancel_tx);
        }

        println!("⏱️  Starting cancellable operation ({}s)", duration_secs);

        // Simulate long-running operation with cancellation support
        let mut completed_steps = 0;
        let total_steps = duration_secs * 2; // 2 steps per second

        for i in 0..total_steps {
            // Check for cancellation
            if cancel_rx.try_recv().is_ok() {
                println!("🛑 Operation cancelled at step {}/{}", i, total_steps);

                // Clean up
                if let Some(id) = &request_id {
                    self.active_requests.lock().remove(id);
                }

                return Ok(CallToolResult {
                    content: vec![json!({
                        "status": "cancelled",
                        "completed_steps": completed_steps,
                        "total_steps": total_steps,
                        "reason": "Client requested cancellation"
                    })],
                    is_error: false,
                });
            }

            // Do some work
            sleep(Duration::from_millis(500)).await;
            completed_steps = i + 1;

            if completed_steps % 4 == 0 {
                println!("   Progress: {}/{} steps", completed_steps, total_steps);
            }
        }

        // Clean up on completion
        if let Some(id) = &request_id {
            self.active_requests.lock().remove(id);
        }

        println!("✅ Operation completed successfully");

        Ok(CallToolResult {
            content: vec![json!({
                "status": "completed",
                "completed_steps": completed_steps,
                "total_steps": total_steps,
            })],
            is_error: false,
        })
    }

    async fn handle(&self, arguments: Value) -> pmcp::Result<CallToolResult> {
        self.handle_with_cancellation(arguments, None).await
    }

    fn cancel(&self, request_id: &RequestId) -> bool {
        if let Some(cancel_tx) = self.active_requests.lock().remove(request_id) {
            let _ = cancel_tx.send(());
            true
        } else {
            false
        }
    }
}

// Tool that demonstrates cleanup on cancellation
struct ResourceIntensiveTool;

#[async_trait]
impl ToolHandler for ResourceIntensiveTool {
    async fn handle_with_cancellation(
        &self,
        arguments: Value,
        request_id: Option<RequestId>,
    ) -> pmcp::Result<CallToolResult> {
        let resource_count = arguments
            .get("resources")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;

        println!("🔧 Allocating {} resources", resource_count);

        // Simulate resource allocation
        let mut allocated_resources = vec![];

        for i in 0..resource_count {
            // Check cancellation before allocating
            if pmcp::is_request_cancelled(&request_id).await {
                println!(
                    "🛑 Allocation cancelled, cleaning up {} resources",
                    allocated_resources.len()
                );

                // Clean up allocated resources
                for (idx, _) in allocated_resources.iter().enumerate() {
                    println!("   Releasing resource {}", idx + 1);
                    sleep(Duration::from_millis(100)).await;
                }

                return Ok(CallToolResult {
                    content: vec![json!({
                        "status": "cancelled",
                        "allocated": allocated_resources.len(),
                        "cleaned_up": allocated_resources.len(),
                    })],
                    is_error: false,
                });
            }

            // Allocate resource
            sleep(Duration::from_millis(500)).await;
            allocated_resources.push(format!("Resource-{}", i + 1));
            println!("   Allocated: {}", allocated_resources.last().unwrap());
        }

        // Process resources
        println!("💡 Processing {} resources", allocated_resources.len());

        for (i, resource) in allocated_resources.iter().enumerate() {
            if pmcp::is_request_cancelled(&request_id).await {
                println!("🛑 Processing cancelled at resource {}", i + 1);

                // Clean up all resources
                for (idx, res) in allocated_resources.iter().enumerate() {
                    println!("   Releasing {}", res);
                    sleep(Duration::from_millis(100)).await;
                }

                return Ok(CallToolResult {
                    content: vec![json!({
                        "status": "cancelled",
                        "processed": i,
                        "total": allocated_resources.len(),
                    })],
                    is_error: false,
                });
            }

            // Process resource
            sleep(Duration::from_millis(800)).await;
            println!("   Processed: {}", resource);
        }

        // Clean up normally
        println!("🧹 Cleaning up resources");
        for resource in &allocated_resources {
            println!("   Releasing {}", resource);
            sleep(Duration::from_millis(100)).await;
        }

        Ok(CallToolResult {
            content: vec![json!({
                "status": "completed",
                "processed": allocated_resources.len(),
                "resources": allocated_resources,
            })],
            is_error: false,
        })
    }

    async fn handle(&self, arguments: Value) -> pmcp::Result<CallToolResult> {
        self.handle_with_cancellation(arguments, None).await
    }
}

// Client demonstrating cancellation
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Request Cancellation Example ===\n");

    // Create client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    // Enable cancellation support
    let capabilities = ClientCapabilities {
        tools: Some(Default::default()),
        cancellation: Some(Default::default()),
        ..Default::default()
    };

    // Track cancelled requests
    let cancelled_requests = Arc::new(Mutex::new(Vec::<RequestId>::new()));
    let cancelled_clone = cancelled_requests.clone();

    client.on_cancelled(move |notification: CancelledNotification| {
        println!(
            "📢 Server confirmed cancellation of request: {:?}",
            notification.request_id
        );
        cancelled_clone.lock().push(notification.request_id);
    });

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // Example 1: Cancel a long-running operation
    println!("📋 Example 1: Cancelling a long-running operation\n");

    let request_id = RequestId::String(format!("req-{}", uuid::Uuid::new_v4()));
    let cancel_id = request_id.clone();

    // Start operation in background
    let client_clone = client.clone();
    let operation = tokio::spawn(async move {
        client_clone
            .call_tool_with_id(
                "cancellable_operation",
                json!({
                    "duration": 10  // 10 second operation
                }),
                request_id,
            )
            .await
    });

    // Cancel after 3 seconds
    tokio::spawn(async move {
        sleep(Duration::from_secs(3)).await;
        println!("\n🚫 Sending cancellation request...\n");

        match client.cancel_request(&cancel_id).await {
            Ok(_) => println!("✅ Cancellation request sent"),
            Err(e) => println!("❌ Failed to send cancellation: {}", e),
        }
    });

    // Wait for operation result
    match operation.await {
        Ok(Ok(result)) => {
            println!(
                "\n📦 Operation result: {}\n",
                serde_json::to_string_pretty(&result.content)?
            );
        },
        Ok(Err(e)) => {
            println!("\n❌ Operation error: {}", e);
        },
        Err(e) => {
            println!("\n❌ Task error: {}", e);
        },
    }

    sleep(Duration::from_millis(500)).await;

    // Example 2: Cancel during resource allocation
    println!("\n📋 Example 2: Cancellation with resource cleanup\n");

    let request_id = RequestId::String(format!("req-{}", uuid::Uuid::new_v4()));
    let cancel_id = request_id.clone();

    // Start resource-intensive operation
    let client_clone = client.clone();
    let operation = tokio::spawn(async move {
        client_clone
            .call_tool_with_id(
                "resource_intensive",
                json!({
                    "resources": 6
                }),
                request_id,
            )
            .await
    });

    // Cancel during allocation
    tokio::spawn(async move {
        sleep(Duration::from_millis(1800)).await; // Cancel after ~3 resources
        println!("\n🚫 Cancelling resource allocation...\n");
        client.cancel_request(&cancel_id).await.ok();
    });

    // Wait for result
    match operation.await {
        Ok(Ok(result)) => {
            println!(
                "\n📦 Operation result: {}\n",
                serde_json::to_string_pretty(&result.content)?
            );
        },
        Ok(Err(e)) => {
            println!("\n❌ Operation error: {}", e);
        },
        Err(e) => {
            println!("\n❌ Task error: {}", e);
        },
    }

    sleep(Duration::from_millis(500)).await;

    // Example 3: Multiple concurrent cancellations
    println!("\n📋 Example 3: Multiple concurrent operations with selective cancellation\n");

    let mut handles = vec![];
    let mut request_ids = vec![];

    // Start 4 operations
    for i in 0..4 {
        let request_id = RequestId::String(format!("concurrent-{}", i));
        request_ids.push(request_id.clone());

        let client_clone = client.clone();
        let handle = tokio::spawn(async move {
            let result = timeout(
                Duration::from_secs(10),
                client_clone.call_tool_with_id(
                    "cancellable_operation",
                    json!({
                        "duration": 8,
                        "operation_id": i
                    }),
                    request_id,
                ),
            )
            .await;

            (i, result)
        });

        handles.push(handle);
        sleep(Duration::from_millis(200)).await;
    }

    // Cancel operations 1 and 3
    sleep(Duration::from_secs(2)).await;
    println!("\n🚫 Cancelling operations 1 and 3...\n");
    client.cancel_request(&request_ids[1]).await.ok();
    client.cancel_request(&request_ids[3]).await.ok();

    // Wait for all operations
    for handle in handles {
        match handle.await {
            Ok((id, Ok(Ok(result)))) => {
                let status = result.content[0]
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!("Operation {}: {}", id, status);
            },
            Ok((id, _)) => {
                println!("Operation {}: error or timeout", id);
            },
            Err(e) => {
                println!("Task error: {}", e);
            },
        }
    }

    // Show cancellation summary
    println!("\n📊 Cancellation Summary:");
    println!("   Total cancelled: {}", cancelled_requests.lock().len());
    for id in cancelled_requests.lock().iter() {
        println!("   - {:?}", id);
    }

    Ok(())
}
