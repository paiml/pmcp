//! Message batching and debouncing utilities.

use crate::error::Result;
use crate::types::Notification;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, sleep};
use tracing::{debug, trace};

/// Configuration for message batching.
#[derive(Debug, Clone)]
pub struct BatchingConfig {
    /// Maximum number of messages in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait before sending a batch
    pub max_wait_time: Duration,
    /// Methods to batch (empty means batch all)
    pub batched_methods: Vec<String>,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 10,
            max_wait_time: Duration::from_millis(100),
            batched_methods: vec![],
        }
    }
}

/// Message batcher that groups notifications.
pub struct MessageBatcher {
    config: BatchingConfig,
    pending: Arc<Mutex<Vec<Notification>>>,
    tx: mpsc::Sender<Vec<Notification>>,
    rx: Arc<Mutex<mpsc::Receiver<Vec<Notification>>>>,
}

impl std::fmt::Debug for MessageBatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageBatcher")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl MessageBatcher {
    /// Create a new message batcher.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageBatcher, BatchingConfig};
    /// use std::time::Duration;
    ///
    /// // Default configuration
    /// let batcher = MessageBatcher::new(BatchingConfig::default());
    ///
    /// // Custom configuration for high-throughput scenarios
    /// let config = BatchingConfig {
    ///     max_batch_size: 50,
    ///     max_wait_time: Duration::from_millis(200),
    ///     batched_methods: vec!["logs.add".to_string(), "progress.update".to_string()],
    /// };
    /// let high_throughput_batcher = MessageBatcher::new(config);
    ///
    /// // Configuration for low-latency scenarios
    /// let low_latency_config = BatchingConfig {
    ///     max_batch_size: 5,
    ///     max_wait_time: Duration::from_millis(10),
    ///     batched_methods: vec![],
    /// };
    /// let low_latency_batcher = MessageBatcher::new(low_latency_config);
    /// ```
    pub fn new(config: BatchingConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            config,
            pending: Arc::new(Mutex::new(Vec::new())),
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    /// Add a notification to the batch.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageBatcher, BatchingConfig};
    /// use pmcp::types::{Notification, ClientNotification, ServerNotification};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = BatchingConfig {
    ///     max_batch_size: 3,
    ///     max_wait_time: Duration::from_millis(100),
    ///     batched_methods: vec![],
    /// };
    /// let batcher = MessageBatcher::new(config);
    ///
    /// // Add various notifications
    /// batcher.add(Notification::Client(ClientNotification::Initialized)).await?;
    /// batcher.add(Notification::Client(ClientNotification::RootsListChanged)).await?;
    ///
    /// // This will trigger immediate batch send (max_batch_size reached)
    /// batcher.add(Notification::Server(ServerNotification::ToolsChanged)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add(&self, notification: Notification) -> Result<()> {
        let mut pending = self.pending.lock().await;
        pending.push(notification);

        if pending.len() >= self.config.max_batch_size {
            let batch = std::mem::take(&mut *pending);
            drop(pending);
            self.tx
                .send(batch)
                .await
                .map_err(|_| crate::error::Error::Internal("Failed to send batch".to_string()))?;
        } else {
            drop(pending);
        }

        Ok(())
    }

    /// Start the batching timer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageBatcher, BatchingConfig};
    /// use pmcp::types::{Notification, ClientNotification};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = BatchingConfig {
    ///     max_batch_size: 10,
    ///     max_wait_time: Duration::from_millis(50),
    ///     batched_methods: vec![],
    /// };
    /// let batcher = MessageBatcher::new(config);
    ///
    /// // Start the timer to automatically flush batches
    /// batcher.start_timer();
    ///
    /// // Add notifications that won't reach max_batch_size
    /// batcher.add(Notification::Client(ClientNotification::Initialized)).await?;
    ///
    /// // Timer will ensure this gets sent after max_wait_time
    /// # Ok(())
    /// # }
    /// ```
    pub fn start_timer(&self) {
        let pending = self.pending.clone();
        let tx = self.tx.clone();
        let max_wait = self.config.max_wait_time;

        tokio::spawn(async move {
            let mut ticker = interval(max_wait);
            loop {
                ticker.tick().await;

                let mut pending_guard = pending.lock().await;
                if !pending_guard.is_empty() {
                    let batch = std::mem::take(&mut *pending_guard);
                    drop(pending_guard);
                    if tx.send(batch).await.is_err() {
                        break;
                    }
                }
            }
        });
    }

    /// Receive the next batch of notifications.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageBatcher, BatchingConfig};
    /// use pmcp::types::{Notification, ClientNotification};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = BatchingConfig {
    ///     max_batch_size: 2,
    ///     max_wait_time: Duration::from_millis(100),
    ///     batched_methods: vec![],
    /// };
    /// let batcher = MessageBatcher::new(config);
    /// batcher.start_timer();
    ///
    /// // Add notifications
    /// batcher.add(Notification::Client(ClientNotification::Initialized)).await?;
    /// batcher.add(Notification::Client(ClientNotification::RootsListChanged)).await?;
    ///
    /// // Receive the batch (triggered by max_batch_size)
    /// if let Some(batch) = batcher.receive_batch().await {
    ///     println!("Received batch with {} notifications", batch.len());
    ///     for notification in batch {
    ///         // Process each notification
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn receive_batch(&self) -> Option<Vec<Notification>> {
        self.rx.lock().await.recv().await
    }
}

