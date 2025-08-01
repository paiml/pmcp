//! OAuth 2.0 middleware for MCP servers.

use crate::error::{Error, ErrorCode, Result};
use crate::server::auth::oauth2::OAuthProvider;
use crate::types::auth::{AuthInfo, AuthScheme};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Authentication context passed to handlers.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authenticated client ID.
    pub client_id: String,

    /// Authenticated user ID.
    pub user_id: String,

    /// Granted scopes.
    pub scopes: Vec<String>,

    /// Additional context data.
    pub metadata: serde_json::Map<String, Value>,
}

impl AuthContext {
    /// Check if a scope is granted.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Check if all required scopes are granted.
    pub fn has_all_scopes(&self, scopes: &[&str]) -> bool {
        scopes.iter().all(|scope| self.has_scope(scope))
    }

    /// Check if any of the required scopes are granted.
    pub fn has_any_scope(&self, scopes: &[&str]) -> bool {
        scopes.iter().any(|scope| self.has_scope(scope))
    }
}

/// Authentication middleware trait.
#[async_trait]
pub trait AuthMiddleware: Send + Sync {
    /// Authenticate a request.
    async fn authenticate(&self, auth_info: Option<&AuthInfo>) -> Result<AuthContext>;

    /// Check if authentication is required.
    fn is_required(&self) -> bool {
        true
    }
}

/// Bearer token authentication middleware.
pub struct BearerTokenMiddleware {
    /// OAuth provider.
    provider: Arc<dyn OAuthProvider>,

    /// Whether authentication is required.
    required: bool,
}

impl std::fmt::Debug for BearerTokenMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BearerTokenMiddleware")
            .field("provider", &"<dyn OAuthProvider>")
            .field("required", &self.required)
            .finish()
    }
}

impl BearerTokenMiddleware {
    /// Create a new bearer token middleware.
    pub fn new(provider: Arc<dyn OAuthProvider>) -> Self {
        Self {
            provider,
            required: true,
        }
    }

    /// Set whether authentication is required.
    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

#[async_trait]
impl AuthMiddleware for BearerTokenMiddleware {
    async fn authenticate(&self, auth_info: Option<&AuthInfo>) -> Result<AuthContext> {
        // Check if auth info is provided
        let auth_info = match auth_info {
            Some(info) => info,
            None => {
                if self.required {
                    return Err(Error::protocol(
                        ErrorCode::AUTHENTICATION_REQUIRED,
                        "Authentication required",
                    ));
                } else {
                    // Return anonymous context
                    return Ok(AuthContext {
                        client_id: "anonymous".to_string(),
                        user_id: "anonymous".to_string(),
                        scopes: vec![],
                        metadata: serde_json::Map::new(),
                    });
                }
            },
        };

        // Check auth scheme
        if auth_info.scheme != AuthScheme::Bearer {
            return Err(Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                "Invalid authentication scheme",
            ));
        }

        // Get token
        let token = auth_info
            .token
            .as_ref()
            .ok_or_else(|| Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, "Missing token"))?;

        // Validate token
        let token_info =
            self.provider.validate_token(token).await.map_err(|_| {
                Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, "Invalid token")
            })?;

        // Create auth context
        Ok(AuthContext {
            client_id: token_info.client_id,
            user_id: token_info.user_id,
            scopes: token_info.scopes,
            metadata: serde_json::Map::new(),
        })
    }

    fn is_required(&self) -> bool {
        self.required
    }
}

/// Client credentials authentication middleware.
pub struct ClientCredentialsMiddleware {
    /// OAuth provider.
    provider: Arc<dyn OAuthProvider>,
}

impl std::fmt::Debug for ClientCredentialsMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientCredentialsMiddleware")
            .field("provider", &"<dyn OAuthProvider>")
            .finish()
    }
}

impl ClientCredentialsMiddleware {
    /// Create a new client credentials middleware.
    pub fn new(provider: Arc<dyn OAuthProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl AuthMiddleware for ClientCredentialsMiddleware {
    async fn authenticate(&self, auth_info: Option<&AuthInfo>) -> Result<AuthContext> {
        let auth_info = auth_info.ok_or_else(|| {
            Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                "Authentication required",
            )
        })?;

        // Extract client credentials from params
        let client_id = auth_info
            .params
            .get("client_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, "Missing client_id")
            })?;

        let client_secret = auth_info
            .params
            .get("client_secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, "Missing client_secret")
            })?;

        // Validate client
        let client =
            self.provider.get_client(client_id).await?.ok_or_else(|| {
                Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, "Invalid client")
            })?;

        // Verify client secret
        if client.client_secret.as_deref() != Some(client_secret) {
            return Err(Error::protocol(
                ErrorCode::AUTHENTICATION_REQUIRED,
                "Invalid client credentials",
            ));
        }

        // Create auth context (client-only, no user)
        Ok(AuthContext {
            client_id: client.client_id.clone(),
            user_id: client.client_id, // Use client ID as user ID for client credentials
            scopes: client.scopes,
            metadata: serde_json::Map::new(),
        })
    }
}

/// Scope-based authorization middleware.
pub struct ScopeMiddleware {
    /// Inner middleware.
    inner: Box<dyn AuthMiddleware>,

    /// Required scopes.
    required_scopes: Vec<String>,

    /// Require all scopes or any scope.
    require_all: bool,
}

impl std::fmt::Debug for ScopeMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScopeMiddleware")
            .field("inner", &"<dyn AuthMiddleware>")
            .field("required_scopes", &self.required_scopes)
            .finish()
    }
}

