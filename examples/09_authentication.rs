//! Example: Authentication in MCP
//!
//! This example demonstrates:
//! - OAuth 2.0 authentication flow
//! - Bearer token authentication
//! - Custom authentication handlers
//! - Token refresh and expiration

use pmcp::{
    Client, Server, ClientCapabilities, ServerCapabilities,
    StdioTransport, AuthHandler, AuthenticationScheme,
    types::{AuthenticationOptions, AuthenticationResult}
};
use async_trait::async_trait;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

// Mock OAuth provider
struct MockOAuthProvider {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    tokens: HashMap<String, (String, DateTime<Utc>)>, // token -> (user_id, expiry)
}

impl MockOAuthProvider {
    fn new() -> Self {
        Self {
            client_id: "demo-client-id".to_string(),
            client_secret: "demo-client-secret".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            tokens: HashMap::new(),
        }
    }
    
    fn generate_token(&mut self, user_id: &str) -> String {
        let token = format!("mock-token-{}", uuid::Uuid::new_v4());
        let expiry = Utc::now() + Duration::hours(1);
        self.tokens.insert(token.clone(), (user_id.to_string(), expiry));
        token
    }
    
    fn validate_token(&self, token: &str) -> Option<String> {
        self.tokens.get(token)
            .filter(|(_, expiry)| expiry > &Utc::now())
            .map(|(user_id, _)| user_id.clone())
    }
}

// OAuth authentication handler
struct OAuthHandler {
    provider: MockOAuthProvider,
}

#[async_trait]
impl AuthHandler for OAuthHandler {
    async fn authenticate(&self, scheme: &AuthenticationScheme) -> pmcp::Result<AuthenticationResult> {
        match scheme {
            AuthenticationScheme::OAuth2 { 
                auth_url, 
                token_url, 
                client_id, 
                scopes,
                .. 
            } => {
                println!("🔐 OAuth2 Authentication Request:");
                println!("   Auth URL: {}", auth_url);
                println!("   Token URL: {}", token_url);
                println!("   Client ID: {}", client_id);
                println!("   Scopes: {}", scopes.join(", "));
                
                // In a real implementation, you would:
                // 1. Open browser to auth_url with parameters
                // 2. Handle callback with authorization code
                // 3. Exchange code for token at token_url
                
                // For demo, we'll simulate successful auth
                let token = self.provider.generate_token("demo-user");
                
                Ok(AuthenticationResult {
                    access_token: token,
                    token_type: "Bearer".to_string(),
                    expires_in: Some(3600),
                    refresh_token: Some(format!("refresh-{}", uuid::Uuid::new_v4())),
                    scope: Some(scopes.join(" ")),
                })
            }
            _ => Err(pmcp::Error::authentication("Unsupported authentication scheme")),
        }
    }
}

// Bearer token authentication handler
struct BearerTokenHandler {
    valid_tokens: HashMap<String, String>, // token -> user_id
}

impl BearerTokenHandler {
    fn new() -> Self {
        let mut tokens = HashMap::new();
        // Pre-configured tokens for demo
        tokens.insert("demo-api-key-123".to_string(), "api-user-1".to_string());
        tokens.insert("demo-api-key-456".to_string(), "api-user-2".to_string());
        
        Self {
            valid_tokens: tokens,
        }
    }
}

#[async_trait]
impl AuthHandler for BearerTokenHandler {
    async fn authenticate(&self, scheme: &AuthenticationScheme) -> pmcp::Result<AuthenticationResult> {
        match scheme {
            AuthenticationScheme::Bearer { token } => {
                println!("🔑 Bearer Token Authentication:");
                println!("   Token: {}...", &token[..token.len().min(20)]);
                
                if self.valid_tokens.contains_key(token) {
                    Ok(AuthenticationResult {
                        access_token: token.clone(),
                        token_type: "Bearer".to_string(),
                        expires_in: None, // No expiry for API keys
                        refresh_token: None,
                        scope: Some("full_access".to_string()),
                    })
                } else {
                    Err(pmcp::Error::authentication("Invalid bearer token"))
                }
            }
            _ => Err(pmcp::Error::authentication("Expected bearer token authentication")),
        }
    }
}

