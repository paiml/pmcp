//! Session management for HTTP-based transports.
//!
//! This module provides session lifecycle management for HTTP and SSE transports,
//! including session creation, persistence, and termination.

use crate::error::{Error, ErrorCode, Result};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// Session configuration options.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Session timeout duration.
    pub timeout: Duration,

    /// Maximum number of concurrent sessions.
    pub max_sessions: usize,

    /// Whether to persist sessions across restarts.
    pub persistent: bool,

    /// Session cookie name.
    pub cookie_name: String,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::hours(24),
            max_sessions: 1000,
            persistent: false,
            cookie_name: "mcp-session-id".to_string(),
        }
    }
}

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: String,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp.
    pub last_activity: DateTime<Utc>,

    /// Session expiry timestamp.
    pub expires_at: DateTime<Utc>,

    /// Client information.
    pub client_info: Option<ClientInfo>,

    /// Custom session data.
    pub data: serde_json::Value,

    /// Whether the session is authenticated.
    pub authenticated: bool,

    /// Authentication info if authenticated.
    pub auth_info: Option<String>,
}

/// Client information stored in session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// User agent string.
    pub user_agent: Option<String>,

    /// Client IP address.
    pub ip_address: Option<String>,

    /// Client protocol version.
    pub protocol_version: String,
}

/// Session manager for handling session lifecycle.
pub struct SessionManager {
    /// Session storage.
    sessions: Arc<DashMap<String, Session>>,

    /// Configuration.
    config: SessionConfig,

    /// Session event callbacks.
    callbacks: Arc<SessionCallbacks>,
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("sessions", &self.sessions.len())
            .field("config", &self.config)
            .field("callbacks", &"Arc<SessionCallbacks>")
            .finish()
    }
}

/// Session callback type.
pub type SessionCallback = Box<dyn Fn(&Session) + Send + Sync>;

/// Callbacks for session lifecycle events.
#[derive(Default)]
pub struct SessionCallbacks {
    /// Called when a session is created.
    pub on_create: Option<SessionCallback>,

    /// Called when a session is destroyed.
    pub on_destroy: Option<SessionCallback>,

    /// Called when a session expires.
    pub on_expire: Option<SessionCallback>,
}

impl std::fmt::Debug for SessionCallbacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCallbacks")
            .field("on_create", &self.on_create.is_some())
            .field("on_destroy", &self.on_destroy.is_some())
            .field("on_expire", &self.on_expire.is_some())
            .finish()
    }
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            config,
            callbacks: Arc::new(SessionCallbacks::default()),
        }
    }

    /// Create a new session.
    pub fn create_session(&self, client_info: Option<ClientInfo>) -> Result<Session> {
        // Check session limit
        if self.sessions.len() >= self.config.max_sessions {
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                "Maximum session limit reached",
            ));
        }

        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            last_activity: now,
            expires_at: now + self.config.timeout,
            client_info,
            data: serde_json::Value::Object(serde_json::Map::new()),
            authenticated: false,
            auth_info: None,
        };

        self.sessions.insert(session.id.clone(), session.clone());

        // Call creation callback
        if let Some(callback) = &self.callbacks.on_create {
            callback(&session);
        }

        info!("Created new session: {}", session.id);
        Ok(session)
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|entry| {
            let mut session = entry.clone();

            // Update last activity
            session.last_activity = Utc::now();

            // Update in storage
            drop(entry);
            self.sessions
                .insert(session_id.to_string(), session.clone());

            session
        })
    }

    /// Validate and refresh a session.
    pub fn validate_session(&self, session_id: &str) -> Result<Session> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid session ID"))?;

        // Check if expired
        if session.expires_at < Utc::now() {
            self.destroy_session(session_id)?;
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Session expired",
            ));
        }

        Ok(session)
    }

    /// Update session data.
    pub fn update_session<F>(&self, session_id: &str, updater: F) -> Result<()>
    where
        F: FnOnce(&mut Session),
    {
        let mut session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::protocol(ErrorCode::INVALID_REQUEST, "Invalid session ID"))?;

        updater(&mut session);
        session.last_activity = Utc::now();

        Ok(())
    }

    /// Authenticate a session.
    pub fn authenticate_session(&self, session_id: &str, auth_info: String) -> Result<()> {
        self.update_session(session_id, |session| {
            session.authenticated = true;
            session.auth_info = Some(auth_info);
        })
    }

    /// Destroy a session.
    pub fn destroy_session(&self, session_id: &str) -> Result<()> {
        let session = self.sessions.remove(session_id).map(|(_, session)| session);

        if let Some(session) = session {
            // Call destruction callback
            if let Some(callback) = &self.callbacks.on_destroy {
                callback(&session);
            }

            info!("Destroyed session: {}", session_id);
            Ok(())
        } else {
            Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Session not found",
            ))
        }
    }

    /// Clean up expired sessions.
    pub fn cleanup_expired(&self) {
        let now = Utc::now();
        let expired: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| entry.expires_at < now)
            .map(|entry| entry.key().clone())
            .collect();

        for session_id in expired {
            if let Some((_, session)) = self.sessions.remove(&session_id) {
                // Call expiry callback
                if let Some(callback) = &self.callbacks.on_expire {
                    callback(&session);
                }

                debug!("Expired session: {}", session_id);
            }
        }
    }

    /// Get all active sessions.
    pub fn active_sessions(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get session count.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Set session callbacks.
    pub fn set_callbacks(&mut self, callbacks: SessionCallbacks) {
        self.callbacks = Arc::new(callbacks);
    }
}

