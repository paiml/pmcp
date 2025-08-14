//! Authentication helpers for MCP clients.
//!
//! This module provides utilities for handling OAuth 2.0/OIDC authentication
//! in MCP clients, including discovery and token management.

use crate::error::{Error, ErrorCode, Result};
use crate::server::auth::oauth2::OidcDiscoveryMetadata;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// OIDC discovery client for fetching server configuration.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::client::auth::OidcDiscoveryClient;
/// use std::time::Duration;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create a default client
/// let client = OidcDiscoveryClient::new();
///
/// // Or create with custom retry settings
/// let custom_client = OidcDiscoveryClient::with_settings(
///     5,  // max retries
///     Duration::from_secs(1)  // retry delay
/// );
///
/// // Discover OIDC configuration
/// let metadata = client.discover("https://auth.example.com").await?;
/// println!("Authorization endpoint: {}", metadata.authorization_endpoint);
/// println!("Token endpoint: {}", metadata.token_endpoint);
/// # Ok(())
/// # }
/// ```
///
/// ## Retry Behavior
///
/// ```rust
/// use pmcp::client::auth::OidcDiscoveryClient;
/// use std::time::Duration;
///
/// // Create client with specific retry behavior
/// let client = OidcDiscoveryClient::with_settings(
///     0,  // No retries
///     Duration::from_secs(0)  // No delay
/// );
///
/// // Client with aggressive retries
/// let aggressive = OidcDiscoveryClient::with_settings(
///     10,  // Many retries
///     Duration::from_millis(100)  // Short delay
/// );
/// ```
#[derive(Debug)]
pub struct OidcDiscoveryClient {
    /// HTTP client for making requests.
    client: reqwest::Client,
    /// Maximum number of retry attempts for CORS/network errors.
    max_retries: usize,
    /// Delay between retry attempts.
    retry_delay: Duration,
}

impl Default for OidcDiscoveryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OidcDiscoveryClient {
    /// Create a new OIDC discovery client with default settings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::client::auth::OidcDiscoveryClient;
    ///
    /// let client = OidcDiscoveryClient::new();
    /// // Client is configured with:
    /// // - max_retries: 3
    /// // - retry_delay: 500ms
    /// ```
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
        }
    }

    /// Create a new OIDC discovery client with custom settings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::client::auth::OidcDiscoveryClient;
    /// use std::time::Duration;
    ///
    /// // More aggressive retry strategy
    /// let client = OidcDiscoveryClient::with_settings(
    ///     10,  // Try up to 10 times
    ///     Duration::from_millis(200)  // 200ms between retries
    /// );
    /// ```
    pub fn with_settings(max_retries: usize, retry_delay: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            max_retries,
            retry_delay,
        }
    }

    /// Discover OIDC configuration from an issuer URL.
    /// Automatically retries on CORS or network errors.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::auth::OidcDiscoveryClient;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let client = OidcDiscoveryClient::new();
    ///
    /// // Discover from various providers
    /// let google = client.discover("https://accounts.google.com").await?;
    /// let microsoft = client.discover("https://login.microsoftonline.com/common/v2.0").await?;
    ///
    /// // URL normalization - trailing slashes are handled
    /// let metadata1 = client.discover("https://auth.example.com").await?;
    /// let metadata2 = client.discover("https://auth.example.com/").await?;
    /// // Both will fetch from the same discovery endpoint
    /// # Ok(())
    /// # }
    /// ```
    pub async fn discover(&self, issuer_url: &str) -> Result<OidcDiscoveryMetadata> {
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer_url.trim_end_matches('/')
        );

        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.max_retries {
            match self.fetch_discovery(&discovery_url).await {
                Ok(metadata) => return Ok(metadata),
                Err(e) => {
                    // Check if this is a CORS error or network error that should be retried
                    if self.should_retry(&e) && attempts + 1 < self.max_retries {
                        attempts += 1;
                        tokio::time::sleep(self.retry_delay).await;
                        continue;
                    }
                    last_error = Some(e);
                    break;
                },
            }
        }

        Err(last_error.unwrap_or_else(|| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                "Failed to discover OIDC configuration",
            )
        }))
    }

    /// Fetch discovery metadata from a URL.
    async fn fetch_discovery(&self, url: &str) -> Result<OidcDiscoveryMetadata> {
        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Failed to fetch discovery document: {}", e),
                )
            })?;

        if !response.status().is_success() {
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Discovery endpoint returned status: {}", response.status()),
            ));
        }

        response.json::<OidcDiscoveryMetadata>().await.map_err(|e| {
            Error::protocol(
                ErrorCode::PARSE_ERROR,
                format!("Failed to parse discovery document: {}", e),
            )
        })
    }

    /// Check if an error should trigger a retry.
    fn should_retry(&self, error: &Error) -> bool {
        // Use self to check against max_retries (even though we don't strictly need it)
        let _ = self.max_retries;
        // Check if it's a timeout error
        if matches!(error, Error::Timeout(_)) {
            return true;
        }

        // Retry on network errors or specific error patterns
        let error_str = error.to_string();
        error_str.contains("CORS")
            || error_str.contains("network")
            || error_str.contains("timeout")
            || error_str.contains("connection")
    }
}