// Server with authentication support
async fn run_auth_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  Starting authenticated server...\n");
    
    let server = Server::builder()
        .name("auth-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            authentication: Some(vec![
                AuthenticationOptions::OAuth2 {
                    auth_url: "https://auth.example.com/oauth/authorize".to_string(),
                    token_url: "https://auth.example.com/oauth/token".to_string(),
                    client_id: "demo-client-id".to_string(),
                    scopes: vec!["read".to_string(), "write".to_string()],
                },
                AuthenticationOptions::Bearer,
            ]),
            ..Default::default()
        })
        .auth_handler(Box::new(OAuthHandler {
            provider: MockOAuthProvider::new(),
        }))
        .build()?;
    
    println!("Server supports authentication schemes:");
    println!("  - OAuth 2.0");
    println!("  - Bearer Token");
    println!("\nListening on stdio...");
    
    server.run_stdio().await?;
    
    Ok(())
}

// Client with authentication
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();
    
    println!("=== MCP Authentication Example ===\n");
    
    // Create client
    let transport = StdioTransport::new();
    let mut client = Client::new(transport);
    
    // Initialize and check server authentication requirements
    let capabilities = ClientCapabilities::default();
    
    println!("Connecting to server...");
    let server_info = client.initialize(capabilities).await?;
    
    if let Some(auth_options) = &server_info.authentication {
        println!("\n🔒 Server requires authentication!");
        println!("Available authentication methods:");
        
        for (i, option) in auth_options.iter().enumerate() {
            match option {
                AuthenticationOptions::OAuth2 { client_id, scopes, .. } => {
                    println!("  {}. OAuth 2.0", i + 1);
                    println!("     Client ID: {}", client_id);
                    println!("     Scopes: {}", scopes.join(", "));
                }
                AuthenticationOptions::Bearer => {
                    println!("  {}. Bearer Token", i + 1);
                }
            }
        }
        
        // Example 1: OAuth authentication
        println!("\n\n🔐 Attempting OAuth authentication...");
        match client.authenticate_oauth(
            "https://auth.example.com/oauth/authorize",
            "https://auth.example.com/oauth/token",
            "demo-client-id",
            vec!["read", "write"],
        ).await {
            Ok(result) => {
                println!("✅ OAuth authentication successful!");
                println!("   Access token: {}...", &result.access_token[..20.min(result.access_token.len())]);
                if let Some(expires) = result.expires_in {
                    println!("   Expires in: {} seconds", expires);
                }
                if let Some(refresh) = &result.refresh_token {
                    println!("   Refresh token: {}...", &refresh[..20.min(refresh.len())]);
                }
            }
            Err(e) => {
                println!("❌ OAuth authentication failed: {}", e);
            }
        }
        
        // Example 2: Bearer token authentication
        println!("\n\n🔑 Attempting bearer token authentication...");
        match client.authenticate_bearer("demo-api-key-123").await {
            Ok(result) => {
                println!("✅ Bearer authentication successful!");
                println!("   Token accepted");
                if let Some(scope) = &result.scope {
                    println!("   Scope: {}", scope);
                }
            }
            Err(e) => {
                println!("❌ Bearer authentication failed: {}", e);
            }
        }
        
        // Example 3: Invalid token
        println!("\n\n⚠️  Testing invalid authentication...");
        match client.authenticate_bearer("invalid-token").await {
            Ok(_) => {
                println!("Unexpected success!");
            }
            Err(e) => {
                println!("✅ Invalid token correctly rejected: {}", e);
            }
        }
        
        // Example 4: Token refresh (if OAuth was successful)
        println!("\n\n🔄 Testing token refresh...");
        if let Some(refresh_token) = client.get_refresh_token() {
            match client.refresh_token(&refresh_token).await {
                Ok(new_result) => {
                    println!("✅ Token refreshed successfully!");
                    println!("   New access token: {}...", &new_result.access_token[..20.min(new_result.access_token.len())]);
                }
                Err(e) => {
                    println!("❌ Token refresh failed: {}", e);
                }
            }
        } else {
            println!("   No refresh token available");
        }
    } else {
        println!("✅ Connected! Server does not require authentication.");
    }
    
    Ok(())
}