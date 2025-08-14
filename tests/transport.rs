use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::Server;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{ClientCapabilities, ClientRequest, Implementation, InitializeParams, Request};
// Use boxed error for tests to satisfy clippy
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use url::Url;

#[tokio::test]
async fn test_streamable_http_transport_send_receive() -> Result<()> {
    // Setup the server
    let server = Arc::new(Mutex::new(
        Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
    ));
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    let http_server = StreamableHttpServer::new(addr, server);
    let (server_addr, server_task) = http_server.start().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // Setup the client transport
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", server_addr))
            .map_err(|e| Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: false,
        on_resumption_token: None,
    };
    let mut client_transport = StreamableHttpTransport::new(client_config);

    // Create an Initialize request first (to get session ID)
    let init_message = TransportMessage::Request {
        id: 1i64.into(),
        request: Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
            protocol_version: pmcp::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        }))),
    };

    // Send the initialization request
    timeout(Duration::from_secs(5), client_transport.send(init_message))
        .await
        .expect("send should not time out")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let init_response = timeout(Duration::from_secs(5), client_transport.receive())
        .await
        .expect("receive should not time out")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // Verify the initialization response
    assert!(matches!(init_response, TransportMessage::Response(_)));

    // Now test a regular request (session should be established)
    let ping_message = TransportMessage::Request {
        id: 2i64.into(),
        request: Request::Client(Box::new(ClientRequest::Ping)),
    };

    timeout(Duration::from_secs(5), client_transport.send(ping_message))
        .await
        .expect("send should not time out")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let ping_response = timeout(Duration::from_secs(5), client_transport.receive())
        .await
        .expect("receive should not time out")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // Verify the ping response
    assert!(matches!(ping_response, TransportMessage::Response(_)));

    // Shutdown the server
    server_task.abort();

    Ok(())
}
