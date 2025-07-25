//! Example: Error handling patterns in MCP
//!
//! This example demonstrates:
//! - Different error types and codes
//! - Error recovery strategies
//! - Retry logic with backoff
//! - Custom error handling

use async_trait::async_trait;
use pmcp::{Client, ClientCapabilities, Error, ErrorCode, StdioTransport, ToolHandler};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Tool that demonstrates various error scenarios
#[allow(dead_code)]
struct ErrorDemoTool {
    call_count: Arc<AtomicU32>,
}

#[allow(dead_code)]
impl ErrorDemoTool {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[async_trait]
impl ToolHandler for ErrorDemoTool {
    async fn handle(&self, arguments: Value) -> pmcp::Result<Value> {
        let scenario = arguments
            .get("scenario")
            .and_then(|v| v.as_str())
            .unwrap_or("success");

        match scenario {
            "parse_error" => {
                // Simulate invalid JSON parsing
                Err(Error::parse("Invalid JSON: expected object, got array"))
            },

            "invalid_request" => {
                // Missing required parameters
                Err(Error::invalid_request("Missing required parameter 'input'"))
            },

            "method_not_found" => {
                // Unknown method
                Err(Error::method_not_found("tools/unknown"))
            },

            "invalid_params" => {
                // Parameter validation failure
                Err(Error::invalid_params("Parameter 'count' must be positive"))
            },

            "internal_error" => {
                // Server-side error
                Err(Error::internal("Database connection failed"))
            },

            "timeout" => {
                // Simulate timeout
                sleep(Duration::from_secs(30)).await;
                Ok(json!({"status": "should_timeout"}))
            },

            "rate_limit" => {
                // Simulate rate limiting
                Err(Error::protocol_with_data(
                    ErrorCode::other(-32001),
                    "Rate limit exceeded",
                    json!({
                        "retry_after": 60,
                        "limit": 100,
                        "window": "1h"
                    }),
                ))
            },

            "transient" => {
                // Fail first 2 calls, succeed on 3rd
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(Error::internal("Temporary failure, please retry"))
                } else {
                    Ok(json!({
                        "status": "success",
                        "attempts": count + 1
                    }))
                }
            },

            "validation" => {
                // Business logic validation error
                let input = arguments
                    .get("input")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if input.len() < 5 {
                    Err(Error::validation(
                        "Input must be at least 5 characters long",
                    ))
                } else if !input.chars().all(|c| c.is_alphanumeric()) {
                    Err(Error::validation(
                        "Input must contain only alphanumeric characters",
                    ))
                } else {
                    Ok(json!({
                        "status": "validated",
                        "input": input
                    }))
                }
            },

            _ => {
                // Success case
                Ok(json!({
                    "status": "success",
                    "scenario": scenario
                }))
            },
        }
    }
}

// Retry logic with exponential backoff
async fn retry_with_backoff<F, T>(
    mut operation: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, Error>
where
    F: FnMut() -> futures::future::BoxFuture<'static, Result<T, Error>>,
{
    let mut delay = initial_delay;
    let mut last_error = None;

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Check if error is retryable
                let msg = e.to_string().to_lowercase();
                let is_retryable = match &e {
                    Error::Transport(_) => true,
                    _ => {
                        msg.contains("timeout")
                            || msg.contains("unavailable")
                            || msg.contains("temporary")
                            || msg.contains("retry")
                    },
                };

                if !is_retryable {
                    return Err(e);
                }

                last_error = Some(e);

                if attempt < max_retries {
                    println!(
                        "⏳ Attempt {} failed. Retrying in {:?}...",
                        attempt + 1,
                        delay
                    );
                    sleep(delay).await;
                    delay *= 2; // Exponential backoff
                }
            },
        }
    }

    Err(last_error.unwrap_or_else(|| Error::internal("All retry attempts failed")))
}

