#[cfg(feature = "streamable-http")]
mod streamable_http_server_tests {
    use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
    use pmcp::server::Server;
    use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
    use pmcp::shared::{Transport, TransportMessage};
    use pmcp::types::{
        ClientCapabilities, ClientRequest, Implementation, InitializeParams, Request,
    };
    // Use boxed error for tests to satisfy clippy
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use url::Url;

    #[tokio::test]
    async fn test_initialization_generates_session_id() -> Result<()> {
        // Setup server with stateful mode (default)
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send initialization without session ID
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

        client.send(init_message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client.receive().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Verify session ID was assigned
        assert!(
            client.session_id().is_some(),
            "Session ID should be generated on initialization"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_non_init_requires_session_in_stateful_mode() -> Result<()> {
        // Setup server with stateful mode
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client without session
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None, // No session ID
            enable_json_response: false,
            on_resumption_token: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send non-init request without session ID - should fail
        let ping_message = TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        let result = client.send(ping_message).await;
        assert!(
            result.is_err(),
            "Non-init request without session should fail"
        );

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_stateless_mode_no_session_required() -> Result<()> {
        // Setup server in stateless mode
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);

        let config = StreamableHttpServerConfig {
            session_id_generator: None, // Stateless mode
            enable_json_response: false,
            event_store: None,
            on_session_initialized: None,
            on_session_closed: None,
        };

        let http_server = StreamableHttpServer::with_config(addr, server, config);
        let (server_addr, server_task) = http_server.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send initialization
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

        client.send(init_message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client.receive().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // No session ID should be generated in stateless mode
        assert!(
            client.session_id().is_none(),
            "No session ID in stateless mode"
        );

        // Non-init requests should still work without session
        let ping_message = TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        client.send(ping_message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client.receive().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        server_task.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_protocol_version_header_included() -> Result<()> {
        // Setup server
        let server = Arc::new(Mutex::new(
            Server::builder()
                .name("test-server")
                .version("1.0.0")
                .build().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
        ));
        let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let http_server = StreamableHttpServer::new(addr, server);
        let (server_addr, server_task) = http_server.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Setup client
        let client_config = StreamableHttpTransportConfig {
            url: Url::parse(&format!("http://{}", server_addr))
                .map_err(|e| Box::new(pmcp::Error::Internal(e.to_string())) as Box<dyn std::error::Error + Send + Sync>)?,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
        };
        let mut client = StreamableHttpTransport::new(client_config);

        // Send initialization
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

        client.send(init_message).await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        let _response = client.receive().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Protocol version should be set after initialization
        assert!(
            client.protocol_version().is_some(),
            "Protocol version should be set"
        );

        server_task.abort();
        Ok(())
    }
}
