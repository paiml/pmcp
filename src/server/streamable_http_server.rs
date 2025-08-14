//! Streamable HTTP server implementation for MCP.
use crate::error::Result;
use crate::server::Server;
use crate::shared::http_constants::{
    APPLICATION_JSON, LAST_EVENT_ID, MCP_PROTOCOL_VERSION, MCP_SESSION_ID, TEXT_EVENT_STREAM,
};
use crate::shared::TransportMessage;
use crate::types::{ClientRequest, Request};
use axum::{
    async_trait,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::StreamExt;
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

/// Event store trait for resumability support
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store an event for later retrieval
    async fn store_event(
        &self,
        stream_id: &str,
        event_id: &str,
        message: &TransportMessage,
    ) -> Result<()>;

    /// Replay events after a given event ID
    async fn replay_events_after(
        &self,
        last_event_id: &str,
    ) -> Result<Vec<(String, TransportMessage)>>;

    /// Get stream ID for an event ID
    async fn get_stream_for_event(&self, event_id: &str) -> Result<Option<String>>;
}

/// Type alias for event list
type EventList = Vec<(String, TransportMessage)>;

/// Type alias for events map
type EventsMap = HashMap<String, EventList>;

/// In-memory event store implementation
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    /// Events by stream ID
    events: Arc<RwLock<EventsMap>>,
    /// Event ID to stream ID mapping
    event_to_stream: Arc<RwLock<HashMap<String, String>>>,
    /// Ordered list of all event IDs
    event_order: Arc<RwLock<Vec<String>>>,
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn store_event(
        &self,
        stream_id: &str,
        event_id: &str,
        message: &TransportMessage,
    ) -> Result<()> {
        let mut events = self.events.write();
        let stream_events = events.entry(stream_id.to_string()).or_default();
        stream_events.push((event_id.to_string(), message.clone()));

        self.event_to_stream
            .write()
            .insert(event_id.to_string(), stream_id.to_string());
        self.event_order.write().push(event_id.to_string());

        Ok(())
    }

    async fn replay_events_after(
        &self,
        last_event_id: &str,
    ) -> Result<Vec<(String, TransportMessage)>> {
        let event_order = self.event_order.read();
        let mut result = Vec::new();

        // Find the position of the last event
        let start_pos = event_order
            .iter()
            .position(|id| id == last_event_id)
            .map_or(0, |pos| pos + 1);

        // Collect all events after that position
        let events = self.events.read();
        let event_to_stream = self.event_to_stream.read();

        for i in start_pos..event_order.len() {
            let event_id = &event_order[i];
            if let Some(stream_id) = event_to_stream.get(event_id) {
                if let Some(stream_events) = events.get(stream_id) {
                    for (eid, msg) in stream_events {
                        if eid == event_id {
                            result.push((eid.clone(), msg.clone()));
                            break;
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    async fn get_stream_for_event(&self, event_id: &str) -> Result<Option<String>> {
        Ok(self.event_to_stream.read().get(event_id).cloned())
    }
}

/// Type alias for session callback
type SessionCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Configuration for the streamable HTTP server.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
/// use std::sync::Arc;
///
/// // Stateless configuration (for serverless/Lambda)
/// let config = StreamableHttpServerConfig {
///     session_id_generator: None,  // No sessions
///     enable_json_response: false,
///     event_store: None,
///     on_session_initialized: None,
///     on_session_closed: None,
/// };
///
/// // Stateful configuration with custom session IDs
/// let config = StreamableHttpServerConfig {
///     session_id_generator: Some(Box::new(|| {
///         format!("session-{}", uuid::Uuid::new_v4())
///     })),
///     enable_json_response: false,
///     event_store: None,
///     on_session_initialized: Some(Box::new(|session_id| {
///         println!("Session started: {}", session_id);
///     })),
///     on_session_closed: Some(Box::new(|session_id| {
///         println!("Session ended: {}", session_id);
///     })),
/// };
/// ```
pub struct StreamableHttpServerConfig {
    /// Function to generate session IDs (None for stateless mode)
    pub session_id_generator: Option<Box<dyn Fn() -> String + Send + Sync>>,
    /// Enable JSON responses instead of SSE
    pub enable_json_response: bool,
    /// Event store for resumability
    pub event_store: Option<Arc<dyn EventStore>>,
    /// Callback when session is initialized
    pub on_session_initialized: Option<SessionCallback>,
    /// Callback when session is closed
    pub on_session_closed: Option<SessionCallback>,
}

impl std::fmt::Debug for StreamableHttpServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpServerConfig")
            .field("session_id_generator", &self.session_id_generator.is_some())
            .field("enable_json_response", &self.enable_json_response)
            .field("event_store", &self.event_store.is_some())
            .field(
                "on_session_initialized",
                &self.on_session_initialized.is_some(),
            )
            .field("on_session_closed", &self.on_session_closed.is_some())
            .finish()
    }
}

impl Default for StreamableHttpServerConfig {
    fn default() -> Self {
        Self {
            session_id_generator: Some(Box::new(|| Uuid::new_v4().to_string())),
            enable_json_response: false,
            event_store: Some(Arc::new(InMemoryEventStore::default())),
            on_session_initialized: None,
            on_session_closed: None,
        }
    }
}

/// Session information
#[derive(Debug, Clone)]
struct SessionInfo {
    initialized: bool,
    protocol_version: Option<String>,
}

/// Server state shared across routes
#[derive(Clone)]
struct ServerState {
    server: Arc<tokio::sync::Mutex<Server>>,
    config: Arc<StreamableHttpServerConfig>,
    /// Active SSE streams by session ID
    sse_streams: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<TransportMessage>>>>,
    /// Session tracking (session ID -> session info)
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
}

/// A streamable HTTP server for MCP.
pub struct StreamableHttpServer {
    addr: SocketAddr,
    state: ServerState,
}

impl std::fmt::Debug for StreamableHttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamableHttpServer")
            .field("addr", &self.addr)
            .field("state", &"ServerState { ... }")
            .finish()
    }
}

/// Helper function to create JSON-RPC error response
fn create_error_response(status: StatusCode, code: i32, message: &str) -> Response {
    let error_body = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": null
    });

    (status, Json(error_body)).into_response()
}

