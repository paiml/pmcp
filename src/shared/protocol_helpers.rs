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
        ClientRequest::Initialize(params) => (
            "initialize".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::ListTools(params) => (
            "tools/list".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::CallTool(params) => (
            "tools/call".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::ListPrompts(params) => (
            "prompts/list".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::GetPrompt(params) => (
            "prompts/get".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::ListResources(params) => (
            "resources/list".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::ListResourceTemplates(params) => (
            "resources/templates/list".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::ReadResource(params) => (
            "resources/read".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::Subscribe(params) => (
            "resources/subscribe".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::Unsubscribe(params) => (
            "resources/unsubscribe".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::Complete(params) => (
            "completion/complete".to_string(),
            Some(serde_json::to_value(params).unwrap()),
        ),
        ClientRequest::SetLoggingLevel { level } => (
            "logging/setLevel".to_string(),
            Some(serde_json::json!({"level": level})),
        ),
        ClientRequest::Ping => ("ping".to_string(), None),
    }
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
