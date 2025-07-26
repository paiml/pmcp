//! Protocol helper functions for parsing and creating messages.

use crate::error::{Error, Result};
use crate::types::{
    ClientNotification, ClientRequest, JSONRPCNotification, JSONRPCRequest, Notification, Request,
    RequestId, ServerNotification, ServerRequest,
};
use serde_json::Value;

/// Parse a JSON-RPC request into a typed Request.
pub fn parse_request(request: JSONRPCRequest<Value>) -> Result<(RequestId, Request)> {
    let id = request.id;
    let method = &request.method;
    let params = request.params.unwrap_or(Value::Null);

    // Try to parse as client request first
    if let Ok(client_req) = parse_client_request(method, &params) {
        return Ok((id, Request::Client(client_req)));
    }

    // Try to parse as server request
    if let Ok(server_req) = parse_server_request(method, &params) {
        return Ok((id, Request::Server(server_req)));
    }

    Err(Error::method_not_found(method))
}

/// Parse a notification from JSON.
pub fn parse_notification(value: Value) -> Result<Notification> {
    let notification: JSONRPCNotification<Value> = serde_json::from_value(value)
        .map_err(|e| Error::parse(format!("Invalid notification: {}", e)))?;

    let method = &notification.method;
    let params = notification.params.unwrap_or(Value::Null);

    // Check for special notification types
    if method == "notifications/progress" {
        let progress = serde_json::from_value(params)
            .map_err(|e| Error::parse(format!("Invalid progress notification: {}", e)))?;
        return Ok(Notification::Progress(progress));
    }

    if method == "notifications/cancelled" {
        let cancelled = serde_json::from_value(params)
            .map_err(|e| Error::parse(format!("Invalid cancelled notification: {}", e)))?;
        return Ok(Notification::Cancelled(cancelled));
    }

    // Try to parse as client notification
    if let Ok(client_notif) = parse_client_notification(method, &params) {
        return Ok(Notification::Client(client_notif));
    }

    // Try to parse as server notification
    if let Ok(server_notif) = parse_server_notification(method, &params) {
        return Ok(Notification::Server(server_notif));
    }

    Err(Error::method_not_found(method))
}

/// Create a JSON-RPC request from typed request.
pub fn create_request(id: RequestId, request: Request) -> JSONRPCRequest<Value> {
    match request {
        Request::Client(client_req) => {
            let (method, params) = client_request_to_jsonrpc(client_req);
            JSONRPCRequest::new(id, method, params)
        },
        Request::Server(server_req) => {
            let (method, params) = server_request_to_jsonrpc(server_req);
            JSONRPCRequest::new(id, method, params)
        },
    }
}

/// Create a JSON-RPC notification from typed notification.
///
/// # Panics
///
/// Panics if serialization to JSON fails (should never happen with valid MCP types).
pub fn create_notification(notification: Notification) -> JSONRPCNotification<Value> {
    match notification {
        Notification::Client(client_notif) => {
            let (method, params) = client_notification_to_jsonrpc(client_notif);
            JSONRPCNotification::new(method, params)
        },
        Notification::Server(server_notif) => {
            let (method, params) = server_notification_to_jsonrpc(server_notif);
            JSONRPCNotification::new(method, params)
        },
        Notification::Progress(progress) => JSONRPCNotification::new(
            "notifications/progress",
            Some(serde_json::to_value(progress).unwrap()),
        ),
        Notification::Cancelled(cancelled) => JSONRPCNotification::new(
            "notifications/cancelled",
            Some(serde_json::to_value(cancelled).unwrap()),
        ),
    }
}

// Helper functions for parsing

fn parse_client_request(method: &str, params: &Value) -> Result<ClientRequest> {
    let request_json = serde_json::json!({
        "method": method,
        "params": params,
    });

    serde_json::from_value(request_json)
        .map_err(|e| Error::parse(format!("Invalid client request: {}", e)))
}

fn parse_server_request(method: &str, params: &Value) -> Result<ServerRequest> {
    let request_json = serde_json::json!({
        "method": method,
        "params": params,
    });

    serde_json::from_value(request_json)
        .map_err(|e| Error::parse(format!("Invalid server request: {}", e)))
}

