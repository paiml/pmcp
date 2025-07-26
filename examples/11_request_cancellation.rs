//! Request Cancellation Example
//!
//! This example demonstrates the concept of request cancellation in MCP.
//! It shows how cancellation tokens and notifications work.

use pmcp::types::{CancelledNotification, RequestId};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== MCP Request Cancellation Example ===");

    // Simulate a long-running operation with cancellation
    let request_id = RequestId::Number(42);

    println!("🚀 Starting long-running operation (ID: {:?})", request_id);

    // Simulate the operation with potential cancellation
    let operation_task = tokio::spawn(async move {
        for i in 1..=10 {
            println!("📊 Operation progress: {}/10", i);
            sleep(Duration::from_millis(500)).await;

            // Simulate cancellation after 3 iterations
            if i == 3 {
                println!("⚠️  Cancellation requested!");

                // Create a cancellation notification
                let cancellation = CancelledNotification {
                    request_id: request_id.clone(),
                    reason: Some("User requested cancellation".to_string()),
                };

                println!("📢 Cancellation notification: {:?}", cancellation);
                return Err("Operation cancelled");
            }
        }

        Ok("Operation completed successfully")
    });

    // Wait for the operation to complete or be cancelled
    match operation_task.await {
        Ok(Ok(result)) => {
            println!("✅ {}", result);
        },
        Ok(Err(error)) => {
            println!("❌ {}", error);
        },
        Err(join_error) => {
            println!("💥 Task failed: {}", join_error);
        },
    }

    println!("🔚 Request cancellation example completed!");

    Ok(())
}
