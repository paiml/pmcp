//! Example OAuth 2.0 server implementation using PMCP.

use async_trait::async_trait;
use pmcp::error::{Error, ErrorCode, Result};
use pmcp::server::auth::{
    AuthMiddleware, BearerTokenMiddleware, GrantType, InMemoryOAuthProvider, OAuthClient,
    OAuthProvider, ResponseType, ScopeMiddleware,
};
use pmcp::server::{Server, ServerCapabilities, ToolHandler};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

/// Public tool that doesn't require authentication
struct GetTimeTool;

#[async_trait]
impl ToolHandler for GetTimeTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        Ok(json!({
            "time": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// Tool that requires read scope
struct ReadDataTool {
    auth_middleware: Arc<dyn AuthMiddleware>,
}

#[async_trait]
impl ToolHandler for ReadDataTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Authenticate request
        let auth_ctx = self
            .auth_middleware
            .authenticate(extra.auth_info.as_ref())
            .await?;
        info!("Authenticated user {} for read_data", auth_ctx.user_id);

        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        Ok(json!({
            "key": key,
            "value": format!("Data for key '{}' (user: {})", key, auth_ctx.user_id),
            "scopes": auth_ctx.scopes
        }))
    }
}

/// Tool that requires write scope
struct WriteDataTool {
    auth_middleware: Arc<dyn AuthMiddleware>,
}

#[async_trait]
impl ToolHandler for WriteDataTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Authenticate request
        let auth_ctx = self
            .auth_middleware
            .authenticate(extra.auth_info.as_ref())
            .await?;
        info!("Authenticated user {} for write_data", auth_ctx.user_id);

        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");

        Ok(json!({
            "success": true,
            "key": key,
            "value": value,
            "written_by": auth_ctx.user_id,
            "scopes": auth_ctx.scopes
        }))
    }
}

/// Tool that requires admin scope
struct AdminOperationTool {
    auth_middleware: Arc<dyn AuthMiddleware>,
}

#[async_trait]
impl ToolHandler for AdminOperationTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Authenticate request
        let auth_ctx = self
            .auth_middleware
            .authenticate(extra.auth_info.as_ref())
            .await?;
        info!(
            "Authenticated admin {} for admin_operation",
            auth_ctx.user_id
        );

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("status");

        Ok(json!({
            "success": true,
            "operation": operation,
            "admin": auth_ctx.user_id,
            "result": format!("Admin operation '{}' completed", operation)
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create OAuth provider
    let oauth_provider = Arc::new(InMemoryOAuthProvider::new("http://localhost:8080"));

    // Register a test client
    let client = OAuthClient {
        client_id: "test-client".to_string(),
        client_secret: Some("test-secret".to_string()),
        client_name: "Test Client".to_string(),
        redirect_uris: vec!["http://localhost:3000/callback".to_string()],
        grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
        response_types: vec![ResponseType::Code],
        scopes: vec!["read".to_string(), "write".to_string(), "admin".to_string()],
        metadata: HashMap::new(),
    };

    let registered_client = oauth_provider.register_client(client).await?;
    info!("Registered OAuth client: {}", registered_client.client_id);

    // Create middleware instances
    let read_middleware = Arc::new(ScopeMiddleware::any(
        Box::new(BearerTokenMiddleware::new(oauth_provider.clone())),
        vec!["read".to_string()],
    ));

    let write_middleware = Arc::new(ScopeMiddleware::any(
        Box::new(BearerTokenMiddleware::new(oauth_provider.clone())),
        vec!["write".to_string()],
    ));

    let admin_middleware = Arc::new(ScopeMiddleware::all(
        Box::new(BearerTokenMiddleware::new(oauth_provider.clone())),
        vec!["admin".to_string()],
    ));

    // Create server with OAuth protection
    let server = Server::builder()
        .name("oauth-example-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(Default::default()),
            ..Default::default()
        })
        // Add public tool - no auth required
        .tool("get_time", GetTimeTool)
        // Add read-only tool - requires 'read' scope
        .tool("read_data", ReadDataTool { auth_middleware: read_middleware })
        // Add write tool - requires 'write' scope
        .tool("write_data", WriteDataTool { auth_middleware: write_middleware })
        // Add admin tool - requires 'admin' scope
        .tool("admin_operation", AdminOperationTool { auth_middleware: admin_middleware })
        .build()?;

    // Print OAuth endpoints
    info!("\nOAuth 2.0 Endpoints:");
    info!("  Authorization: http://localhost:8080/oauth2/authorize");
    info!("  Token: http://localhost:8080/oauth2/token");
    info!("  Registration: http://localhost:8080/oauth2/register");
    info!("  Revocation: http://localhost:8080/oauth2/revoke");

    info!("\nRegistered Client:");
    info!("  Client ID: {}", registered_client.client_id);
    info!(
        "  Client Secret: {}",
        registered_client.client_secret.as_deref().unwrap_or("N/A")
    );
    info!("  Scopes: {:?}", registered_client.scopes);

    info!("\nExample OAuth Flow:");
    info!("1. Authorize: http://localhost:8080/oauth2/authorize?response_type=code&client_id={}&redirect_uri=http://localhost:3000/callback&scope=read%20write", registered_client.client_id);
    info!("2. Exchange code for token at /oauth2/token");
    info!("3. Use token in Authorization header: Bearer <token>");

    // For demonstration, create a test token
    let test_token = oauth_provider
        .create_access_token(
            &registered_client.client_id,
            "test-user",
            vec!["read".to_string(), "write".to_string()],
        )
        .await?;

    info!("\nTest Token (for development):");
    info!("  Access Token: {}", test_token.access_token);
    info!(
        "  Expires In: {} seconds",
        test_token.expires_in.unwrap_or(0)
    );
    info!("  Scopes: {}", test_token.scope.as_deref().unwrap_or(""));

    info!("\nStarting OAuth-protected MCP server on stdio...");
    info!("Try these commands with the test token:");
    info!("  - get_time (no auth required)");
    info!("  - read_data (requires 'read' scope)");
    info!("  - write_data (requires 'write' scope)");
    info!("  - admin_operation (requires 'admin' scope - will fail with test token)");

    // Run server
    if let Err(e) = server.run_stdio().await {
        error!("Server error: {}", e);
    }

    Ok(())
}
