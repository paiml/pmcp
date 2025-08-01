//! Server-side authentication providers and middleware.

pub mod middleware;
pub mod oauth2;

pub use middleware::{
    AuthContext, AuthMiddleware, BearerTokenMiddleware, ClientCredentialsMiddleware,
    CompositeMiddleware, ScopeMiddleware,
};

pub use oauth2::{
    AccessToken, AuthorizationCode, AuthorizationRequest, GrantType, InMemoryOAuthProvider,
    OAuthClient, OAuthError, OAuthMetadata, OAuthProvider, ProxyOAuthProvider, ResponseType,
    RevocationRequest, TokenInfo, TokenRequest, TokenType,
};
