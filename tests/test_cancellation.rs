//! Tests for request cancellation functionality.

use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

/// A tool that respects cancellation.
#[derive(Clone)]
struct CancellableToolHandler;

#[async_trait::async_trait]
impl ToolHandler for CancellableToolHandler {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Simulate a long-running operation
        tokio::select! {
            () = sleep(Duration::from_secs(5)) => {
                Ok(json!({ "result": "completed" }))
            }
            () = extra.cancelled() => {
                Err(pmcp::Error::cancelled("Operation cancelled"))
            }
        }
    }
}

/// A tool that checks cancellation periodically.
#[derive(Clone)]
struct PeriodicCheckToolHandler;

#[async_trait::async_trait]
impl ToolHandler for PeriodicCheckToolHandler {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        for i in 0..10 {
            if extra.is_cancelled() {
                return Err(pmcp::Error::cancelled(format!(
                    "Operation cancelled at step {}",
                    i
                )));
            }
            sleep(Duration::from_millis(100)).await;
        }
        Ok(json!({ "result": "completed all steps" }))
    }
}

#[tokio::test]
async fn test_cancel_request_from_server() {
    let server = Server::builder()
        .name("test-cancellation-server")
        .version("1.0.0")
        .tool("slow-tool", CancellableToolHandler)
        .build()
        .unwrap();

    // Cancel a request with reason
    server
        .cancel_request(
            "test-request-123".to_string(),
            Some("Test cancellation".to_string()),
        )
        .await
        .unwrap();

    // Cancel without reason
    server
        .cancel_request("test-request-456".to_string(), None)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_multiple_cancellations() {
    let server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .build()
        .unwrap();

    // Cancel multiple requests
    server
        .cancel_request("request-1".to_string(), Some("Reason 1".to_string()))
        .await
        .unwrap();

    server
        .cancel_request("request-2".to_string(), None)
        .await
        .unwrap();

    // These operations should complete without error
}

#[tokio::test]
async fn test_request_handler_extra() {
    let token = CancellationToken::new();
    let extra = RequestHandlerExtra::new("test-req".to_string(), token.clone())
        .with_session_id(Some("session-123".to_string()));

    assert_eq!(extra.request_id, "test-req");
    assert_eq!(extra.session_id, Some("session-123".to_string()));
    assert!(!extra.is_cancelled());

    // Cancel the token
    token.cancel();
    assert!(extra.is_cancelled());

    // Test waiting for cancellation
    let extra_clone = extra.clone();
    let handle = tokio::spawn(async move {
        extra_clone.cancelled().await;
        true
    });

    // Should complete immediately since already cancelled
    let result = tokio::time::timeout(Duration::from_millis(100), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_tool_respects_cancellation() {
    let handler = CancellableToolHandler;
    let token = CancellationToken::new();
    let extra = RequestHandlerExtra::new("test-req".to_string(), token.clone());

    // Start the tool
    let handle = tokio::spawn(async move { handler.handle(json!({}), extra).await });

    // Cancel after a short delay
    sleep(Duration::from_millis(100)).await;
    token.cancel();

    // Tool should return an error
    let result = handle.await.unwrap();
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("cancelled"));
    }
}

#[tokio::test]
async fn test_periodic_check_tool() {
    let handler = PeriodicCheckToolHandler;
    let token = CancellationToken::new();
    let extra = RequestHandlerExtra::new("test-req".to_string(), token.clone());

    // Start the tool
    let handle = tokio::spawn(async move { handler.handle(json!({}), extra).await });

    // Cancel after a few steps
    sleep(Duration::from_millis(350)).await;
    token.cancel();

    // Tool should return an error
    let result = handle.await.unwrap();
    assert!(result.is_err());
}
