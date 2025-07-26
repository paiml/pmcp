//! Integration tests for new features in v0.2.0

use pmcp::*;
use std::sync::Arc;

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_transport() {
    use pmcp::{WebSocketConfig, WebSocketTransport};
    use url::Url;

    let config = WebSocketConfig {
        url: Url::parse("ws://localhost:8080").unwrap(),
        auto_reconnect: true,
        ..Default::default()
    };

    let transport = WebSocketTransport::new(config);
    assert!(!transport.is_connected());
}

#[cfg(feature = "http")]
#[tokio::test]
async fn test_http_transport() {
    use pmcp::{HttpConfig, HttpTransport};
    use url::Url;

    let config = HttpConfig {
        base_url: Url::parse("http://localhost:8080").unwrap(),
        sse_endpoint: Some("/events".to_string()),
        ..Default::default()
    };

    let transport = HttpTransport::new(config);
    assert!(!transport.is_connected());
}

#[tokio::test]
async fn test_sampling_client_method() {
    use pmcp::{Client, Content, CreateMessageRequest, Role, SamplingMessage, StdioTransport};

    let transport = StdioTransport::new();
    let _client = Client::new(transport);

    // Would need initialized client to actually test
    let request = CreateMessageRequest {
        messages: vec![SamplingMessage {
            role: Role::User,
            content: Content::Text {
                text: "Test message".to_string(),
            },
        }],
        model_preferences: None,
        system_prompt: None,
        include_context: pmcp::types::IncludeContext::default(),
        temperature: None,
        max_tokens: None,
        stop_sequences: None,
        metadata: None,
    };

    // Verify request can be created
    assert_eq!(request.messages.len(), 1);
}

#[tokio::test]
async fn test_sampling_server_handler() {
    use async_trait::async_trait;
    use pmcp::{
        Content, CreateMessageParams, CreateMessageResult, SamplingHandler, Server, TokenUsage,
    };

    struct TestSampling;

    #[async_trait]
    impl SamplingHandler for TestSampling {
        async fn create_message(
            &self,
            _params: CreateMessageParams,
        ) -> pmcp::Result<CreateMessageResult> {
            Ok(CreateMessageResult {
                content: Content::Text {
                    text: "Test response".to_string(),
                },
                model: "test-model".to_string(),
                usage: Some(TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                }),
                stop_reason: None,
            })
        }
    }

    let _server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .sampling(TestSampling)
        .build()
        .unwrap();

    // Server should have sampling capability configured when handler is added
}

#[tokio::test]
async fn test_authentication() {
    use pmcp::{AuthInfo, AuthScheme, Client, StdioTransport};

    let transport = StdioTransport::new();
    let _client = Client::new(transport);

    let auth = AuthInfo {
        scheme: AuthScheme::Bearer,
        token: Some("test-token".to_string()),
        oauth: None,
        params: std::collections::HashMap::default(),
    };

    // Would need initialized client to actually authenticate
    assert!(auth.token.is_some());
}

#[tokio::test]
async fn test_middleware() {
    use pmcp::types::{JSONRPCRequest, RequestId};
    use pmcp::{AuthMiddleware, LoggingMiddleware, MiddlewareChain};

    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(LoggingMiddleware::default()));
    chain.add(Arc::new(AuthMiddleware::new("test-token".to_string())));

    let mut request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::from(1i64),
        method: "test".to_string(),
        params: None,
    };

    assert!(chain.process_request(&mut request).await.is_ok());
}

#[tokio::test]
async fn test_message_batching() {
    use pmcp::types::{ClientNotification, Notification};
    use pmcp::{BatchingConfig, MessageBatcher};
    use std::time::Duration;

    let config = BatchingConfig {
        max_batch_size: 2,
        max_wait_time: Duration::from_millis(10),
        batched_methods: vec![],
    };

    let batcher = MessageBatcher::new(config);
    batcher.start_timer();

    let notif1 = Notification::Client(ClientNotification::Initialized);
    let notif2 = Notification::Client(ClientNotification::RootsListChanged);

    batcher.add(notif1).await.unwrap();
    batcher.add(notif2).await.unwrap();

    // Should batch messages
    let batch = batcher.receive_batch().await;
    assert!(batch.is_some());
    assert_eq!(batch.unwrap().len(), 2);
}

#[tokio::test]
async fn test_message_debouncing() {
    use pmcp::types::{ClientNotification, Notification};
    use pmcp::{DebouncingConfig, MessageDebouncer};
    use std::time::Duration;

    let config = DebouncingConfig {
        wait_time: Duration::from_millis(10),
        debounced_methods: std::collections::HashMap::default(),
    };

    let debouncer = MessageDebouncer::new(config);

    let notif1 = Notification::Client(ClientNotification::Initialized);
    let notif2 = Notification::Client(ClientNotification::RootsListChanged);

    // Add with same key - second should override first
    debouncer.add("test".to_string(), notif1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    debouncer.add("test".to_string(), notif2).await.unwrap();

    // Should only get the last notification
    let received = debouncer.receive().await;
    assert!(received.is_some());
}

#[tokio::test]
async fn test_retry_middleware() {
    use pmcp::shared::Middleware;
    use pmcp::types::{JSONRPCRequest, RequestId};
    use pmcp::RetryMiddleware;

    let retry = RetryMiddleware::default();

    let mut request = JSONRPCRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::from(1i64),
        method: "test".to_string(),
        params: None,
    };

    assert!(retry.on_request(&mut request).await.is_ok());
}