fn parse_client_notification(method: &str, params: &Value) -> Result<ClientNotification> {
    let notif_json = serde_json::json!({
        "method": method,
        "params": params,
    });

    serde_json::from_value(notif_json)
        .map_err(|e| Error::parse(format!("Invalid client notification: {}", e)))
}

fn parse_server_notification(method: &str, params: &Value) -> Result<ServerNotification> {
    let notif_json = serde_json::json!({
        "method": method,
        "params": params,
    });

    serde_json::from_value(notif_json)
        .map_err(|e| Error::parse(format!("Invalid server notification: {}", e)))
}

fn client_request_to_jsonrpc(req: ClientRequest) -> (String, Option<Value>) {
    match req {
        // Core protocol requests
        ClientRequest::Initialize(params) => create_method_params("initialize", params),
        ClientRequest::Ping => ("ping".to_string(), None),
        ClientRequest::SetLoggingLevel { level } => (
            "logging/setLevel".to_string(),
            Some(serde_json::json!({"level": level})),
        ),
        // Tool requests
        ClientRequest::ListTools(params) => create_method_params("tools/list", params),
        ClientRequest::CallTool(params) => create_method_params("tools/call", params),
        // Prompt requests
        ClientRequest::ListPrompts(params) => create_method_params("prompts/list", params),
        ClientRequest::GetPrompt(params) => create_method_params("prompts/get", params),
        // Resource requests
        ClientRequest::ListResources(params) => create_method_params("resources/list", params),
        ClientRequest::ListResourceTemplates(params) => {
            create_method_params("resources/templates/list", params)
        },
        ClientRequest::ReadResource(params) => create_method_params("resources/read", params),
        ClientRequest::Subscribe(params) => create_method_params("resources/subscribe", params),
        ClientRequest::Unsubscribe(params) => create_method_params("resources/unsubscribe", params),
        // Completion requests
        ClientRequest::Complete(params) => create_method_params("completion/complete", params),
        // Sampling requests
        ClientRequest::CreateMessage(params) => {
            create_method_params("sampling/createMessage", params)
        },
    }
}

/// Helper function to create method and params tuple.
fn create_method_params<T: serde::Serialize>(method: &str, params: T) -> (String, Option<Value>) {
    (
        method.to_string(),
        Some(serde_json::to_value(params).unwrap()),
    )
}