/// Session cleanup task that runs periodically.
pub async fn session_cleanup_task(manager: Arc<SessionManager>, interval: Duration) {
    let mut interval = tokio::time::interval(
        interval
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(300)),
    );

    loop {
        interval.tick().await;
        manager.cleanup_expired();
    }
}

/// Extract session ID from HTTP headers.
pub fn extract_session_id(headers: &std::collections::HashMap<String, String>) -> Option<String> {
    // Try cookie first
    if let Some(cookie_header) = headers.get("cookie").or_else(|| headers.get("Cookie")) {
        for cookie in cookie_header.split(';') {
            let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
            if parts.len() == 2 && parts[0] == "mcp-session-id" {
                return Some(parts[1].to_string());
            }
        }
    }

    // Try X-Session-ID header
    headers
        .get("X-Session-ID")
        .or_else(|| headers.get("x-session-id"))
        .map(|s| s.to_string())
}

/// Session middleware for HTTP requests.
pub struct SessionMiddleware {
    manager: Arc<SessionManager>,
}

impl std::fmt::Debug for SessionMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionMiddleware")
            .field("manager", &"Arc<SessionManager>")
            .finish()
    }
}

impl SessionMiddleware {
    /// Create new session middleware.
    pub fn new(manager: Arc<SessionManager>) -> Self {
        Self { manager }
    }

    /// Process request with session handling.
    pub async fn process<F, R>(
        &self,
        headers: &std::collections::HashMap<String, String>,
        handler: F,
    ) -> Result<(Option<Session>, R)>
    where
        F: FnOnce(Option<Session>) -> R,
    {
        let session = if let Some(session_id) = extract_session_id(headers) {
            self.manager.get_session(&session_id)
        } else {
            None
        };

        let result = handler(session.clone());
        Ok((session, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let manager = SessionManager::new(SessionConfig::default());

        let session = manager.create_session(None).unwrap();
        assert!(!session.id.is_empty());
        assert!(!session.authenticated);
        assert_eq!(manager.session_count(), 1);
    }

    #[test]
    fn test_session_validation() {
        let manager = SessionManager::new(SessionConfig::default());

        let session = manager.create_session(None).unwrap();
        let validated = manager.validate_session(&session.id).unwrap();
        assert_eq!(session.id, validated.id);

        // Invalid session
        let result = manager.validate_session("invalid-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_session_authentication() {
        let manager = SessionManager::new(SessionConfig::default());

        let session = manager.create_session(None).unwrap();
        manager
            .authenticate_session(&session.id, "user123".to_string())
            .unwrap();

        let updated = manager.get_session(&session.id).unwrap();
        assert!(updated.authenticated);
        assert_eq!(updated.auth_info, Some("user123".to_string()));
    }

    #[test]
    fn test_session_destruction() {
        let manager = SessionManager::new(SessionConfig::default());

        let session = manager.create_session(None).unwrap();
        assert_eq!(manager.session_count(), 1);

        manager.destroy_session(&session.id).unwrap();
        assert_eq!(manager.session_count(), 0);

        // Should fail to get destroyed session
        assert!(manager.get_session(&session.id).is_none());
    }

    #[test]
    fn test_session_expiry() {
        let config = SessionConfig {
            timeout: Duration::milliseconds(100), // Very short timeout
            ..Default::default()
        };
        let manager = SessionManager::new(config);

        let session = manager.create_session(None).unwrap();

        // Manually set expiry to past
        manager
            .update_session(&session.id, |s| {
                s.expires_at = Utc::now() - Duration::seconds(1);
            })
            .unwrap();

        manager.cleanup_expired();
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_session_limit() {
        let config = SessionConfig {
            max_sessions: 2,
            ..Default::default()
        };
        let manager = SessionManager::new(config);

        // Create max sessions
        manager.create_session(None).unwrap();
        manager.create_session(None).unwrap();

        // Should fail to create more
        let result = manager.create_session(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_session_id() {
        let mut headers = std::collections::HashMap::new();

        // Test cookie extraction
        headers.insert(
            "Cookie".to_string(),
            "mcp-session-id=test123; other=value".to_string(),
        );
        assert_eq!(extract_session_id(&headers), Some("test123".to_string()));

        // Test header extraction
        headers.clear();
        headers.insert("X-Session-ID".to_string(), "test456".to_string());
        assert_eq!(extract_session_id(&headers), Some("test456".to_string()));

        // Test no session
        headers.clear();
        assert_eq!(extract_session_id(&headers), None);
    }
}