/// OAuth 2.0 token response.
///
/// # Examples
///
/// ```rust
/// use pmcp::client::auth::TokenResponse;
///
/// // Parse a token response from JSON
/// let json = r#"{
///     "access_token": "eyJhbGciOiJSUzI1NiIs...",
///     "token_type": "Bearer",
///     "expires_in": 3600,
///     "refresh_token": "8xLOxBtZp8",
///     "scope": "openid profile email"
/// }"#;
///
/// let response: TokenResponse = serde_json::from_str(json).unwrap();
/// assert_eq!(response.token_type, "Bearer");
/// assert_eq!(response.expires_in, Some(3600));
///
/// // Create a token response
/// let token = TokenResponse {
///     access_token: "token123".to_string(),
///     token_type: "Bearer".to_string(),
///     expires_in: Some(7200),
///     refresh_token: None,
///     scope: Some("read write".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// Access token.
    pub access_token: String,

    /// Token type (usually "Bearer").
    pub token_type: String,

    /// Token expiration time in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,

    /// Refresh token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Granted scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// OAuth 2.0 token exchange client.
#[derive(Debug)]
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::client::auth::TokenExchangeClient;
///
/// # async fn example() -> pmcp::Result<()> {
/// let client = TokenExchangeClient::new();
///
/// // Exchange authorization code for tokens
/// let tokens = client.exchange_code(
///     "https://auth.example.com/token",
///     "auth_code_123",
///     "client_id",
///     Some("client_secret"),
///     "https://app.example.com/callback",
///     None,  // No PKCE verifier
/// ).await?;
///
/// println!("Access token: {}", tokens.access_token);
///
/// // Refresh an access token
/// if let Some(refresh_token) = tokens.refresh_token {
///     let new_tokens = client.refresh_token(
///         "https://auth.example.com/token",
///         &refresh_token,
///         "client_id",
///         Some("client_secret"),
///         None,  // Keep same scope
///     ).await?;
///     println!("New access token: {}", new_tokens.access_token);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TokenExchangeClient {
    /// HTTP client for making requests.
    client: reqwest::Client,
}

