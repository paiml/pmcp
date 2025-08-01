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
///
/// # Examples
///
/// ```rust
/// use pmcp::types::auth::{OAuthInfo, PkceMethod};
///
/// // Basic OAuth configuration
/// let oauth = OAuthInfo {
///     auth_url: "https://provider.com/oauth/authorize".to_string(),
///     token_url: "https://provider.com/oauth/token".to_string(),
///     client_id: "your-client-id".to_string(),
///     scopes: Some(vec!["read".to_string(), "write".to_string()]),
///     redirect_uri: Some("http://localhost:8080/callback".to_string()),
///     pkce_method: Some(PkceMethod::S256),
/// };
///
/// // GitHub OAuth configuration
/// let github_oauth = OAuthInfo {
///     auth_url: "https://github.com/login/oauth/authorize".to_string(),
///     token_url: "https://github.com/login/oauth/access_token".to_string(),
///     client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
///     scopes: Some(vec![
///         "repo".to_string(),
///         "user:email".to_string(),
///         "read:org".to_string(),
///     ]),
///     redirect_uri: Some("http://localhost:3000/auth/github/callback".to_string()),
///     pkce_method: Some(PkceMethod::S256), // Use PKCE for enhanced security
/// };
///
/// // Google OAuth configuration
/// let google_oauth = OAuthInfo {
///     auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
///     token_url: "https://oauth2.googleapis.com/token".to_string(),
///     client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
///     scopes: Some(vec![
///         "https://www.googleapis.com/auth/drive.readonly".to_string(),
///         "https://www.googleapis.com/auth/userinfo.email".to_string(),
///     ]),
///     redirect_uri: Some("http://localhost:3000/auth/google/callback".to_string()),
///     pkce_method: Some(PkceMethod::S256),
/// };
///
/// // Build authorization URL with parameters
/// fn build_auth_url(oauth: &OAuthInfo, state: &str) -> String {
///     let mut url = oauth.auth_url.clone();
///     url.push_str("?client_id=");
///     url.push_str(&oauth.client_id);
///     url.push_str("&redirect_uri=");
///     url.push_str(oauth.redirect_uri.as_ref().unwrap());
///     url.push_str("&response_type=code");
///     url.push_str("&state=");
///     url.push_str(state);
///     
///     if let Some(scopes) = &oauth.scopes {
///         url.push_str("&scope=");
///         url.push_str(&scopes.join(" "));
///     }
///     
///     url
/// }
/// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::AuthInfo;
    ///
    /// // Create auth info for endpoints that don't require authentication
    /// let auth = AuthInfo::none();
    /// assert!(!auth.is_required());
    /// assert_eq!(auth.authorization_header(), None);
    ///
    /// // Use in client configuration
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// // Client uses no auth by default
    /// # Ok(())
    /// # }
    /// ```
    pub fn none() -> Self {
        Self {
            scheme: AuthScheme::None,
            token: None,
            oauth: None,
            params: HashMap::new(),
        }
    }

    /// Create auth info with bearer token.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::AuthInfo;
    ///
    /// // Create auth info with a bearer token
    /// let auth = AuthInfo::bearer("my-api-token-123");
    /// assert!(auth.is_required());
    /// assert_eq!(auth.authorization_header(), Some("Bearer my-api-token-123".to_string()));
    ///
    /// // Use environment variable for token
    /// # std::env::set_var("API_TOKEN", "secret-token");
    /// let token = std::env::var("API_TOKEN").unwrap_or_default();
    /// let auth = AuthInfo::bearer(token);
    ///
    /// // Use in client configuration with bearer auth
    /// let auth2 = AuthInfo::bearer("secret-api-key");
    /// // This auth info can be used when configuring HTTP transports
    /// ```
    pub fn bearer(token: impl Into<String>) -> Self {
        Self {
            scheme: AuthScheme::Bearer,
            token: Some(token.into()),
            oauth: None,
            params: HashMap::new(),
        }
    }

    /// Create auth info for OAuth.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{AuthInfo, types::auth::{OAuthInfo, PkceMethod}};
    ///
    /// // Configure OAuth 2.0 authentication
    /// let oauth_info = OAuthInfo {
    ///     auth_url: "https://auth.example.com/authorize".to_string(),
    ///     token_url: "https://auth.example.com/token".to_string(),
    ///     client_id: "my-client-id".to_string(),
    ///     scopes: Some(vec!["read".to_string(), "write".to_string()]),
    ///     redirect_uri: Some("http://localhost:8080/callback".to_string()),
    ///     pkce_method: Some(PkceMethod::S256), // Recommended for security
    /// };
    ///
    /// let auth = AuthInfo::oauth2(oauth_info);
    /// assert!(auth.is_required());
    ///
    /// // GitHub OAuth example
    /// let github_oauth = OAuthInfo {
    ///     auth_url: "https://github.com/login/oauth/authorize".to_string(),
    ///     token_url: "https://github.com/login/oauth/access_token".to_string(),
    ///     client_id: "your-github-app-id".to_string(),
    ///     scopes: Some(vec!["repo".to_string(), "user:email".to_string()]),
    ///     redirect_uri: Some("http://localhost:3000/auth/callback".to_string()),
    ///     pkce_method: Some(PkceMethod::S256),
    /// };
    ///
    /// let github_auth = AuthInfo::oauth2(github_oauth);
    /// ```
    pub fn oauth2(oauth: OAuthInfo) -> Self {
        Self {
            scheme: AuthScheme::OAuth2,
            token: None,
            oauth: Some(oauth),
            params: HashMap::new(),
        }
    }

    /// Check if authentication is required.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{AuthInfo, types::auth::AuthScheme};
    ///
    /// // No authentication
    /// let no_auth = AuthInfo::none();
    /// assert!(!no_auth.is_required());
    ///
    /// // Bearer token authentication
    /// let bearer_auth = AuthInfo::bearer("token");
    /// assert!(bearer_auth.is_required());
    ///
    /// // Custom authentication scheme
    /// let mut custom_auth = AuthInfo::none();
    /// custom_auth.scheme = AuthScheme::Custom("ApiKey".to_string());
    /// assert!(custom_auth.is_required());
    ///
    /// // Use in conditional logic
    /// fn process_request(auth: &AuthInfo) {
    ///     if auth.is_required() {
    ///         println!("Authentication required: {:?}", auth.scheme);
    ///     } else {
    ///         println!("No authentication needed");
    ///     }
    /// }
    /// ```
    pub fn is_required(&self) -> bool {
        !matches!(self.scheme, AuthScheme::None)
    }

    /// Get the authorization header value if applicable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::AuthInfo;
    /// use std::collections::HashMap;
    ///
    /// // Bearer token generates authorization header
    /// let bearer_auth = AuthInfo::bearer("my-secret-token");
    /// assert_eq!(
    ///     bearer_auth.authorization_header(),
    ///     Some("Bearer my-secret-token".to_string())
    /// );
    ///
    /// // No auth returns None
    /// let no_auth = AuthInfo::none();
    /// assert_eq!(no_auth.authorization_header(), None);
    ///
    /// // OAuth returns None (uses different flow)
    /// # use pmcp::types::auth::OAuthInfo;
    /// let oauth_auth = AuthInfo::oauth2(OAuthInfo {
    ///     auth_url: "https://example.com/auth".to_string(),
    ///     token_url: "https://example.com/token".to_string(),
    ///     client_id: "client".to_string(),
    ///     scopes: None,
    ///     redirect_uri: None,
    ///     pkce_method: None,
    /// });
    /// assert_eq!(oauth_auth.authorization_header(), None);
    ///
    /// // Use in HTTP headers
    /// let mut headers = HashMap::new();
    /// if let Some(auth_header) = bearer_auth.authorization_header() {
    ///     headers.insert("Authorization".to_string(), auth_header);
    /// }
    /// ```
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
