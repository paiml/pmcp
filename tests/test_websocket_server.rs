//! Tests for WebSocket server transport.

#[cfg(feature = "websocket")]
use pmcp::server::transport::websocket::WebSocketServerBuilder;
#[cfg(feature = "websocket")]
use pmcp::shared::{Transport, TransportMessage};
#[cfg(feature = "websocket")]
use pmcp::types::{ClientNotification, Notification, ProgressNotification};
#[cfg(feature = "websocket")]
use std::time::Duration;
#[cfg(feature = "websocket")]
use tokio::time::timeout;

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_server_bind() {
    let mut transport = WebSocketServerBuilder::new()
        .bind_addr("127.0.0.1:0".parse().unwrap())
        .build();

    // Should successfully bind
    transport.bind().await.expect("Failed to bind");

    // Server should not be connected until client connects
    assert!(!transport.is_connected());
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_server_accept_timeout() {
    let mut transport = WebSocketServerBuilder::new()
        .bind_addr("127.0.0.1:0".parse().unwrap())
        .build();

    transport.bind().await.expect("Failed to bind");

    // Accept should timeout if no client connects
    let result = timeout(Duration::from_millis(100), transport.accept()).await;
    assert!(result.is_err(), "Accept should timeout");
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_server_client_connection() {
    use tokio_tungstenite::connect_async;

    let mut server_transport = WebSocketServerBuilder::new()
        .bind_addr("127.0.0.1:9003".parse().unwrap())
        .build();

    server_transport.bind().await.expect("Failed to bind");

    // Spawn task to accept connection
    let server_handle = tokio::spawn(async move {
        server_transport.accept().await.expect("Failed to accept");
        assert!(server_transport.is_connected());
        server_transport
    });

    // Give server time to start listening
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect as client
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9003")
        .await
        .expect("Failed to connect");

    // Wait for server to accept
    let server_transport = server_handle.await.unwrap();
    assert!(server_transport.is_connected());

    drop(ws_stream);
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_server_send_receive() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    let mut server_transport = WebSocketServerBuilder::new()
        .bind_addr("127.0.0.1:9004".parse().unwrap())
        .build();

    server_transport.bind().await.expect("Failed to bind");

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        server_transport.accept().await.expect("Failed to accept");

        // Send a notification
        let notification = TransportMessage::Notification(Notification::Client(
            ClientNotification::Progress(ProgressNotification {
                progress: 50.0,
                message: Some("Testing".to_string()),
                progress_token: pmcp::types::ProgressToken::String("test-token".to_string()),
            }),
        ));
        server_transport
            .send(notification)
            .await
            .expect("Failed to send");

        // Receive a message
        let received = server_transport.receive().await.expect("Failed to receive");

        (server_transport, received)
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect as client
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9004")
        .await
        .expect("Failed to connect");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Receive notification from server
    let msg = ws_receiver.next().await.unwrap().unwrap();
    if let Message::Text(text) = msg {
        assert!(text.contains("progress"));
        assert!(text.contains("Testing"));
    } else {
        panic!("Expected text message");
    }

    // Send a simple request to server
    let request_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "ping",
        "id": "test-ping"
    });
    ws_sender
        .send(Message::Text(request_msg.to_string().into()))
        .await
        .unwrap();

    // Wait for server to process
    let (server_transport, received) = server_handle.await.unwrap();

    match received {
        TransportMessage::Request { id, request: _ } => {
            // Success - received the ping request
            assert_eq!(id, pmcp::types::RequestId::from("test-ping"));
        },
        _ => panic!("Expected request message, got: {:?}", received),
    }

    drop(server_transport);
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_server_close() {
    let mut transport = WebSocketServerBuilder::new()
        .bind_addr("127.0.0.1:9005".parse().unwrap())
        .build();

    transport.bind().await.expect("Failed to bind");

    // Close should work even without connection
    transport.close().await.expect("Failed to close");
    assert!(!transport.is_connected());
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn test_websocket_server_builder_options() {
    let transport = WebSocketServerBuilder::new()
        .bind_addr("127.0.0.1:9006".parse().unwrap())
        .max_frame_size(1024 * 1024)
        .max_message_size(2 * 1024 * 1024)
        .accept_unmasked_frames(true)
        .build();

    assert_eq!(transport.transport_type(), "websocket-server");
}