impl StreamableHttpServer {
    /// Creates a new `StreamableHttpServer` with default config
    pub fn new(addr: SocketAddr, server: Arc<tokio::sync::Mutex<Server>>) -> Self {
        Self::with_config(addr, server, StreamableHttpServerConfig::default())
    }

    /// Creates a new `StreamableHttpServer` with custom config
    pub fn with_config(
        addr: SocketAddr,
        server: Arc<tokio::sync::Mutex<Server>>,
        config: StreamableHttpServerConfig,
    ) -> Self {
        let state = ServerState {
            server,
            config: Arc::new(config),
            sse_streams: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        };

        Self { addr, state }
    }

    /// Starts the server and returns the bound address and a task handle.
    pub async fn start(self) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let app = Router::new()
            .route("/", post(handle_post_request))
            .route("/", get(handle_get_sse))
            .route("/", delete(handle_delete_session))
            .with_state(self.state);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Ok((local_addr, server_task))
    }
}

/// Validate request headers and return appropriate error response
fn validate_headers(headers: &HeaderMap, method: &str) -> std::result::Result<(), Response> {
    match method {
        "POST" => {
            // Validate Content-Type
            if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
                let ct = content_type.to_str().unwrap_or("");
                if !ct.contains(APPLICATION_JSON) {
                    return Err(create_error_response(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        -32700,
                        "Content-Type must be application/json",
                    ));
                }
            } else {
                return Err(create_error_response(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    -32700,
                    "Content-Type header is required",
                ));
            }

            // Validate Accept
            if let Some(accept) = headers.get(header::ACCEPT) {
                let accept_str = accept.to_str().unwrap_or("");
                if !accept_str.contains(APPLICATION_JSON) && !accept_str.contains(TEXT_EVENT_STREAM)
                {
                    return Err(create_error_response(
                        StatusCode::NOT_ACCEPTABLE,
                        -32700,
                        "Accept header must include application/json or text/event-stream",
                    ));
                }
            } else {
                return Err(create_error_response(
                    StatusCode::NOT_ACCEPTABLE,
                    -32700,
                    "Accept header is required",
                ));
            }
        },
        "GET" => {
            // Validate Accept for SSE
            if let Some(accept) = headers.get(header::ACCEPT) {
                let accept_str = accept.to_str().unwrap_or("");
                if !accept_str.contains(TEXT_EVENT_STREAM) {
                    return Err(create_error_response(
                        StatusCode::NOT_ACCEPTABLE,
                        -32700,
                        "Accept header must be text/event-stream for SSE",
                    ));
                }
            } else {
                return Err(create_error_response(
                    StatusCode::NOT_ACCEPTABLE,
                    -32700,
                    "Accept header is required for SSE",
                ));
            }
        },
        _ => {},
    }

    Ok(())
}

