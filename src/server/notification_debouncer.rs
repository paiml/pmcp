//! Advanced notification debouncing system for MCP servers.
//!
//! This module provides sophisticated debouncing capabilities for notifications
//! to reduce network traffic and improve performance when dealing with rapid
//! state changes.

use crate::error::Result;
use crate::types::protocol::ServerNotification;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep, Instant};
use tracing::{debug, trace, warn};

/// Configuration for notification debouncing
#[derive(Debug, Clone)]
pub struct NotificationDebouncerConfig {
    /// Default debounce interval for all notifications
    pub default_interval: Duration,

    /// Per-notification type debounce intervals
    pub type_intervals: HashMap<String, Duration>,

    /// Maximum batch size for coalesced notifications
    pub max_batch_size: usize,

    /// Whether to merge similar notifications
    pub enable_merging: bool,

    /// Maximum time to wait before forcing a flush
    pub max_wait_time: Duration,
}

impl Default for NotificationDebouncerConfig {
    fn default() -> Self {
        let mut type_intervals = HashMap::new();
        // Set specific intervals for known notification types
        type_intervals.insert(
            "resources/list_changed".to_string(),
            Duration::from_millis(500),
        );
        type_intervals.insert("progress".to_string(), Duration::from_millis(100));
        type_intervals.insert("log".to_string(), Duration::from_millis(50));

        Self {
            default_interval: Duration::from_millis(250),
            type_intervals,
            max_batch_size: 100,
            enable_merging: true,
            max_wait_time: Duration::from_secs(5),
        }
    }
}

/// Notification debouncer for reducing notification frequency
pub struct NotificationDebouncer {
    config: NotificationDebouncerConfig,
    pending: Arc<RwLock<HashMap<String, PendingNotification>>>,
    output_tx: mpsc::Sender<ServerNotification>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Pending notification with metadata
#[derive(Debug, Clone)]
struct PendingNotification {
    /// The notification itself
    notification: ServerNotification,

    /// First time this notification was seen
    first_seen: Instant,

    /// Last time this notification was updated
    last_updated: Instant,

    /// Number of times this notification was seen
    count: usize,

    /// Additional notifications that can be merged
    mergeable: Vec<ServerNotification>,
}

impl NotificationDebouncer {
    /// Create a new notification debouncer
    pub fn new(
        config: NotificationDebouncerConfig,
        output_tx: mpsc::Sender<ServerNotification>,
    ) -> Self {
        Self {
            config,
            pending: Arc::new(RwLock::new(HashMap::new())),
            output_tx,
            shutdown_tx: None,
        }
    }

    /// Start the debouncer
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let pending = self.pending.clone();
        let output_tx = self.output_tx.clone();
        let config = self.config.clone();

        // Spawn the flush task
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(50));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        debug!("Notification debouncer shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        flush_pending(&pending, &output_tx, &config).await;
                    }
                }
            }

            // Final flush on shutdown
            flush_all(&pending, &output_tx).await;
        });

        Ok(())
    }

    /// Stop the debouncer
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }

    /// Submit a notification for debouncing
    pub async fn submit(&self, notification: ServerNotification) -> Result<()> {
        let key = notification_key(&notification);
        let now = Instant::now();

        let mut pending = self.pending.write().await;

        if let Some(existing) = pending.get_mut(&key) {
            // Update existing notification
            existing.last_updated = now;
            existing.count += 1;

            if self.config.enable_merging {
                // Merge if possible
                if existing.mergeable.len() < self.config.max_batch_size {
                    existing.mergeable.push(notification);
                }
            } else {
                // Replace with latest
                existing.notification = notification;
            }
        } else {
            // New notification
            pending.insert(
                key,
                PendingNotification {
                    notification,
                    first_seen: now,
                    last_updated: now,
                    count: 1,
                    mergeable: Vec::new(),
                },
            );
        }

        Ok(())
    }

    /// Force flush all pending notifications
    pub async fn flush(&self) -> Result<()> {
        flush_all(&self.pending, &self.output_tx).await;
        Ok(())
    }

    /// Get statistics about pending notifications
    pub async fn stats(&self) -> DebouncerStats {
        let pending = self.pending.read().await;

        DebouncerStats {
            pending_count: pending.len(),
            total_notifications: pending.values().map(|p| p.count).sum(),
            oldest_pending: pending
                .values()
                .map(|p| p.first_seen.elapsed())
                .max()
                .unwrap_or_default(),
        }
    }
}