// Client with comprehensive error handling
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Error Handling Example ===\n");

    // Create client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);

    let capabilities = ClientCapabilities {
        tools: Some(Default::default()),
        ..Default::default()
    };

    println!("Connecting to server...");
    let _server_info = client.initialize(capabilities).await?;
    println!("✅ Connected!\n");

    // Example 1: Handle different error types
    println!("📋 Example 1: Different error types\n");

    let error_scenarios = vec![
        ("parse_error", "Parse Error"),
        ("invalid_request", "Invalid Request"),
        ("method_not_found", "Method Not Found"),
        ("invalid_params", "Invalid Parameters"),
        ("internal_error", "Internal Server Error"),
        ("rate_limit", "Rate Limiting"),
    ];

    for (scenario, description) in error_scenarios {
        print!("Testing {}: ", description);

        match client
            .call_tool("error_demo".to_string(), json!({"scenario": scenario}))
            .await
        {
            Ok(_) => println!("✅ Unexpected success"),
            Err(e) => {
                println!("❌ {}", e);

                // Analyze error
                if let Some(code) = e.error_code() {
                    println!("   Error code: {:?}", code);
                }

                if let Error::Protocol {
                    data: Some(data), ..
                } = &e
                {
                    println!(
                        "   Additional data: {}",
                        serde_json::to_string_pretty(data)?
                    );
                }
            },
        }
        println!();
    }

    // Example 2: Validation errors
    println!("\n📋 Example 2: Input validation\n");

    let test_inputs = vec![
        ("abc", "Too short"),
        ("hello123", "Valid input"),
        ("hello world!", "Invalid characters"),
        ("valid1234", "Another valid input"),
    ];

    for (input, description) in test_inputs {
        print!("Testing '{}' ({}): ", input, description);

        match client
            .call_tool(
                "error_demo".to_string(),
                json!({
                    "scenario": "validation",
                    "input": input
                }),
            )
            .await
        {
            Ok(result) => {
                println!(
                    "✅ Result: {}",
                    serde_json::to_string_pretty(&result.content)?
                );
            },
            Err(e) => {
                println!("❌ {}", e);
            },
        }
    }

    // Example 3: Retry with backoff
    println!("\n\n📋 Example 3: Retry with exponential backoff\n");

    let client_clone = client.clone();
    let result = retry_with_backoff(
        || {
            let client = client_clone.clone();
            Box::pin(async move {
                client
                    .call_tool("error_demo".to_string(), json!({"scenario": "transient"}))
                    .await
            })
        },
        3,
        Duration::from_millis(500),
    )
    .await;

    match result {
        Ok(res) => {
            println!(
                "\n✅ Success after retries: {}",
                serde_json::to_string_pretty(&res.content)?
            );
        },
        Err(e) => {
            println!("\n❌ Failed after all retries: {}", e);
        },
    }

    // Example 4: Timeout handling
    println!("\n\n📋 Example 4: Timeout handling\n");

    print!("Testing timeout (5s limit): ");
    let start = std::time::Instant::now();

    match tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("error_demo".to_string(), json!({"scenario": "timeout"})),
    )
    .await
    {
        Ok(Ok(_)) => println!("✅ Unexpected success"),
        Ok(Err(e)) => println!("❌ Error: {}", e),
        Err(_) => {
            let elapsed = start.elapsed();
            println!("⏱️  Timed out after {:?}", elapsed);
        },
    }

    // Example 5: Error recovery strategies
    println!("\n\n📋 Example 5: Error recovery strategies\n");

    // Strategy 1: Fallback
    println!("Strategy 1 - Fallback:");
    let primary_result = client
        .call_tool(
            "error_demo".to_string(),
            json!({
                "scenario": "internal_error"
            }),
        )
        .await;

    match primary_result {
        Ok(res) => {
            println!("   Primary succeeded: {:?}", res.content);
        },
        Err(e) => {
            println!("   Primary failed: {}", e);
            println!("   Trying fallback...");

            // Try simpler operation
            match client
                .call_tool("error_demo".to_string(), json!({"scenario": "success"}))
                .await
            {
                Ok(_) => println!("   ✅ Fallback succeeded"),
                Err(e) => println!("   ❌ Fallback also failed: {}", e),
            }
        },
    }

    // Strategy 2: Circuit breaker pattern
    println!("\nStrategy 2 - Circuit breaker:");
    let mut failures = 0;
    let failure_threshold = 3;
    let mut circuit_open = false;

    for i in 0..5 {
        if circuit_open {
            println!("   Attempt {}: Circuit open, skipping", i + 1);
            continue;
        }

        match client
            .call_tool(
                "error_demo".to_string(),
                json!({
                    "scenario": if i < 3 { "internal_error" } else { "success" }
                }),
            )
            .await
        {
            Ok(_) => {
                println!("   Attempt {}: ✅ Success", i + 1);
                failures = 0; // Reset on success
            },
            Err(e) => {
                failures += 1;
                println!("   Attempt {}: ❌ Failed ({})", i + 1, e);

                if failures >= failure_threshold {
                    circuit_open = true;
                    println!("   🚫 Circuit breaker opened!");
                }
            },
        }
    }

    // Example 6: Error aggregation
    println!("\n\n📋 Example 6: Batch operations with error aggregation\n");

    let operations = vec![
        ("success", "Operation 1"),
        ("invalid_params", "Operation 2"),
        ("success", "Operation 3"),
        ("internal_error", "Operation 4"),
        ("success", "Operation 5"),
    ];

    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (scenario, name) in operations {
        match client
            .call_tool("error_demo".to_string(), json!({"scenario": scenario}))
            .await
        {
            Ok(res) => results.push((name, res)),
            Err(e) => errors.push((name, e)),
        }
    }

    println!("Batch results:");
    println!("   ✅ Successful: {} operations", results.len());
    for (name, _) in &results {
        println!("      - {}", name);
    }

    println!("   ❌ Failed: {} operations", errors.len());
    for (name, error) in &errors {
        println!("      - {}: {}", name, error);
    }

    #[allow(clippy::cast_precision_loss)]
    let success_rate = (results.len() as f64 / (results.len() + errors.len()) as f64) * 100.0;
    println!("\n   Success rate: {:.1}%", success_rate);

    Ok(())
}