/// Process session for initialization request
fn process_init_session(
    state: &ServerState,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<(Option<String>, bool), Response> {
    if let Some(generator) = &state.config.session_id_generator {
        // Stateful mode
        if let Some(sid) = session_id {
            // Check if session already exists and is initialized
            if let Some(session_info) = state.sessions.read().get(&sid) {
                if session_info.initialized {
                    // Session already initialized - reject re-initialization
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        -32600,
                        "Session already initialized",
                    ));
                }
            }
            // Use existing session ID
            Ok((Some(sid), false))
        } else {
            // Generate new session ID
            let new_id = generator();
            // Create new session entry
            state.sessions.write().insert(
                new_id.clone(),
                SessionInfo {
                    initialized: false,
                    protocol_version,
                },
            );
            if let Some(callback) = &state.config.on_session_initialized {
                callback(&new_id);
            }
            Ok((Some(new_id), true))
        }
    } else {
        // Stateless mode
        Ok((None, false))
    }
}

/// Validate session for non-initialization request
fn validate_non_init_session(
    state: &ServerState,
    session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if state.config.session_id_generator.is_some() {
        // Stateful mode - require and validate session ID
        match session_id {
            None => {
                // Missing session ID
                Err(create_error_response(
                    StatusCode::BAD_REQUEST,
                    -32600,
                    "Session ID required for non-initialization requests",
                ))
            },
            Some(sid) => {
                // Validate session exists
                if !state.sessions.read().contains_key(&sid) {
                    // Unknown session ID
                    Err(create_error_response(
                        StatusCode::NOT_FOUND,
                        -32600,
                        "Unknown session ID",
                    ))
                } else {
                    Ok(Some(sid))
                }
            },
        }
    } else {
        // Stateless mode
        Ok(None)
    }
}

/// Extract negotiated protocol version from initialize response
fn extract_negotiated_version(response: &TransportMessage) -> Option<String> {
    if let TransportMessage::Response(ref json_resp) = response {
        if let crate::types::jsonrpc::ResponsePayload::Result(ref value) = json_resp.payload {
            if let Ok(init_result) =
                serde_json::from_value::<crate::types::InitializeResult>(value.clone())
            {
                return Some(init_result.protocol_version.0);
            }
        }
    }
    None
}

/// Update session info after initialization
fn update_session_after_init(
    state: &ServerState,
    session_id: Option<&String>,
    negotiated_version: Option<String>,
) {
    if let Some(sid) = session_id {
        if let Some(session_info) = state.sessions.write().get_mut(sid) {
            session_info.initialized = true;
            session_info.protocol_version =
                negotiated_version.or_else(|| Some(crate::DEFAULT_PROTOCOL_VERSION.to_string()));
        }
    }
}

/// Build response with appropriate format (JSON or SSE)
fn build_response(
    state: &ServerState,
    response: TransportMessage,
    session_id: Option<&String>,
) -> Response {
    if state.config.enable_json_response {
        // JSON response mode
        (StatusCode::OK, Json(response)).into_response()
    } else {
        // SSE streaming mode
        if let Some(sid) = session_id {
            if let Some(sender) = state.sse_streams.read().get(sid) {
                // Send to existing SSE stream
                let _ = sender.send(response);
                StatusCode::ACCEPTED.into_response()
            } else {
                // Return as SSE stream
                let (tx, rx) = mpsc::unbounded_channel();
                tx.send(response).unwrap();

                let stream = UnboundedReceiverStream::new(rx);
                let sse = Sse::new(stream.map(|msg| {
                    let event_id = Uuid::new_v4().to_string();
                    Ok::<_, Infallible>(
                        Event::default()
                            .id(event_id)
                            .event("message")
                            .data(serde_json::to_string(&msg).unwrap()),
                    )
                }));

                sse.into_response()
            }
        } else {
            // No session, return JSON
            (StatusCode::OK, Json(response)).into_response()
        }
    }
}

