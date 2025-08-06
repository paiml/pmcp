//! Example demonstrating OIDC discovery and OAuth 2.0 token exchange.
//!
//! This example shows how to:
//! 1. Discover OIDC configuration from a provider
//! 2. Exchange authorization codes for tokens
//! 3. Refresh access tokens
//! 4. Handle CORS and network errors with retries

use pmcp::client::auth::{OidcDiscoveryClient, TokenResponse};
use pmcp::server::auth::oauth2::{
    InMemoryOAuthProvider, OAuthClient, OAuthProvider, OidcDiscoveryMetadata,
};
use pmcp::Result;
use std::time::Duration;
use tokio::time::sleep;

/// Mock OIDC server for testing.
struct MockOidcServer {
    metadata: OidcDiscoveryMetadata,
}

impl MockOidcServer {
    fn new() -> Self {
        Self {
            metadata: OidcDiscoveryMetadata {
                issuer: "https://auth.example.com".to_string(),
                authorization_endpoint: "https://auth.example.com/authorize".to_string(),
                token_endpoint: "https://auth.example.com/token".to_string(),
                jwks_uri: Some("https://auth.example.com/jwks".to_string()),
                userinfo_endpoint: Some("https://auth.example.com/userinfo".to_string()),
                registration_endpoint: Some("https://auth.example.com/register".to_string()),
                revocation_endpoint: Some("https://auth.example.com/revoke".to_string()),
                introspection_endpoint: Some("https://auth.example.com/introspect".to_string()),
                response_types_supported: vec![pmcp::server::auth::oauth2::ResponseType::Code],
                grant_types_supported: vec![
                    pmcp::server::auth::oauth2::GrantType::AuthorizationCode,
                    pmcp::server::auth::oauth2::GrantType::RefreshToken,
                ],
                scopes_supported: vec![
                    "openid".to_string(),
                    "profile".to_string(),
                    "email".to_string(),
                ],
                token_endpoint_auth_methods_supported: vec![
                    "client_secret_basic".to_string(),
                    "client_secret_post".to_string(),
                ],
                code_challenge_methods_supported: vec!["plain".to_string(), "S256".to_string()],
            },
        }
    }
}