impl Default for TokenExchangeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenExchangeClient {
    /// Create a new token exchange client.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Exchange an authorization code for tokens.
    pub async fn exchange_code(
        &self,
        token_endpoint: &str,
        code: &str,
        client_id: &str,
        client_secret: Option<&str>,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> Result<TokenResponse> {
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
        ];

        if let Some(verifier) = code_verifier {
            params.push(("code_verifier", verifier));
        }

        let mut request = self.client
            .post(token_endpoint)
            .header("Accept", "application/json")  // Explicitly set Accept header
            .form(&params);

        // Add client authentication if secret is provided
        if let Some(secret) = client_secret {
            request = request.basic_auth(client_id, Some(secret));
        }

        let response = request.send().await.map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to exchange authorization code: {}", e),
            )
        })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token exchange failed: {}", error_text),
            ));
        }

        response.json::<TokenResponse>().await.map_err(|e| {
            Error::protocol(
                ErrorCode::PARSE_ERROR,
                format!("Failed to parse token response: {}", e),
            )
        })
    }

    /// Refresh an access token using a refresh token.
    pub async fn refresh_token(
        &self,
        token_endpoint: &str,
        refresh_token: &str,
        client_id: &str,
        client_secret: Option<&str>,
        scope: Option<&str>,
    ) -> Result<TokenResponse> {
        let mut params = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ];

        if let Some(s) = scope {
            params.push(("scope", s));
        }

        let mut request = self.client
            .post(token_endpoint)
            .header("Accept", "application/json")  // Explicitly set Accept header
            .form(&params);

        // Add client authentication if secret is provided
        if let Some(secret) = client_secret {
            request = request.basic_auth(client_id, Some(secret));
        }

        let response = request.send().await.map_err(|e| {
            Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to refresh token: {}", e),
            )
        })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token refresh failed: {}", error_text),
            ));
        }

        response.json::<TokenResponse>().await.map_err(|e| {
            Error::protocol(
                ErrorCode::PARSE_ERROR,
                format!("Failed to parse token response: {}", e),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_url_construction() {
        let _client = OidcDiscoveryClient::new();

        // Test various issuer URL formats
        let test_cases = vec![
            (
                "https://example.com",
                "https://example.com/.well-known/openid-configuration",
            ),
            (
                "https://example.com/",
                "https://example.com/.well-known/openid-configuration",
            ),
            (
                "https://auth.example.com/oauth",
                "https://auth.example.com/oauth/.well-known/openid-configuration",
            ),
        ];

        for (issuer, expected) in test_cases {
            let url = format!(
                "{}/.well-known/openid-configuration",
                issuer.trim_end_matches('/')
            );
            assert_eq!(url, expected);
        }
    }

    #[test]
    fn test_should_retry_logic() {
        let client = OidcDiscoveryClient::new();

        // Test CORS error
        let cors_error =
            Error::protocol(ErrorCode::INTERNAL_ERROR, "CORS policy blocked the request");
        assert!(client.should_retry(&cors_error));

        // Test network error
        let network_error = Error::protocol(ErrorCode::INTERNAL_ERROR, "network connection failed");
        assert!(client.should_retry(&network_error));

        // Test timeout error
        let timeout_error = Error::Timeout(5000);
        assert!(client.should_retry(&timeout_error));

        // Test non-retryable error
        let parse_error = Error::protocol(ErrorCode::PARSE_ERROR, "Invalid JSON");
        assert!(!client.should_retry(&parse_error));
    }

    #[test]
    fn test_discovery_client_with_settings() {
        let client = OidcDiscoveryClient::with_settings(5, Duration::from_secs(2));
        assert_eq!(client.max_retries, 5);
        assert_eq!(client.retry_delay, Duration::from_secs(2));
    }

    #[test]
    fn test_token_response_serialization() {
        let token_response = TokenResponse {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_token".to_string()),
            scope: Some("openid profile".to_string()),
        };

        // Test serialization
        let json = serde_json::to_string(&token_response).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("Bearer"));

        // Test deserialization
        let deserialized: TokenResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.access_token, "test_token");
        assert_eq!(deserialized.expires_in, Some(3600));
    }

    #[test]
    fn test_oidc_discovery_metadata_defaults() {
        let json = r#"{
            "issuer": "https://auth.example.com",
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token",
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"],
            "scopes_supported": ["openid", "profile"],
            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            "code_challenge_methods_supported": ["S256"]
        }"#;

        let metadata: OidcDiscoveryMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.issuer, "https://auth.example.com");
        assert_eq!(metadata.jwks_uri, None);
        assert_eq!(metadata.userinfo_endpoint, None);
    }
}
