use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::Result;
use std::time::Duration;
use tokio::time::timeout;
use url::Url;

#[tokio::test]
async fn test_streamable_http_transport_send_receive() -> Result<()> {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
        .create_async()
        .await;

    let config = StreamableHttpTransportConfig {
        url: Url::parse(&server.url()).map_err(|e| pmcp::Error::Internal(e.to_string()))?,
        request_init: None,
        auth_provider: None,
        session_id: None,
        reconnect_config: None,
    };
    let mut transport = StreamableHttpTransport::new(config);

    let message = TransportMessage::Request {
        id: 1i64.into(),
        request: pmcp::types::Request::Client(Box::new(pmcp::types::ClientRequest::Ping)),
    };

    timeout(Duration::from_secs(5), transport.send(message))
        .await
        .expect("send should not time out")?;

    let received = timeout(Duration::from_secs(5), transport.receive())
        .await
        .expect("receive should not time out")?;

    mock.assert_async().await;
    assert!(matches!(received, TransportMessage::Response(_)));

    Ok(())
}
