use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::server::Server;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{ClientRequest, Request};
use pmcp::Result;
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
            .build()?,
    ));
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    let http_server = StreamableHttpServer::new(addr, server);
    let (server_addr, server_task) = http_server.start().await?;

    // Setup the client transport
    let client_config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{}", server_addr))
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?,
        extra_headers: vec![],
        auth_provider: None,
        reconnect_config: None,
        on_resumption_token: None,
    };
    let mut client_transport = StreamableHttpTransport::new(client_config);

    // Create a Ping request
    let message = TransportMessage::Request {
        id: 1i64.into(),
        request: Request::Client(Box::new(ClientRequest::Ping)),
    };

    // Send the request and wait for the response
    timeout(Duration::from_secs(5), client_transport.send(message))
        .await
        .expect("send should not time out")?;

    let received = timeout(Duration::from_secs(5), client_transport.receive())
        .await
        .expect("receive should not time out")?;

    // Verify the response
    assert!(matches!(received, TransportMessage::Response(_)));

    // Shutdown the server
    server_task.abort();

    Ok(())
}