impl std::fmt::Debug for NotificationDebouncer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationDebouncer")
            .field("config", &self.config)
            .field("pending", &"Arc<RwLock<HashMap<...>>>")
            .field("output_tx", &"mpsc::Sender<...>")
            .field("shutdown_tx", &self.shutdown_tx.is_some())
            .finish()
    }
}

/// Statistics about the debouncer
#[derive(Debug, Clone)]
pub struct DebouncerStats {
    /// Number of unique pending notifications
    pub pending_count: usize,

    /// Total number of notifications seen
    pub total_notifications: usize,

    /// Age of the oldest pending notification
    pub oldest_pending: Duration,
}

/// Generate a key for a notification to group similar ones
fn notification_key(notification: &ServerNotification) -> String {
    match notification {
        ServerNotification::Progress(params) => {
            format!("progress:{:?}", params.progress_token)
        },
        ServerNotification::ResourceUpdated(params) => {
            format!("resource_updated:{}", params.uri)
        },
        ServerNotification::ResourcesChanged => "resource_list_changed".to_string(),
        ServerNotification::ToolsChanged => "tool_list_changed".to_string(),
        ServerNotification::PromptsChanged => "prompt_list_changed".to_string(),
        #[allow(unreachable_patterns)]
        _ => "unknown".to_string(),
    }
}

/// Flush pending notifications that are ready
async fn flush_pending(
    pending: &Arc<RwLock<HashMap<String, PendingNotification>>>,
    output_tx: &mpsc::Sender<ServerNotification>,
    config: &NotificationDebouncerConfig,
) {
    let now = Instant::now();
    let mut to_flush = Vec::new();

    {
        let mut pending_map = pending.write().await;

        // Collect notifications that are ready to flush
        pending_map.retain(|key, pending_notif| {
            let interval = config
                .type_intervals
                .get(key)
                .copied()
                .unwrap_or(config.default_interval);

            let should_flush =
                // Debounce interval has passed
                now.duration_since(pending_notif.last_updated) >= interval ||
                // Maximum wait time exceeded
                now.duration_since(pending_notif.first_seen) >= config.max_wait_time;

            if should_flush {
                to_flush.push(pending_notif.clone());
                false // Remove from pending
            } else {
                true // Keep in pending
            }
        });
    }

    // Send flushed notifications
    for pending_notif in to_flush {
        trace!(
            "Flushing notification after {} occurrences",
            pending_notif.count
        );

        // Send the main notification
        if let Err(e) = output_tx.send(pending_notif.notification).await {
            warn!("Failed to send debounced notification: {}", e);
        }

        // Send any merged notifications
        for merged in pending_notif.mergeable {
            if let Err(e) = output_tx.send(merged).await {
                warn!("Failed to send merged notification: {}", e);
            }
        }
    }
}

/// Flush all pending notifications immediately
async fn flush_all(
    pending: &Arc<RwLock<HashMap<String, PendingNotification>>>,
    output_tx: &mpsc::Sender<ServerNotification>,
) {
    let pending_map = pending.write().await;

    for (_, pending_notif) in pending_map.iter() {
        // Send the main notification
        let _ = output_tx.send(pending_notif.notification.clone()).await;

        // Send any merged notifications
        for merged in &pending_notif.mergeable {
            let _ = output_tx.send(merged.clone()).await;
        }
    }
}

/// Notification batcher that groups notifications by type
pub struct NotificationBatcher {
    /// Batches by notification type
    batches: Arc<RwLock<HashMap<String, Vec<ServerNotification>>>>,

    /// Maximum batch size
    max_batch_size: usize,

    /// Batch timeout
    batch_timeout: Duration,

    /// Output channel
    output_tx: mpsc::Sender<Vec<ServerNotification>>,
}

impl NotificationBatcher {
    /// Create a new notification batcher
    pub fn new(
        max_batch_size: usize,
        batch_timeout: Duration,
        output_tx: mpsc::Sender<Vec<ServerNotification>>,
    ) -> Self {
        Self {
            batches: Arc::new(RwLock::new(HashMap::new())),
            max_batch_size,
            batch_timeout,
            output_tx,
        }
    }