fn server_request_to_jsonrpc(req: ServerRequest) -> (String, Option<Value>) {
    match req {
        ServerRequest::CreateMessage(params) => (
            "sampling/createMessage".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
    }
}

fn client_notification_to_jsonrpc(notif: ClientNotification) -> (String, Option<Value>) {
    match notif {
        ClientNotification::Initialized => ("notifications/initialized".to_string(), None),
        ClientNotification::RootsListChanged => {
            ("notifications/roots/list_changed".to_string(), None)
        },
        ClientNotification::Cancelled(params) => (
            "notifications/cancelled".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientNotification::Progress(params) => (
            "notifications/progress".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
    }
}

fn server_notification_to_jsonrpc(notif: ServerNotification) -> (String, Option<Value>) {
    match notif {
        ServerNotification::Progress(params) => (
            "notifications/progress".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ServerNotification::ToolsChanged => ("notifications/tools/list_changed".to_string(), None),
        ServerNotification::PromptsChanged => {
            ("notifications/prompts/list_changed".to_string(), None)
        },
        ServerNotification::ResourcesChanged => {
            ("notifications/resources/list_changed".to_string(), None)
        },
        ServerNotification::ResourceUpdated(params) => (
            "notifications/resources/updated".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ServerNotification::LogMessage(params) => (
            "notifications/message".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CallToolRequest, CancelledNotification, ClientCapabilities, CompleteRequest,
        CompletionArgument, CompletionReference, GetPromptRequest, Implementation,
        InitializeRequest, ListPromptsRequest, ListResourceTemplatesRequest, ListResourcesRequest,
        ListToolsRequest, LoggingLevel, Progress, ProgressNotification, ProgressToken,
        ReadResourceRequest, SubscribeRequest, UnsubscribeRequest,
    };
    use serde_json::json;

    #[test]
    fn test_parse_client_request_initialize() {
        let id = RequestId::from(1i64);
        let method = "initialize";
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ClientRequest::Initialize(_)) => (),
            _ => panic!("Expected Initialize request"),
        }
    }

    #[test]
    fn test_parse_client_request_list_tools() {
        let id = RequestId::from(2i64);
        let method = "tools/list";
        let params = json!({ "cursor": null });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ClientRequest::ListTools(_)) => (),
            _ => panic!("Expected ListTools request"),
        }
    }

    #[test]
    fn test_parse_client_request_call_tool() {
        let id = RequestId::from(3i64);
        let method = "tools/call";
        let params = json!({
            "name": "test-tool",
            "arguments": {"input": "test"}
        });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ClientRequest::CallTool(_)) => (),
            _ => panic!("Expected CallTool request"),
        }
    }

    #[test]
    fn test_parse_client_request_ping() {
        let id = RequestId::from(4i64);
        let method = "ping";

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), None);
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ClientRequest::Ping) => (),
            _ => panic!("Expected Ping request"),
        }
    }

    #[test]
    fn test_parse_server_request_create_message() {
        let id = RequestId::from(5i64);
        let method = "sampling/createMessage";
        let params = json!({
            "messages": [],
            "includeContext": "none"
        });

        let request = JSONRPCRequest::new(id.clone(), method.to_string(), Some(params));
        let result = parse_request(request).unwrap();

        assert_eq!(result.0, id);
        match result.1 {
            Request::Client(ClientRequest::CreateMessage(_)) => (),
            _ => panic!("Expected CreateMessage request"),
        }
    }

    #[test]
    fn test_parse_request_unknown_method() {
        let id = RequestId::from(6i64);
        let method = "unknown/method";

        let request = JSONRPCRequest::new(id, method.to_string(), None);
        let result = parse_request(request);

        assert!(result.is_err());
        let error_str = result.unwrap_err().to_string();
        assert!(error_str.contains("Method not found"));
    }

    #[test]
    fn test_parse_notification_progress() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": {
                "progressToken": "test-token",
                "progress": 50.0,
                "message": "Processing..."
            }
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Progress(progress) => {
                assert_eq!(
                    progress.progress_token,
                    ProgressToken::String("test-token".to_string())
                );
                assert!((progress.progress - 50.0).abs() < f64::EPSILON);
                assert_eq!(progress.message, Some("Processing...".to_string()));
            },
            _ => panic!("Expected Progress notification"),
        }
    }

    #[test]
    fn test_parse_notification_cancelled() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {
                "requestId": "test-request",
                "reason": "User cancelled"
            }
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Cancelled(cancelled) => {
                assert_eq!(
                    cancelled.request_id,
                    RequestId::String("test-request".to_string())
                );
                assert_eq!(cancelled.reason, Some("User cancelled".to_string()));
            },
            _ => panic!("Expected Cancelled notification"),
        }
    }

    #[test]
    fn test_parse_client_notification_initialized() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Client(ClientNotification::Initialized) => (),
            _ => panic!("Expected Initialized notification"),
        }
    }

    #[test]
    fn test_parse_server_notification_tools_changed() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed"
        });

        let result = parse_notification(notification_json).unwrap();
        match result {
            Notification::Server(ServerNotification::ToolsChanged) => (),
            _ => panic!("Expected ToolsChanged notification"),
        }
    }

    #[test]
    fn test_parse_notification_unknown_method() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "unknown/notification"
        });

        let result = parse_notification(notification_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[test]
    fn test_parse_notification_invalid_json() {
        let invalid_json = json!("not a notification");
        let result = parse_notification(invalid_json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid notification"));
    }

    #[test]
    fn test_create_client_request_initialize() {
        let id = RequestId::from(1i64);
        let request = Request::Client(ClientRequest::Initialize(InitializeRequest {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        }));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "initialize");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_client_request_list_tools() {
        let id = RequestId::from(2i64);
        let request = Request::Client(ClientRequest::ListTools(ListToolsRequest { cursor: None }));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "tools/list");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_client_request_call_tool() {
        let id = RequestId::from(3i64);
        let request = Request::Client(ClientRequest::CallTool(CallToolRequest {
            name: "test-tool".to_string(),
            arguments: json!({"input": "test"}),
        }));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "tools/call");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_client_request_ping() {
        let id = RequestId::from(4i64);
        let request = Request::Client(ClientRequest::Ping);

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "ping");
        assert!(jsonrpc_request.params.is_none());
    }

    #[test]
    fn test_create_client_request_set_logging_level() {
        let id = RequestId::from(5i64);
        let request = Request::Client(ClientRequest::SetLoggingLevel {
            level: LoggingLevel::Debug,
        });

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "logging/setLevel");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_server_request_create_message() {
        let id = RequestId::from(6i64);
        let request = Request::Server(ServerRequest::CreateMessage(
            crate::types::protocol::CreateMessageParams {
                messages: vec![],
                model_preferences: None,
                system_prompt: None,
                include_context: crate::types::protocol::IncludeContext::None,
                temperature: None,
                max_tokens: None,
                stop_sequences: None,
                metadata: None,
            },
        ));

        let jsonrpc_request = create_request(id.clone(), request);
        assert_eq!(jsonrpc_request.id, id);
        assert_eq!(jsonrpc_request.method, "sampling/createMessage");
        assert!(jsonrpc_request.params.is_some());
    }

    #[test]
    fn test_create_notification_client_initialized() {
        let notification = Notification::Client(ClientNotification::Initialized);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/initialized");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_client_roots_list_changed() {
        let notification = Notification::Client(ClientNotification::RootsListChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/roots/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_progress() {
        let progress = ProgressNotification {
            progress_token: ProgressToken::String("test".to_string()),
            progress: 75.0,
            message: Some("Almost done".to_string()),
        };
        let notification = Notification::Progress(progress);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/progress");
        assert!(jsonrpc_notif.params.is_some());
    }

    #[test]
    fn test_create_notification_cancelled() {
        let cancelled = CancelledNotification {
            request_id: RequestId::String("test-req".to_string()),
            reason: Some("Timeout".to_string()),
        };
        let notification = Notification::Cancelled(cancelled);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/cancelled");
        assert!(jsonrpc_notif.params.is_some());
    }

    #[test]
    fn test_create_notification_server_tools_changed() {
        let notification = Notification::Server(ServerNotification::ToolsChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/tools/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_server_prompts_changed() {
        let notification = Notification::Server(ServerNotification::PromptsChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/prompts/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_create_notification_server_resources_changed() {
        let notification = Notification::Server(ServerNotification::ResourcesChanged);
        let jsonrpc_notif = create_notification(notification);
        assert_eq!(jsonrpc_notif.method, "notifications/resources/list_changed");
        assert!(jsonrpc_notif.params.is_none());
    }

    #[test]
    fn test_client_request_to_jsonrpc_all_variants() {
        // Test all ClientRequest variants to ensure complete coverage
        let test_cases = vec![
            (
                ClientRequest::ListPrompts(ListPromptsRequest { cursor: None }),
                "prompts/list",
            ),
            (
                ClientRequest::GetPrompt(GetPromptRequest {
                    name: "test".to_string(),
                    arguments: std::collections::HashMap::new(),
                }),
                "prompts/get",
            ),
            (
                ClientRequest::ListResources(ListResourcesRequest { cursor: None }),
                "resources/list",
            ),
            (
                ClientRequest::ListResourceTemplates(ListResourceTemplatesRequest { cursor: None }),
                "resources/templates/list",
            ),
            (
                ClientRequest::ReadResource(ReadResourceRequest {
                    uri: "test://uri".to_string(),
                }),
                "resources/read",
            ),
            (
                ClientRequest::Subscribe(SubscribeRequest {
                    uri: "test://uri".to_string(),
                }),
                "resources/subscribe",
            ),
            (
                ClientRequest::Unsubscribe(UnsubscribeRequest {
                    uri: "test://uri".to_string(),
                }),
                "resources/unsubscribe",
            ),
            (
                ClientRequest::Complete(CompleteRequest {
                    r#ref: CompletionReference::Resource {
                        uri: "test://uri".to_string(),
                    },
                    argument: CompletionArgument {
                        name: "test".to_string(),
                        value: "val".to_string(),
                    },
                }),
                "completion/complete",
            ),
        ];

        for (request, expected_method) in test_cases {
            let (method, params) = client_request_to_jsonrpc(request);
            assert_eq!(method, expected_method);
            assert!(params.is_some());
        }
    }

    #[test]
    fn test_client_notification_to_jsonrpc_all_variants() {
        let cancelled = CancelledNotification {
            request_id: RequestId::String("test".to_string()),
            reason: None,
        };
        let progress = ProgressNotification {
            progress_token: ProgressToken::String("test".to_string()),
            progress: 50.0,
            message: None,
        };

        let test_cases = vec![
            (
                ClientNotification::Cancelled(cancelled),
                "notifications/cancelled",
                true,
            ),
            (
                ClientNotification::Progress(progress),
                "notifications/progress",
                true,
            ),
        ];

        for (notification, expected_method, should_have_params) in test_cases {
            let (method, params) = client_notification_to_jsonrpc(notification);
            assert_eq!(method, expected_method);
            assert_eq!(params.is_some(), should_have_params);
        }
    }

    #[test]
    fn test_server_notification_to_jsonrpc_all_variants() {
        let progress = Progress {
            progress_token: ProgressToken::String("test".to_string()),
            progress: 25.0,
            message: None,
        };
        let resource_updated = crate::types::protocol::ResourceUpdatedParams {
            uri: "test://uri".to_string(),
        };
        let log_message = crate::types::protocol::LogMessageParams {
            level: crate::types::protocol::LogLevel::Info,
            message: String::new(),
            logger: None,
            data: None,
        };

        let test_cases = vec![
            (
                ServerNotification::Progress(progress),
                "notifications/progress",
                true,
            ),
            (
                ServerNotification::ResourceUpdated(resource_updated),
                "notifications/resources/updated",
                true,
            ),
            (
                ServerNotification::LogMessage(log_message),
                "notifications/message",
                true,
            ),
        ];

        for (notification, expected_method, should_have_params) in test_cases {
            let (method, params) = server_notification_to_jsonrpc(notification);
            assert_eq!(method, expected_method);
            assert_eq!(params.is_some(), should_have_params);
        }
    }

    #[test]
    fn test_parse_invalid_progress_notification() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": {
                "invalid": "data"
            }
        });

        let result = parse_notification(notification_json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid progress notification"));
    }

    #[test]
    fn test_parse_invalid_cancelled_notification() {
        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {
                "invalid": "data"
            }
        });

        let result = parse_notification(notification_json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid cancelled notification"));
    }

    #[test]
    fn test_roundtrip_request_parsing() {
        // Test that we can create a request and parse it back
        let original_id = RequestId::from(42i64);
        let original_request = Request::Client(ClientRequest::Ping);

        let jsonrpc_request = create_request(original_id.clone(), original_request.clone());
        let (parsed_id, parsed_request) = parse_request(jsonrpc_request).unwrap();

        assert_eq!(parsed_id, original_id);
        match (original_request, parsed_request) {
            (Request::Client(ClientRequest::Ping), Request::Client(ClientRequest::Ping)) => (),
            _ => panic!("Roundtrip failed"),
        }
    }

    #[test]
    fn test_roundtrip_notification_parsing() {
        // Test that we can create a notification and parse it back
        let original_notification = Notification::Client(ClientNotification::Initialized);

        let jsonrpc_notif = create_notification(original_notification.clone());
        let notification_value = serde_json::to_value(&jsonrpc_notif).unwrap();
        let parsed_notification = parse_notification(notification_value).unwrap();

        match (original_notification, parsed_notification) {
            (
                Notification::Client(ClientNotification::Initialized),
                Notification::Client(ClientNotification::Initialized),
            ) => (),
            _ => panic!("Roundtrip failed"),
        }
    }
}
