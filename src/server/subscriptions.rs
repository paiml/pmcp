//! Server-side resource subscription management.

use crate::error::Result;
use crate::types::{protocol::ResourceUpdatedParams, ServerNotification};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages resource subscriptions for the server.
///
/// This struct keeps track of which resources are subscribed to
/// and provides methods to notify subscribers when resources change.
#[derive(Clone)]
pub struct SubscriptionManager {
    /// Map of resource URI to set of subscriber IDs
    subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Optional callback for sending notifications
    notification_sender: Option<Arc<dyn Fn(ServerNotification) + Send + Sync>>,
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SubscriptionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionManager")
            .field(
                "subscriptions",
                &self.subscriptions.try_read().map(|s| s.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
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

    /// Subscribe to a resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to subscribe to
    /// * `subscriber_id` - Unique identifier for the subscriber (usually session ID)
    pub async fn subscribe(&self, uri: String, subscriber_id: String) -> Result<()> {
        self.subscriptions
            .write()
            .await
            .entry(uri)
            .or_default()
            .insert(subscriber_id);
        Ok(())
    }

    /// Unsubscribe from a resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to unsubscribe from
    /// * `subscriber_id` - Unique identifier for the subscriber
    pub async fn unsubscribe(&self, uri: String, subscriber_id: String) -> Result<()> {
        let mut subs = self.subscriptions.write().await;
        if let Some(subscribers) = subs.get_mut(&uri) {
            subscribers.remove(&subscriber_id);
            if subscribers.is_empty() {
                subs.remove(&uri);
                drop(subs);
            }
        }
        Ok(())
    }

    /// Unsubscribe from all resources for a given subscriber.
    ///
    /// This is useful when a client disconnects.
    ///
    /// # Arguments
    ///
    /// * `subscriber_id` - Unique identifier for the subscriber
    pub async fn unsubscribe_all(&self, subscriber_id: &str) -> Result<()> {
        let mut subs = self.subscriptions.write().await;
        let mut empty_uris = Vec::new();

        for (uri, subscribers) in subs.iter_mut() {
            subscribers.remove(subscriber_id);
            if subscribers.is_empty() {
                empty_uris.push(uri.clone());
            }
        }

        // Remove empty subscription entries
        for uri in empty_uris {
            subs.remove(&uri);
        }
        drop(subs);

        Ok(())
    }

    /// Check if a resource has any subscribers.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI to check
    pub async fn has_subscribers(&self, uri: &str) -> bool {
        let subs = self.subscriptions.read().await;
        subs.get(uri).is_some_and(|s| !s.is_empty())
    }

    /// Get all subscribed resources for a subscriber.
    ///
    /// # Arguments
    ///
    /// * `subscriber_id` - Unique identifier for the subscriber
    pub async fn get_subscriptions(&self, subscriber_id: &str) -> Vec<String> {
        let subs = self.subscriptions.read().await;
        subs.iter()
            .filter_map(|(uri, subscribers)| {
                if subscribers.contains(subscriber_id) {
                    Some(uri.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all subscribers for a resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI
    pub async fn get_subscribers(&self, uri: &str) -> Vec<String> {
        let subs = self.subscriptions.read().await;
        subs.get(uri)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Notify subscribers that a resource has been updated.
    ///
    /// # Arguments
    ///
    /// * `uri` - The resource URI that was updated
    ///
    /// # Returns
    ///
    /// The number of subscribers notified
    pub async fn notify_resource_updated(&self, uri: String) -> Result<usize> {
        let subs = self.subscriptions.read().await;

        if let Some(subscribers) = subs.get(&uri) {
            let subscriber_count = subscribers.len();
            drop(subs);
            if subscriber_count > 0 {
                // Send notification if sender is available
                if let Some(sender) = &self.notification_sender {
                    let notification = ServerNotification::ResourceUpdated(ResourceUpdatedParams {
                        uri: uri.clone(),
                    });
                    sender(notification);
                }
                // Return count regardless of whether notification was sent
                return Ok(subscriber_count);
            }
        }

        Ok(0)
    }

    /// Get statistics about current subscriptions.
    pub async fn get_stats(&self) -> SubscriptionStats {
        let subs = self.subscriptions.read().await;
        let total_resources = subs.len();
        let total_subscriptions = subs.values().map(std::collections::HashSet::len).sum();

        let mut subscriber_counts = HashMap::new();
        for subscribers in subs.values() {
            for subscriber in subscribers {
                *subscriber_counts.entry(subscriber.clone()).or_insert(0) += 1;
            }
        }
        drop(subs);

        SubscriptionStats {
            total_resources,
            total_subscriptions,
            unique_subscribers: subscriber_counts.len(),
            subscriptions_per_resource: if total_resources > 0 {
                #[allow(clippy::cast_precision_loss)]
                {
                    total_subscriptions as f64 / total_resources as f64
                }
            } else {
                0.0
            },
        }
    }
}

/// Statistics about current subscriptions.
#[derive(Debug, Clone)]
pub struct SubscriptionStats {
    /// Total number of unique resources being subscribed to
    pub total_resources: usize,
    /// Total number of subscriptions across all resources
    pub total_subscriptions: usize,
    /// Number of unique subscribers
    pub unique_subscribers: usize,
    /// Average number of subscriptions per resource
    pub subscriptions_per_resource: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let manager = SubscriptionManager::new();

        // Subscribe
        manager
            .subscribe("file://test.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        assert!(manager.has_subscribers("file://test.txt").await);

        let subs = manager.get_subscriptions("client1").await;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0], "file://test.txt");

        // Unsubscribe
        manager
            .unsubscribe("file://test.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        assert!(!manager.has_subscribers("file://test.txt").await);

        let subs = manager.get_subscriptions("client1").await;
        assert_eq!(subs.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let manager = SubscriptionManager::new();

        // Multiple clients subscribe to same resource
        manager
            .subscribe("file://shared.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://shared.txt".to_string(), "client2".to_string())
            .await
            .unwrap();

        let subscribers = manager.get_subscribers("file://shared.txt").await;
        assert_eq!(subscribers.len(), 2);
        assert!(subscribers.contains(&"client1".to_string()));
        assert!(subscribers.contains(&"client2".to_string()));

        // One client unsubscribes
        manager
            .unsubscribe("file://shared.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        assert!(manager.has_subscribers("file://shared.txt").await);

        let subscribers = manager.get_subscribers("file://shared.txt").await;
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0], "client2");
    }

    #[tokio::test]
    async fn test_unsubscribe_all() {
        let manager = SubscriptionManager::new();

        // Client subscribes to multiple resources
        manager
            .subscribe("file://test1.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test2.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test3.txt".to_string(), "client1".to_string())
            .await
            .unwrap();

        // Another client subscribes to one of them
        manager
            .subscribe("file://test2.txt".to_string(), "client2".to_string())
            .await
            .unwrap();

        // Unsubscribe all for client1
        manager.unsubscribe_all("client1").await.unwrap();

        let subs = manager.get_subscriptions("client1").await;
        assert_eq!(subs.len(), 0);

        // Client2 should still be subscribed
        assert!(manager.has_subscribers("file://test2.txt").await);
        assert!(!manager.has_subscribers("file://test1.txt").await);
        assert!(!manager.has_subscribers("file://test3.txt").await);
    }

    #[tokio::test]
    async fn test_stats() {
        let manager = SubscriptionManager::new();

        manager
            .subscribe("file://test1.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test1.txt".to_string(), "client2".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test2.txt".to_string(), "client1".to_string())
            .await
            .unwrap();
        manager
            .subscribe("file://test3.txt".to_string(), "client3".to_string())
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_resources, 3);
        assert_eq!(stats.total_subscriptions, 4);
        assert_eq!(stats.unique_subscribers, 3);
        assert!((stats.subscriptions_per_resource - 1.33).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_notify_resource_updated() {
        use std::sync::Mutex;

        let manager = SubscriptionManager::new();
        let notifications = Arc::new(Mutex::new(Vec::new()));

        // Set up notification sender
        let notifications_clone = notifications.clone();
        let mut manager_mut = manager.clone();
        manager_mut.set_notification_sender(move |notif| {
            notifications_clone.lock().unwrap().push(notif);
        });

        // Subscribe to resource
        manager_mut
            .subscribe("file://test.txt".to_string(), "client1".to_string())
            .await
            .unwrap();

        // Notify update
        let count = manager_mut
            .notify_resource_updated("file://test.txt".to_string())
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Check notification was sent
        let notifs = notifications.lock().unwrap();
        assert_eq!(notifs.len(), 1);
        match &notifs[0] {
            ServerNotification::ResourceUpdated(n) => assert_eq!(n.uri, "file://test.txt"),
            _ => panic!("Wrong notification type"),
        }
    }
}