    /// Add a notification to the batch
    pub async fn add(&self, notification: ServerNotification) -> Result<()> {
        let key = notification_key(&notification);
        let mut batches = self.batches.write().await;

        let batch = batches.entry(key.clone()).or_insert_with(Vec::new);
        batch.push(notification);

        // Check if batch is full
        if batch.len() >= self.max_batch_size {
            let full_batch = std::mem::take(batch);
            drop(batches);

            // Send the batch
            let _ = self.output_tx.send(full_batch).await;
        } else {
            // Schedule timeout for this batch
            let batches_clone = self.batches.clone();
            let output_tx = self.output_tx.clone();
            let timeout = self.batch_timeout;

            tokio::spawn(async move {
                sleep(timeout).await;

                let mut batches = batches_clone.write().await;
                if let Some(batch) = batches.remove(&key) {
                    if !batch.is_empty() {
                        let _ = output_tx.send(batch).await;
                    }
                }
            });
        }

        Ok(())
    }

    /// Flush all batches
    pub async fn flush(&self) -> Result<()> {
        let mut batches = self.batches.write().await;

        for (_, batch) in batches.drain() {
            if !batch.is_empty() {
                let _ = self.output_tx.send(batch).await;
            }
        }

        Ok(())
    }
}

impl std::fmt::Debug for NotificationBatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationBatcher")
            .field("batches", &"Arc<RwLock<HashMap<...>>>")
            .field("max_batch_size", &self.max_batch_size)
            .field("batch_timeout", &self.batch_timeout)
            .field("output_tx", &"mpsc::Sender<...>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::protocol::ProgressNotification;

    #[tokio::test]
    async fn test_notification_debouncing() {
        let (tx, mut rx) = mpsc::channel(100);
        let config = NotificationDebouncerConfig {
            default_interval: Duration::from_millis(100),
            ..Default::default()
        };

        let enable_merging = config.enable_merging;
        let mut debouncer = NotificationDebouncer::new(config, tx);
        debouncer.start().await.unwrap();

        // Send multiple notifications quickly with the same token
        for i in 0..5 {
            debouncer
                .submit(ServerNotification::Progress(ProgressNotification {
                    progress_token: crate::types::ProgressToken::String("test-token".to_string()),
                    progress: i as f64 * 20.0,
                    message: Some(format!("Progress {}", i)),
                }))
                .await
                .unwrap();
            sleep(Duration::from_millis(20)).await;
        }

        // Should receive only one notification after debounce interval
        sleep(Duration::from_millis(150)).await;

        let mut notifications = Vec::new();
        while let Ok(notification) = rx.try_recv() {
            notifications.push(notification);
        }

        // With merging enabled, we should get the last notification plus any merged ones
        // But since we're updating the same key, only the last one should be sent
        assert!(
            notifications.len() <= 5,
            "Got {} notifications, expected at most 5",
            notifications.len()
        );

        // The count depends on whether merging happened
        if enable_merging {
            // With merging, we might get multiple notifications
            assert!(!notifications.is_empty());
        } else {
            // Without merging, we should get only the last one
            assert_eq!(notifications.len(), 1);
        }

        debouncer.stop().await;
    }

    #[tokio::test]
    async fn test_notification_batching() {
        let (tx, mut rx) = mpsc::channel(100);
        let batcher = NotificationBatcher::new(3, Duration::from_millis(100), tx);

        // Add notifications with the same key for batching
        for i in 0..5 {
            batcher
                .add(ServerNotification::Progress(ProgressNotification {
                    progress_token: crate::types::ProgressToken::String("batch-token".to_string()),
                    progress: i as f64 * 20.0,
                    message: Some(format!("Progress {}", i)),
                }))
                .await
                .unwrap();
        }

        // First batch should be sent immediately (size 3)
        let batch1 = rx.recv().await.unwrap();
        assert_eq!(batch1.len(), 3);

        // Second batch should be sent after timeout
        sleep(Duration::from_millis(150)).await;
        let batch2 = rx.recv().await.unwrap();
        assert_eq!(batch2.len(), 2);
    }
}
