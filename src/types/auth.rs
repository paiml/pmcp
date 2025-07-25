//! Authentication-related types for MCP.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthInfo {
    /// Authentication scheme/type
    pub scheme: AuthScheme,

    /// Authentication token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// OAuth-specific information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthInfo>,

    /// Additional authentication parameters
    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}

/// Authentication scheme types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthScheme {
    /// No authentication
    None,
    /// Bearer token authentication
    Bearer,
    /// OAuth 2.0 authentication
    OAuth2,
    /// Custom authentication scheme
    Custom(String),
}

/// OAuth-specific authentication information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthInfo {
    /// OAuth authorization URL
    pub auth_url: String,

    /// OAuth token URL
    pub token_url: String,

    /// OAuth client ID
    pub client_id: String,

    /// OAuth scopes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,

    /// OAuth redirect URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,

    /// PKCE challenge method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pkce_method: Option<PkceMethod>,
}

/// PKCE challenge method for OAuth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PkceMethod {
    /// Plain text (not recommended)
    #[serde(rename = "plain")]
    Plain,
    /// SHA-256 (recommended)
    #[serde(rename = "S256")]
    S256,
}

impl Default for AuthScheme {
    fn default() -> Self {
        Self::None
    }
}

impl AuthInfo {
    /// Create auth info with no authentication.
    pub fn none() -> Self {
        Self {
            scheme: AuthScheme::None,
            token: None,
            oauth: None,
            params: HashMap::new(),
        }
    }

    /// Create auth info with bearer token.
    pub fn bearer(token: impl Into<String>) -> Self {
        Self {
            scheme: AuthScheme::Bearer,
            token: Some(token.into()),
            oauth: None,
            params: HashMap::new(),
        }
    }

    /// Create auth info for OAuth.
    pub fn oauth2(oauth: OAuthInfo) -> Self {
        Self {
            scheme: AuthScheme::OAuth2,
            token: None,
            oauth: Some(oauth),
            params: HashMap::new(),
        }
    }

    /// Check if authentication is required.
    pub fn is_required(&self) -> bool {
        !matches!(self.scheme, AuthScheme::None)
    }

    /// Get the authorization header value if applicable.
    pub fn authorization_header(&self) -> Option<String> {
        match (&self.scheme, &self.token) {
            (AuthScheme::Bearer, Some(token)) => Some(format!("Bearer {}", token)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_none() {
        let auth = AuthInfo::none();
        assert!(!auth.is_required());
        assert_eq!(auth.authorization_header(), None);
    }

    #[test]
    fn auth_bearer() {
        let auth = AuthInfo::bearer("test-token");
        assert!(auth.is_required());
        assert_eq!(
            auth.authorization_header(),
            Some("Bearer test-token".to_string())
        );
    }

    #[test]
    fn oauth_info() {
        let oauth = OAuthInfo {
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            client_id: "test-client".to_string(),
            scopes: Some(vec!["read".to_string(), "write".to_string()]),
            redirect_uri: Some("http://localhost:8080/callback".to_string()),
            pkce_method: Some(PkceMethod::S256),
        };

        let auth = AuthInfo::oauth2(oauth);
        assert!(auth.is_required());
        assert_eq!(auth.scheme, AuthScheme::OAuth2);
    }
}