/// Validate protocol version for non-init requests
fn validate_protocol_version(
    state: &ServerState,
    session_id: Option<&String>,
    protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    if let Some(version) = protocol_version {
        // Check if the provided version is supported
        if !crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&version.as_str()) {
            return Err(create_error_response(
                StatusCode::BAD_REQUEST,
                -32600,
                &format!("Unsupported protocol version: {}", version),
            ));
        }
    }

    // For stateful mode, also validate against session's negotiated version if exists
    if state.config.session_id_generator.is_some() {
        if let Some(sid) = session_id {
            if let Some(session_info) = state.sessions.read().get(sid.as_str()) {
                if let Some(ref negotiated_version) = session_info.protocol_version {
                    // If header provided, it should match the negotiated version
                    if let Some(provided_version) = protocol_version {
                        if provided_version != negotiated_version {
                            return Err(create_error_response(
                                StatusCode::BAD_REQUEST,
                                -32600,
                                &format!(
                                    "Protocol version mismatch: expected {}, got {}",
                                    negotiated_version, provided_version
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Handle POST requests
async fn handle_post_request(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Validate headers
    if let Err(error_response) = validate_headers(&headers, "POST") {
        return error_response;
    }

    // Parse the JSON body
    let message: TransportMessage = match serde_json::from_str(&body) {
        Ok(msg) => msg,
        Err(e) => {
            return create_error_response(
                StatusCode::BAD_REQUEST,
                -32700,
                &format!("Invalid JSON: {}", e),
            );
        },
    };

    // Extract session ID from headers
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Extract protocol version from headers
    let protocol_version = headers
        .get(MCP_PROTOCOL_VERSION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Check if this is an initialization request
    let is_init_request = matches!(
        &message,
        TransportMessage::Request { request: Request::Client(boxed), .. }
            if matches!(**boxed, ClientRequest::Initialize(_))
    );

    // Handle session ID logic based on request type
    let (response_session_id, _is_new_session) = if is_init_request {
        match process_init_session(&state, session_id.clone(), protocol_version.clone()) {
            Ok(result) => result,
            Err(error_response) => return error_response,
        }
    } else {
        match validate_non_init_session(&state, session_id.clone()) {
            Ok(sid) => (sid, false),
            Err(error_response) => return error_response,
        }
    };

    // Validate protocol version for non-init requests
    if !is_init_request {
        if let Err(error_response) =
            validate_protocol_version(&state, session_id.as_ref(), protocol_version.as_ref())
        {
            return error_response;
        }
    }

    // Process the message
    match message {
        TransportMessage::Request { id, request } => {
            let server = state.server.lock().await;
            let json_response = server.handle_request(id, request).await;
            let response = TransportMessage::Response(json_response.clone());

            // Handle initialization response
            let negotiated_version = if is_init_request {
                let version = extract_negotiated_version(&response);
                update_session_after_init(&state, response_session_id.as_ref(), version.clone());
                version
            } else {
                None
            };

            // Store event if we have an event store
            if let Some(event_store) = &state.config.event_store {
                if let Some(sid) = &response_session_id {
                    let event_id = Uuid::new_v4().to_string();
                    let _ = event_store.store_event(sid, &event_id, &response).await;
                }
            }

            // Build response with headers
            let mut response = build_response(&state, response, session_id.as_ref());

            // Always add session header in stateful mode
            if let Some(sid) = &response_session_id {
                response
                    .headers_mut()
                    .insert(MCP_SESSION_ID, sid.parse().unwrap());
            }

            // Add protocol version header
            let version_to_send = if is_init_request {
                // For init responses, use the negotiated version
                negotiated_version.unwrap_or_else(|| crate::DEFAULT_PROTOCOL_VERSION.to_string())
            } else {
                // For subsequent responses, echo the session's negotiated version
                if let Some(ref sid) = response_session_id {
                    if let Some(session_info) = state.sessions.read().get(sid) {
                        session_info
                            .protocol_version
                            .clone()
                            .unwrap_or_else(|| crate::DEFAULT_PROTOCOL_VERSION.to_string())
                    } else {
                        crate::DEFAULT_PROTOCOL_VERSION.to_string()
                    }
                } else {
                    // Stateless mode or no session - use default
                    crate::DEFAULT_PROTOCOL_VERSION.to_string()
                }
            };

            response
                .headers_mut()
                .insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());

            response
        },
        TransportMessage::Notification { .. } => {
            // Notifications get 202 Accepted
            StatusCode::ACCEPTED.into_response()
        },
        TransportMessage::Response(_) => StatusCode::ACCEPTED.into_response(),
    }
}

/// Handle GET requests for SSE streams
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    // Validate headers
    if let Err(error_response) = validate_headers(&headers, "GET") {
        return error_response;
    }

    // Extract session ID
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Validate or generate session ID
    let session_id = if let Some(sid) = session_id {
        // Validate session exists
        if state.config.session_id_generator.is_some() && !state.sessions.read().contains_key(&sid)
        {
            return create_error_response(StatusCode::NOT_FOUND, -32600, "Unknown session ID");
        }
        sid
    } else if let Some(generator) = &state.config.session_id_generator {
        // Generate new session for GET SSE
        let new_id = generator();
        state.sessions.write().insert(
            new_id.clone(),
            SessionInfo {
                initialized: true, // GET SSE implicitly initializes
                protocol_version: None,
            },
        );
        if let Some(callback) = &state.config.on_session_initialized {
            callback(&new_id);
        }
        new_id
    } else {
        // Stateless mode, no SSE
        return create_error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            -32601,
            "SSE not supported in stateless mode",
        );
    };

    // Check if stream already exists for this session
    if state.sse_streams.read().contains_key(&session_id) {
        return create_error_response(
            StatusCode::CONFLICT,
            -32600,
            "SSE stream already exists for this session",
        );
    }

    // Create SSE stream
    let (tx, rx) = mpsc::unbounded_channel();
    state
        .sse_streams
        .write()
        .insert(session_id.clone(), tx.clone());

    // Check for Last-Event-ID for resumability
    if let Some(last_event_id) = headers.get(LAST_EVENT_ID) {
        if let Ok(last_id) = last_event_id.to_str() {
            if let Some(event_store) = &state.config.event_store {
                // Replay events after the last event ID
                if let Ok(events) = event_store.replay_events_after(last_id).await {
                    for (_event_id, msg) in events {
                        let _ = tx.send(msg);
                    }
                }
            }
        }
    }

    let stream = UnboundedReceiverStream::new(rx);
    let session_id_header = session_id.clone();

    let sse = Sse::new(stream.map(move |msg| {
        let event_id = Uuid::new_v4().to_string();

        // Store event if we have an event store
        if let Some(event_store) = &state.config.event_store {
            let sid = session_id.clone();
            let msg_clone = msg.clone();
            let store = event_store.clone();
            let event_id_clone = event_id.clone();
            tokio::spawn(async move {
                let _ = store.store_event(&sid, &event_id_clone, &msg_clone).await;
            });
        }

        Ok::<_, Infallible>(
            Event::default()
                .id(event_id)
                .event("message")
                .data(serde_json::to_string(&msg).unwrap()),
        )
    }));

    let mut response = sse.into_response();

    // Add session ID header
    response
        .headers_mut()
        .insert(MCP_SESSION_ID, session_id_header.parse().unwrap());

    // Add SSE-specific headers for hardening
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    // Content-Type is already set by Axum's Sse

    response
}

/// Handle DELETE requests to terminate sessions
async fn handle_delete_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Extract session ID
    let session_id = headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(sid) = session_id {
        // Check if session exists
        let session_exists = state.sessions.read().contains_key(&sid);

        if !session_exists && state.config.session_id_generator.is_some() {
            // Unknown session in stateful mode
            return create_error_response(StatusCode::NOT_FOUND, -32600, "Unknown session ID");
        }

        // Remove SSE stream if exists
        state.sse_streams.write().remove(&sid);

        // Remove session from tracking
        state.sessions.write().remove(&sid);

        // Notify callback
        if let Some(callback) = &state.config.on_session_closed {
            callback(&sid);
        }

        (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
    } else {
        // No session to delete
        create_error_response(StatusCode::NOT_FOUND, -32600, "No session ID provided")
    }
}