impl ScopeMiddleware {
    /// Create a new scope middleware that requires all scopes.
    pub fn all(inner: Box<dyn AuthMiddleware>, scopes: Vec<String>) -> Self {
        Self {
            inner,
            required_scopes: scopes,
            require_all: true,
        }
    }

    /// Create a new scope middleware that requires any scope.
    pub fn any(inner: Box<dyn AuthMiddleware>, scopes: Vec<String>) -> Self {
        Self {
            inner,
            required_scopes: scopes,
            require_all: false,
        }
    }
}

#[async_trait]
impl AuthMiddleware for ScopeMiddleware {
    async fn authenticate(&self, auth_info: Option<&AuthInfo>) -> Result<AuthContext> {
        // First authenticate with inner middleware
        let context = self.inner.authenticate(auth_info).await?;

        // Check scopes
        let scope_refs: Vec<&str> = self.required_scopes.iter().map(|s| s.as_str()).collect();

        let has_required_scopes = if self.require_all {
            context.has_all_scopes(&scope_refs)
        } else {
            context.has_any_scope(&scope_refs)
        };

        if !has_required_scopes {
            return Err(Error::protocol(
                ErrorCode::PERMISSION_DENIED,
                "Insufficient scopes",
            ));
        }

        Ok(context)
    }

    fn is_required(&self) -> bool {
        self.inner.is_required()
    }
}

/// Composite middleware that tries multiple auth methods.
pub struct CompositeMiddleware {
    /// List of middleware to try in order.
    middlewares: Vec<Box<dyn AuthMiddleware>>,

    /// Whether to require at least one to succeed.
    require_any: bool,
}

impl std::fmt::Debug for CompositeMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeMiddleware")
            .field(
                "middlewares",
                &format!("{} middlewares", self.middlewares.len()),
            )
            .field("require_any", &self.require_any)
            .finish()
    }
}

impl CompositeMiddleware {
    /// Create a new composite middleware.
    pub fn new(middlewares: Vec<Box<dyn AuthMiddleware>>) -> Self {
        Self {
            middlewares,
            require_any: true,
        }
    }

    /// Set whether to require at least one middleware to succeed.
    pub fn with_require_any(mut self, require_any: bool) -> Self {
        self.require_any = require_any;
        self
    }
}

#[async_trait]
impl AuthMiddleware for CompositeMiddleware {
    async fn authenticate(&self, auth_info: Option<&AuthInfo>) -> Result<AuthContext> {
        let mut last_error = None;

        // Try each middleware in order
        for middleware in &self.middlewares {
            match middleware.authenticate(auth_info).await {
                Ok(context) => return Ok(context),
                Err(e) => last_error = Some(e),
            }
        }

        // If we get here, all middlewares failed
        if self.require_any {
            Err(last_error.unwrap_or_else(|| {
                Error::protocol(ErrorCode::AUTHENTICATION_REQUIRED, "Authentication failed")
            }))
        } else {
            // Return anonymous context if none required
            Ok(AuthContext {
                client_id: "anonymous".to_string(),
                user_id: "anonymous".to_string(),
                scopes: vec![],
                metadata: serde_json::Map::new(),
            })
        }
    }

    fn is_required(&self) -> bool {
        self.require_any && self.middlewares.iter().any(|m| m.is_required())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::auth::oauth2::InMemoryOAuthProvider;

    #[tokio::test]
    async fn test_bearer_token_middleware() {
        let provider = Arc::new(InMemoryOAuthProvider::new("http://localhost:8080"));
        let middleware = BearerTokenMiddleware::new(provider.clone());

        // Test missing auth
        let result = middleware.authenticate(None).await;
        assert!(result.is_err());

        // Test invalid token
        let auth = AuthInfo::bearer("invalid-token");
        let result = middleware.authenticate(Some(&auth)).await;
        assert!(result.is_err());

        // Create valid token
        let token = provider
            .create_access_token(
                "client-123",
                "user-456",
                vec!["read".to_string(), "write".to_string()],
            )
            .await
            .unwrap();

        // Test valid token
        let auth = AuthInfo::bearer(&token.access_token);
        let context = middleware.authenticate(Some(&auth)).await.unwrap();
        assert_eq!(context.client_id, "client-123");
        assert_eq!(context.user_id, "user-456");
        assert!(context.has_scope("read"));
        assert!(context.has_scope("write"));
    }

    #[tokio::test]
    async fn test_scope_middleware() {
        let provider = Arc::new(InMemoryOAuthProvider::new("http://localhost:8080"));
        let bearer = Box::new(BearerTokenMiddleware::new(provider.clone()));
        let scope_middleware =
            ScopeMiddleware::all(bearer, vec!["read".to_string(), "write".to_string()]);

        // Create token with insufficient scopes
        let token = provider
            .create_access_token("client-123", "user-456", vec!["read".to_string()])
            .await
            .unwrap();

        let auth = AuthInfo::bearer(&token.access_token);
        let result = scope_middleware.authenticate(Some(&auth)).await;
        assert!(result.is_err());

        // Create token with sufficient scopes
        let token = provider
            .create_access_token(
                "client-123",
                "user-456",
                vec!["read".to_string(), "write".to_string()],
            )
            .await
            .unwrap();

        let auth = AuthInfo::bearer(&token.access_token);
        let context = scope_middleware.authenticate(Some(&auth)).await.unwrap();
        assert!(context.has_all_scopes(&["read", "write"]));
    }
}
