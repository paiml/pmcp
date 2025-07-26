//! Server-side roots management.
//!
//! Roots represent directories or files that the server can operate on.
//! Servers can register roots to inform clients about their working directories.

use crate::error::Result;
use crate::types::{ServerNotification, ServerRequest};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Represents a root directory or file that the server can operate on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Root {
    /// The URI identifying the root. This must start with file:// for now.
    pub uri: String,
    /// An optional name for the root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Result of listing roots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRootsResult {
    /// The list of roots.
    pub roots: Vec<Root>,
}

/// Parameters for roots list changed notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootsListChangedParams {}

/// Manages server roots.
#[derive(Clone)]
pub struct RootsManager {
    /// The registered roots.
    roots: Arc<RwLock<Vec<Root>>>,
    /// Optional callback for sending notifications.
    notification_sender: Option<Arc<dyn Fn(ServerNotification) + Send + Sync>>,
}

impl Default for RootsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for RootsManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RootsManager")
            .field(
                "roots",
                &self.roots.try_read().map(|r| r.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl RootsManager {
    /// Create a new roots manager.
    pub fn new() -> Self {
        Self {
            roots: Arc::new(RwLock::new(Vec::new())),
            notification_sender: None,
        }
    }

    /// Set the notification sender callback.
    ///
    /// This should be called after the server is initialized with a transport.
    pub fn set_notification_sender<F>(&mut self, sender: F)
    where
        F: Fn(ServerNotification) + Send + Sync + 'static,
    {
        self.notification_sender = Some(Arc::new(sender));
    }

    /// Register a root directory or file.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the root (must start with file://)
    /// * `name` - Optional human-readable name for the root
    ///
    /// # Returns
    ///
    /// A function that can be called to unregister the root.
    ///
    /// # Errors
    ///
    /// Returns an error if the URI doesn't start with file://
    pub async fn register_root<S: Into<String>>(
        &self,
        uri: S,
        name: Option<String>,
    ) -> Result<impl FnOnce() + Send + 'static> {
        let uri = uri.into();

        if !uri.starts_with("file://") {
            return Err(crate::error::Error::invalid_params(
                "Root URI must start with file://",
            ));
        }

        let root = Root {
            uri: uri.clone(),
            name,
        };

        {
            self.roots.write().await.push(root);
            info!("Registered root: {}", uri);
        }

        // Send notification
        self.send_roots_list_changed();

        // Return unregister function
        let roots = self.roots.clone();
        let notification_sender = self.notification_sender.clone();
        let unregister_uri = uri.clone();

        Ok(move || {
            let roots = roots.clone();
            let notification_sender = notification_sender.clone();
            let uri = unregister_uri.clone();

            tokio::spawn(async move {
                let mut roots_guard = roots.write().await;
                if let Some(pos) = roots_guard.iter().position(|r| r.uri == uri) {
                    roots_guard.remove(pos);
                    drop(roots_guard);
                    info!("Unregistered root: {}", uri);

                    // Send notification
                    if let Some(sender) = notification_sender {
                        sender(ServerNotification::RootsListChanged);
                    }
                }
            });
        })
    }

    /// Get a copy of all registered roots.
    pub async fn get_roots(&self) -> Vec<Root> {
        self.roots.read().await.clone()
    }

    /// Request the list of roots from the client.
    ///
    /// This is useful when the server needs to know what directories the client has access to.
    ///
    /// # Arguments
    ///
    /// * `request_sender` - Function to send requests to the client
    pub async fn request_client_roots<F, Fut>(&self, request_sender: F) -> Result<ListRootsResult>
    where
        F: FnOnce(ServerRequest) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value>>,
    {
        let request = ServerRequest::ListRoots;
        let response = request_sender(request).await?;

        serde_json::from_value(response).map_err(|e| {
            crate::error::Error::protocol_msg(format!("Invalid roots response: {}", e))
        })
    }

    /// Check if any roots are registered.
    pub async fn has_roots(&self) -> bool {
        !self.roots.read().await.is_empty()
    }

    /// Send a notification that the roots list has changed.
    fn send_roots_list_changed(&self) {
        if let Some(sender) = &self.notification_sender {
            sender(ServerNotification::RootsListChanged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_root() {
        let manager = RootsManager::new();

        // Register a root
        let _unregister = manager
            .register_root("file:///home/user/project", Some("My Project".to_string()))
            .await
            .unwrap();

        let roots = manager.get_roots().await;
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].uri, "file:///home/user/project");
        assert_eq!(roots[0].name, Some("My Project".to_string()));
    }

    #[tokio::test]
    async fn test_register_root_without_name() {
        let manager = RootsManager::new();

        let _unregister = manager
            .register_root("file:///home/user/project", None)
            .await
            .unwrap();

        let roots = manager.get_roots().await;
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].uri, "file:///home/user/project");
        assert_eq!(roots[0].name, None);
    }

    #[tokio::test]
    async fn test_invalid_root_uri() {
        let manager = RootsManager::new();

        let result = manager
            .register_root("https://example.com/project", None)
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(e) if e.to_string().contains("must start with file://")));
    }

    #[tokio::test]
    async fn test_multiple_roots() {
        let manager = RootsManager::new();

        let _u1 = manager
            .register_root("file:///home/user/project1", Some("Project 1".to_string()))
            .await
            .unwrap();
        let _u2 = manager
            .register_root("file:///home/user/project2", Some("Project 2".to_string()))
            .await
            .unwrap();
        let _u3 = manager
            .register_root("file:///home/user/project3", None)
            .await
            .unwrap();

        let roots = manager.get_roots().await;
        assert_eq!(roots.len(), 3);

        let names: Vec<Option<String>> = roots.iter().map(|r| r.name.clone()).collect();
        assert_eq!(
            names,
            vec![
                Some("Project 1".to_string()),
                Some("Project 2".to_string()),
                None
            ]
        );
    }

    #[tokio::test]
    async fn test_unregister_root() {
        let manager = RootsManager::new();

        let unregister1 = manager
            .register_root("file:///home/user/project1", None)
            .await
            .unwrap();
        let _u2 = manager
            .register_root("file:///home/user/project2", None)
            .await
            .unwrap();
        let _u3 = manager
            .register_root("file:///home/user/project3", None)
            .await
            .unwrap();

        assert_eq!(manager.get_roots().await.len(), 3);

        // Unregister the first root
        unregister1();

        // Give the async unregister task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let roots = manager.get_roots().await;
        assert_eq!(roots.len(), 2);

        let uris: Vec<String> = roots.iter().map(|r| r.uri.clone()).collect();
        assert_eq!(
            uris,
            vec!["file:///home/user/project2", "file:///home/user/project3"]
        );
    }

    #[tokio::test]
    async fn test_notification_sent() {
        use std::sync::Mutex;

        let manager = RootsManager::new();
        let notifications = Arc::new(Mutex::new(Vec::new()));

        // Set up notification sender
        let notifications_clone = notifications.clone();
        let mut manager_mut = manager.clone();
        manager_mut.set_notification_sender(move |notif| {
            notifications_clone.lock().unwrap().push(notif);
        });

        // Register a root
        let _unregister = manager_mut
            .register_root("file:///home/user/project", None)
            .await
            .unwrap();

        // Check notification was sent
        {
            let notifs = notifications.lock().unwrap();
            assert_eq!(notifs.len(), 1);
            assert!(matches!(notifs[0], ServerNotification::RootsListChanged));
            drop(notifs);
        }
    }

    #[tokio::test]
    async fn test_get_roots_returns_copy() {
        let manager = RootsManager::new();

        let _u1 = manager
            .register_root("file:///home/user/project1", None)
            .await
            .unwrap();
        let _u2 = manager
            .register_root("file:///home/user/project2", None)
            .await
            .unwrap();

        let roots1 = manager.get_roots().await;
        let roots2 = manager.get_roots().await;

        // Different Vec instances but same content
        assert_eq!(roots1, roots2);
        assert_eq!(roots1.len(), 2);
    }
}