/// Configuration for debouncing.
#[derive(Debug, Clone)]
pub struct DebouncingConfig {
    /// Time to wait before sending after last update
    pub wait_time: Duration,
    /// Methods to debounce (method -> wait time)
    pub debounced_methods: HashMap<String, Duration>,
}

impl Default for DebouncingConfig {
    fn default() -> Self {
        Self {
            wait_time: Duration::from_millis(50),
            debounced_methods: HashMap::new(),
        }
    }
}

/// Message debouncer that delays and coalesces rapid notifications.
pub struct MessageDebouncer {
    config: DebouncingConfig,
    pending: Arc<Mutex<HashMap<String, Notification>>>,
    timers: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    tx: mpsc::Sender<Notification>,
    rx: Arc<Mutex<mpsc::Receiver<Notification>>>,
}

impl std::fmt::Debug for MessageDebouncer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageDebouncer")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl MessageDebouncer {
    /// Create a new message debouncer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageDebouncer, DebouncingConfig};
    /// use std::time::Duration;
    /// use std::collections::HashMap;
    ///
    /// // Default configuration
    /// let debouncer = MessageDebouncer::new(DebouncingConfig::default());
    ///
    /// // Custom configuration with per-method settings
    /// let mut config = DebouncingConfig {
    ///     wait_time: Duration::from_millis(100),
    ///     debounced_methods: HashMap::new(),
    /// };
    ///
    /// // Different debounce times for different notification types
    /// config.debounced_methods.insert(
    ///     "progress.update".to_string(),
    ///     Duration::from_millis(50)
    /// );
    /// config.debounced_methods.insert(
    ///     "file.changed".to_string(),
    ///     Duration::from_millis(500)
    /// );
    ///
    /// let custom_debouncer = MessageDebouncer::new(config);
    /// ```
    pub fn new(config: DebouncingConfig) -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            config,
            pending: Arc::new(Mutex::new(HashMap::new())),
            timers: Arc::new(Mutex::new(HashMap::new())),
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    /// Add a notification to be debounced.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageDebouncer, DebouncingConfig};
    /// use pmcp::types::{Notification, ServerNotification};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let debouncer = MessageDebouncer::new(DebouncingConfig {
    ///     wait_time: Duration::from_millis(100),
    ///     debounced_methods: Default::default(),
    /// });
    ///
    /// // Rapid file change notifications
    /// debouncer.add(
    ///     "file:/path/to/file.rs".to_string(),
    ///     Notification::Server(ServerNotification::ResourcesChanged)
    /// ).await?;
    ///
    /// // Another change to same file within debounce window
    /// tokio::time::sleep(Duration::from_millis(50)).await;
    /// debouncer.add(
    ///     "file:/path/to/file.rs".to_string(),
    ///     Notification::Server(ServerNotification::ResourcesChanged)
    /// ).await?;
    ///
    /// // Only the last notification will be sent after debounce period
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add(&self, key: String, notification: Notification) -> Result<()> {
        trace!("Debouncing notification with key: {}", key);

        let wait_time = self
            .config
            .debounced_methods
            .get(&key)
            .copied()
            .unwrap_or(self.config.wait_time);

        // Store the latest notification
        {
            let mut pending = self.pending.lock().await;
            pending.insert(key.clone(), notification);
        }

        // Cancel existing timer
        {
            let mut timers = self.timers.lock().await;
            if let Some(handle) = timers.remove(&key) {
                handle.abort();
            }
        }

        // Start new timer
        let pending = self.pending.clone();
        let tx = self.tx.clone();
        let timers = self.timers.clone();
        let key_clone = key.clone();

        let handle = tokio::spawn(async move {
            sleep(wait_time).await;

            // Send the notification
            let notification = {
                let mut pending = pending.lock().await;
                pending.remove(&key_clone)
            };

            if let Some(notification) = notification {
                debug!("Sending debounced notification: {}", key_clone);
                let _ = tx.send(notification).await;
            }

            // Remove timer handle
            let mut timers = timers.lock().await;
            timers.remove(&key_clone);
        });

        // Store timer handle
        {
            let mut timers = self.timers.lock().await;
            timers.insert(key, handle);
        }

        Ok(())
    }

    /// Receive the next debounced notification.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageDebouncer, DebouncingConfig};
    /// use pmcp::types::{Notification, ServerNotification};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let debouncer = MessageDebouncer::new(DebouncingConfig::default());
    ///
    /// // Spawn a task to receive notifications
    /// tokio::spawn(async move {
    ///     while let Some(notification) = debouncer.receive().await {
    ///         match notification {
    ///             Notification::Server(ServerNotification::ResourcesChanged) => {
    ///                 println!("Resources changed (after debounce)");
    ///             }
    ///             _ => {}
    ///         }
    ///     }
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub async fn receive(&self) -> Option<Notification> {
        self.rx.lock().await.recv().await
    }

    /// Flush all pending notifications immediately.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::utils::{MessageDebouncer, DebouncingConfig};
    /// use pmcp::types::{Notification, ServerNotification, ClientNotification};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let debouncer = MessageDebouncer::new(DebouncingConfig {
    ///     wait_time: Duration::from_secs(5), // Long debounce
    ///     debounced_methods: Default::default(),
    /// });
    ///
    /// // Add several notifications
    /// debouncer.add(
    ///     "resource1".to_string(),
    ///     Notification::Server(ServerNotification::ResourcesChanged)
    /// ).await?;
    /// debouncer.add(
    ///     "resource2".to_string(),
    ///     Notification::Client(ClientNotification::RootsListChanged)
    /// ).await?;
    ///
    /// // Flush immediately instead of waiting for debounce
    /// let pending = debouncer.flush().await;
    /// println!("Flushed {} pending notifications", pending.len());
    ///
    /// // Process all flushed notifications
    /// for notification in pending {
    ///     // Handle each notification immediately
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn flush(&self) -> Vec<Notification> {
        // Cancel all timers
        {
            let mut timers = self.timers.lock().await;
            for (_, handle) in timers.drain() {
                handle.abort();
            }
        }

        // Get all pending notifications
        let mut pending = self.pending.lock().await;
        pending.drain().map(|(_, v)| v).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClientNotification;

    #[tokio::test]
    async fn test_message_batcher() {
        let config = BatchingConfig {
            max_batch_size: 2,
            max_wait_time: Duration::from_millis(10),
            batched_methods: vec![],
        };

        let batcher = MessageBatcher::new(config);
        batcher.start_timer();

        // Add two notifications
        let notif1 = Notification::Client(ClientNotification::Initialized);
        let notif2 = Notification::Client(ClientNotification::RootsListChanged);

        batcher.add(notif1).await.unwrap();
        batcher.add(notif2).await.unwrap();

        // Should receive batch immediately (max size reached)
        let batch = batcher.receive_batch().await;
        assert!(batch.is_some());
        assert_eq!(batch.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_message_debouncer() {
        let config = DebouncingConfig {
            wait_time: Duration::from_millis(10),
            debounced_methods: HashMap::new(),
        };

        let debouncer = MessageDebouncer::new(config);

        // Add notifications with same key
        let notif1 = Notification::Client(ClientNotification::Initialized);
        let notif2 = Notification::Client(ClientNotification::RootsListChanged);

        debouncer.add("test".to_string(), notif1).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        debouncer.add("test".to_string(), notif2).await.unwrap();

        // Should only receive the last one after debounce period
        let received = debouncer.receive().await;
        assert!(received.is_some());

        // Should be the second notification
        match received.unwrap() {
            Notification::Client(ClientNotification::RootsListChanged) => {},
            _ => panic!("Wrong notification received"),
        }
    }
}
