//! Streamable HTTP server implementation for MCP.
use crate::error::Result;
use crate::server::Server;
use crate::shared::TransportMessage;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A streamable HTTP server for MCP.
#[derive(Debug)]
pub struct StreamableHttpServer {
    addr: SocketAddr,
    server: Arc<Mutex<Server>>,
}

impl StreamableHttpServer {
    /// Creates a new StreamableHttpServer.
    pub fn new(addr: SocketAddr, server: Arc<Mutex<Server>>) -> Self {
        Self { addr, server }
    }

    /// Starts the server and returns the bound address and a task handle.
    pub async fn start(self) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let app = Router::new()
            .route("/", post(handle_post_request))
            .with_state(self.server);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        let server_task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Ok((local_addr, server_task))
    }
}

async fn handle_post_request(
    State(server): State<Arc<Mutex<Server>>>,
    Json(message): Json<TransportMessage>,
) -> Response {
    let (id, request) = match message {
        TransportMessage::Request { id, request } => (id, request),
        _ => {
            // Not a request, so we don't need to respond.
            return (StatusCode::ACCEPTED, "Notification or batch received").into_response();
        }
    };

    let server = server.lock().await;
    let response = server.handle_request(id, request).await;

    (StatusCode::OK, Json(response)).into_response()
}