/// Simulate OIDC discovery with retry logic.
async fn discover_with_retries(issuer_url: &str) -> Result<OidcDiscoveryMetadata> {
    println!("🔍 Discovering OIDC configuration for: {}", issuer_url);

    // Create discovery client with custom retry settings
    let _client = OidcDiscoveryClient::with_settings(
        5,                          // max retries
        Duration::from_millis(500), // retry delay
    );

    // Simulate network issues for demonstration
    let mut attempt = 0;
    loop {
        attempt += 1;
        println!("  Attempt {}/5...", attempt);

        // Simulate occasional CORS/network errors
        if attempt < 3 {
            println!("  ❌ Simulated CORS error");
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        // Return mock metadata on success
        println!("  ✅ Discovery successful!");
        return Ok(MockOidcServer::new().metadata);
    }
}

/// Simulate token exchange.
async fn exchange_authorization_code(
    token_endpoint: &str,
    auth_code: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<TokenResponse> {
    println!("\n🔄 Exchanging authorization code for tokens");
    println!("  Token endpoint: {}", token_endpoint);
    println!("  Auth code: {}...", &auth_code[..8.min(auth_code.len())]);
    println!("  Client ID: {}", client_id);
    println!("  Using client secret: {}", client_secret.is_some());

    // Simulate token response
    sleep(Duration::from_millis(100)).await;

    Ok(TokenResponse {
        access_token: "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("refresh_token_abc123".to_string()),
        scope: Some("openid profile email".to_string()),
    })
}

/// Simulate token refresh.
async fn refresh_access_token(
    token_endpoint: &str,
    refresh_token: &str,
    client_id: &str,
) -> Result<TokenResponse> {
    println!("\n🔄 Refreshing access token");
    println!("  Token endpoint: {}", token_endpoint);
    println!(
        "  Refresh token: {}...",
        &refresh_token[..10.min(refresh_token.len())]
    );
    println!("  Client ID: {}", client_id);

    // Simulate token response
    sleep(Duration::from_millis(100)).await;

    Ok(TokenResponse {
        access_token: "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.new...".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("refresh_token_xyz789".to_string()),
        scope: Some("openid profile email".to_string()),
    })
}

/// Demonstrate OAuth provider setup.
async fn setup_oauth_provider() -> Result<()> {
    println!("\n📦 Setting up OAuth provider");

    let provider = InMemoryOAuthProvider::new("https://auth.example.com");

    // Register a client
    let client = OAuthClient {
        client_id: "example-client".to_string(),
        client_secret: Some("super-secret".to_string()),
        client_name: "Example Client".to_string(),
        redirect_uris: vec!["https://app.example.com/callback".to_string()],
        grant_types: vec![
            pmcp::server::auth::oauth2::GrantType::AuthorizationCode,
            pmcp::server::auth::oauth2::GrantType::RefreshToken,
        ],
        response_types: vec![pmcp::server::auth::oauth2::ResponseType::Code],
        scopes: vec!["openid".to_string(), "profile".to_string()],
        metadata: std::collections::HashMap::new(),
    };

    let registered = provider.register_client(client).await?;
    println!("  ✅ Registered client: {}", registered.client_id);

    // Get provider metadata
    let metadata = provider.metadata().await?;
    println!("  📋 Provider metadata:");
    println!("     - Issuer: {}", metadata.issuer);
    println!("     - Auth endpoint: {}", metadata.authorization_endpoint);
    println!("     - Token endpoint: {}", metadata.token_endpoint);

    // Try OIDC discovery (will fail with default implementation)
    match provider.discover("https://auth.example.com").await {
        Ok(discovered) => {
            println!("  ✅ Discovery succeeded: {}", discovered.issuer);
        },
        Err(e) => {
            println!("  ℹ️  Discovery not implemented (expected): {}", e);
        },
    }

    Ok(())
}

/// Demonstrate transport isolation.
async fn demonstrate_transport_isolation() -> Result<()> {
    use pmcp::shared::protocol::{Protocol, ProtocolOptions, TransportId};
    use pmcp::types::{JSONRPCResponse, RequestId};

    println!("\n🔒 Demonstrating transport isolation");

    // Create two separate transports
    let transport1 = TransportId::from_string("websocket-1".to_string());
    let transport2 = TransportId::from_string("http-sse-1".to_string());

    let mut protocol1 = Protocol::with_transport_id(ProtocolOptions::default(), transport1.clone());
    let mut protocol2 = Protocol::with_transport_id(ProtocolOptions::default(), transport2.clone());

    println!("  Created transport 1: {:?}", transport1);
    println!("  Created transport 2: {:?}", transport2);

    // Register the same request ID on both transports
    let request_id = RequestId::from("test-request");
    let mut rx1 = protocol1.register_request(request_id.clone());
    let mut rx2 = protocol2.register_request(request_id.clone());

    println!("  Registered request '{}' on both transports", request_id);

    // Complete request for transport 1
    let response1 = JSONRPCResponse::success(
        request_id.clone(),
        serde_json::json!({"source": "transport1"}),
    );
    protocol1.complete_request(&request_id, response1).unwrap();

    // Complete request for transport 2
    let response2 = JSONRPCResponse::success(
        request_id.clone(),
        serde_json::json!({"source": "transport2"}),
    );
    protocol2.complete_request(&request_id, response2).unwrap();

    // Wait for async completion
    sleep(Duration::from_millis(50)).await;

    // Verify isolation
    match rx1.try_recv() {
        Ok(resp) => {
            println!("  ✅ Transport 1 received: {:?}", resp.result());
        },
        Err(_) => {
            println!("  ❌ Transport 1 didn't receive response");
        },
    }

    match rx2.try_recv() {
        Ok(resp) => {
            println!("  ✅ Transport 2 received: {:?}", resp.result());
        },
        Err(_) => {
            println!("  ❌ Transport 2 didn't receive response");
        },
    }

    Ok(())
}

#[tokio::main]
#[allow(clippy::result_large_err)]
async fn main() -> Result<()> {
    println!("🚀 OIDC Discovery and OAuth 2.0 Example\n");
    println!("{}", "=".repeat(50));

    // 1. Discover OIDC configuration
    let metadata = discover_with_retries("https://auth.example.com").await?;

    println!("\n📋 Discovered Configuration:");
    println!("  Issuer: {}", metadata.issuer);
    println!("  Authorization: {}", metadata.authorization_endpoint);
    println!("  Token: {}", metadata.token_endpoint);
    if let Some(jwks) = &metadata.jwks_uri {
        println!("  JWKS: {}", jwks);
    }
    if let Some(userinfo) = &metadata.userinfo_endpoint {
        println!("  UserInfo: {}", userinfo);
    }
    println!("  Supported scopes: {:?}", metadata.scopes_supported);

    // 2. Exchange authorization code for tokens
    let tokens = exchange_authorization_code(
        &metadata.token_endpoint,
        "auth_code_from_callback",
        "example-client",
        Some("client-secret"),
    )
    .await?;

    println!("\n🎫 Received Tokens:");
    println!("  Access token: {}...", &tokens.access_token[..20]);
    println!("  Token type: {}", tokens.token_type);
    println!("  Expires in: {:?} seconds", tokens.expires_in);
    if let Some(refresh) = &tokens.refresh_token {
        println!("  Refresh token: {}...", &refresh[..10]);
    }
    if let Some(scope) = &tokens.scope {
        println!("  Scope: {}", scope);
    }

    // 3. Refresh the access token
    if let Some(refresh_token) = tokens.refresh_token {
        let new_tokens =
            refresh_access_token(&metadata.token_endpoint, &refresh_token, "example-client")
                .await?;

        println!("\n🔄 Refreshed Tokens:");
        println!("  New access token: {}...", &new_tokens.access_token[..20]);
        println!("  Expires in: {:?} seconds", new_tokens.expires_in);
    }

    // 4. Setup OAuth provider
    setup_oauth_provider().await?;

    // 5. Demonstrate transport isolation
    demonstrate_transport_isolation().await?;

    println!("\n✅ Example completed successfully!");

    Ok(())
}
