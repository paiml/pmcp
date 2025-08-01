//! Advanced reconnection logic with exponential backoff.
//!
//! This module provides sophisticated reconnection strategies for network transports,
//! including exponential backoff, jitter, and circuit breaker patterns.

use crate::error::{Error, ErrorCode, Result};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Reconnection configuration.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Initial delay before first reconnection attempt.
    pub initial_delay: Duration,

    /// Maximum delay between reconnection attempts.
    pub max_delay: Duration,

    /// Factor by which delay grows after each failure.
    pub growth_factor: f64,

    /// Maximum number of reconnection attempts (None for unlimited).
    pub max_retries: Option<u32>,

    /// Jitter factor (0.0 to 1.0) to randomize delays.
    pub jitter_factor: f64,

    /// Whether to reset delay after successful connection.
    pub reset_on_success: bool,

    /// Minimum time a connection must be alive to be considered successful.
    pub success_threshold: Duration,

    /// Circuit breaker threshold (failures before circuit opens).
    pub circuit_breaker_threshold: Option<u32>,

    /// Circuit breaker reset timeout.
    pub circuit_breaker_timeout: Duration,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            growth_factor: 2.0,
            max_retries: None,
            jitter_factor: 0.1,
            reset_on_success: true,
            success_threshold: Duration::from_secs(60),
            circuit_breaker_threshold: Some(5),
            circuit_breaker_timeout: Duration::from_secs(60),
        }
    }
}

/// Connection state for reconnection logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected and not attempting to connect.
    Disconnected,

    /// Currently attempting to connect.
    Connecting,

    /// Successfully connected.
    Connected,

    /// Connection failed, waiting before retry.
    WaitingRetry,

    /// Circuit breaker is open, not attempting connections.
    CircuitOpen,
}

/// Reconnection manager for handling connection lifecycle.
pub struct ReconnectManager {
    /// Configuration.
    config: ReconnectConfig,

    /// Current state.
    state: Arc<RwLock<ConnectionState>>,

    /// Current retry count.
    retry_count: AtomicU32,

    /// Consecutive failure count.
    failure_count: AtomicU32,

    /// Total connection attempts.
    total_attempts: AtomicU64,

    /// Total successful connections.
    total_successes: AtomicU64,

    /// Last connection attempt time.
    last_attempt: Arc<Mutex<Option<Instant>>>,

    /// Last successful connection time.
    last_success: Arc<Mutex<Option<Instant>>>,

    /// Circuit breaker opened time.
    circuit_opened_at: Arc<Mutex<Option<Instant>>>,

    /// Whether reconnection is enabled.
    enabled: AtomicBool,

    /// Callbacks.
    callbacks: Arc<ReconnectCallbacks>,
}

/// Callback types for reconnection events.
pub type ConnectingCallback = Box<dyn Fn(u32) + Send + Sync>;
pub type ConnectionCallback = Box<dyn Fn() + Send + Sync>;
pub type FailureCallback = Box<dyn Fn(&Error) + Send + Sync>;

/// Callbacks for reconnection events.
#[derive(Default)]
pub struct ReconnectCallbacks {
    /// Called before connection attempt.
    pub on_connecting: Option<ConnectingCallback>,

    /// Called on successful connection.
    pub on_connected: Option<ConnectionCallback>,

    /// Called on connection failure.
    pub on_failed: Option<FailureCallback>,

    /// Called when circuit breaker opens.
    pub on_circuit_open: Option<ConnectionCallback>,

    /// Called when circuit breaker closes.
    pub on_circuit_close: Option<ConnectionCallback>,
}

impl ReconnectManager {
    /// Create a new reconnection manager.
    pub fn new(config: ReconnectConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            retry_count: AtomicU32::new(0),
            failure_count: AtomicU32::new(0),
            total_attempts: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            last_attempt: Arc::new(Mutex::new(None)),
            last_success: Arc::new(Mutex::new(None)),
            circuit_opened_at: Arc::new(Mutex::new(None)),
            enabled: AtomicBool::new(true),
            callbacks: Arc::new(ReconnectCallbacks::default()),
        }
    }

    /// Set callbacks for reconnection events.
    pub fn set_callbacks(&mut self, callbacks: ReconnectCallbacks) {
        self.callbacks = Arc::new(callbacks);
    }

    /// Enable or disable reconnection.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Get current connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Check if reconnection should be attempted.
    pub async fn should_reconnect(&self) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return false;
        }

        let state = *self.state.read().await;
        match state {
            ConnectionState::Connected | ConnectionState::Connecting => false,
            ConnectionState::CircuitOpen => {
                // Check if circuit should be closed
                if let Some(opened_at) = *self.circuit_opened_at.lock().await {
                    if opened_at.elapsed() >= self.config.circuit_breaker_timeout {
                        info!("Circuit breaker timeout reached, closing circuit");
                        *self.circuit_opened_at.lock().await = None;
                        *self.state.write().await = ConnectionState::Disconnected;

                        if let Some(callback) = &self.callbacks.on_circuit_close {
                            callback();
                        }

                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            _ => {
                // Check retry limit
                if let Some(max_retries) = self.config.max_retries {
                    self.retry_count.load(Ordering::Relaxed) < max_retries
                } else {
                    true
                }
            },
        }
    }

    /// Calculate next retry delay with exponential backoff and jitter.
    pub fn calculate_delay(&self) -> Duration {
        let retry_count = self.retry_count.load(Ordering::Relaxed);

        // Calculate base delay with exponential backoff
        let base_delay = self.config.initial_delay.as_secs_f64()
            * self.config.growth_factor.powi(retry_count as i32);

        // Cap at maximum delay
        let capped_delay = base_delay.min(self.config.max_delay.as_secs_f64());

        // Add jitter
        let jitter_range = capped_delay * self.config.jitter_factor;
        // Simple jitter calculation without external dependency
        let jitter = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .subsec_nanos();
            let random_factor = nanos as f64 / 1_000_000_000.0; // 0.0 to 1.0
            random_factor * jitter_range * 2.0 - jitter_range
        };
        let final_delay = (capped_delay + jitter).max(0.0);

        Duration::from_secs_f64(final_delay)
    }

    /// Notify that a connection attempt is starting.
    pub async fn on_connecting(&self) {
        *self.state.write().await = ConnectionState::Connecting;
        *self.last_attempt.lock().await = Some(Instant::now());
        self.total_attempts.fetch_add(1, Ordering::Relaxed);

        let retry_count = self.retry_count.load(Ordering::Relaxed);
        debug!("Connection attempt {} starting", retry_count + 1);

        if let Some(callback) = &self.callbacks.on_connecting {
            callback(retry_count);
        }
    }

    /// Notify that a connection was successful.
    pub async fn on_connected(&self) {
        *self.state.write().await = ConnectionState::Connected;
        *self.last_success.lock().await = Some(Instant::now());

        self.total_successes.fetch_add(1, Ordering::Relaxed);
        self.failure_count.store(0, Ordering::Relaxed);

        if self.config.reset_on_success {
            self.retry_count.store(0, Ordering::Relaxed);
        }

        info!("Connection established successfully");

        if let Some(callback) = &self.callbacks.on_connected {
            callback();
        }
    }

    /// Notify that a connection attempt failed.
    pub async fn on_connection_failed(&self, error: &Error) {
        *self.state.write().await = ConnectionState::WaitingRetry;

        self.retry_count.fetch_add(1, Ordering::Relaxed);
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        warn!("Connection attempt failed: {}", error);

        // Check circuit breaker
        if let Some(threshold) = self.config.circuit_breaker_threshold {
            if failure_count >= threshold {
                *self.state.write().await = ConnectionState::CircuitOpen;
                *self.circuit_opened_at.lock().await = Some(Instant::now());

                warn!("Circuit breaker opened after {} failures", failure_count);

                if let Some(callback) = &self.callbacks.on_circuit_open {
                    callback();
                }
            }
        }

        if let Some(callback) = &self.callbacks.on_failed {
            callback(error);
        }
    }

    /// Notify that the connection was lost.
    pub async fn on_disconnected(&self) {
        let state = *self.state.read().await;
        if state == ConnectionState::Connected {
            // Check if connection was successful long enough
            if let Some(last_success) = *self.last_success.lock().await {
                if last_success.elapsed() >= self.config.success_threshold {
                    // Reset retry count for long-lived connections
                    self.retry_count.store(0, Ordering::Relaxed);
                    debug!("Long-lived connection ended, resetting retry count");
                }
            }
        }

        *self.state.write().await = ConnectionState::Disconnected;
        info!("Connection lost");
    }

    /// Execute reconnection with the provided connect function.
    pub async fn reconnect_with<F, Fut>(&self, connect: F) -> Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        loop {
            if !self.should_reconnect().await {
                return Err(Error::protocol(
                    ErrorCode::INTERNAL_ERROR,
                    "Reconnection disabled or limit reached",
                ));
            }

            self.on_connecting().await;

            match connect().await {
                Ok(()) => {
                    self.on_connected().await;
                    return Ok(());
                },
                Err(e) => {
                    self.on_connection_failed(&e).await;

                    if !self.should_reconnect().await {
                        return Err(e);
                    }

                    let delay = self.calculate_delay();
                    info!("Retrying connection in {:?}", delay);
                    sleep(delay).await;
                },
            }
        }
    }

    /// Get reconnection statistics.
    pub fn stats(&self) -> ReconnectStats {
        ReconnectStats {
            total_attempts: self.total_attempts.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            current_retry_count: self.retry_count.load(Ordering::Relaxed),
            consecutive_failures: self.failure_count.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters.
    pub fn reset(&self) {
        self.retry_count.store(0, Ordering::Relaxed);
        self.failure_count.store(0, Ordering::Relaxed);
    }
}

/// Reconnection statistics.
#[derive(Debug, Clone)]
pub struct ReconnectStats {
    /// Total connection attempts.
    pub total_attempts: u64,

    /// Total successful connections.
    pub total_successes: u64,

    /// Current retry count.
    pub current_retry_count: u32,

    /// Consecutive failure count.
    pub consecutive_failures: u32,
}

/// Reconnection guard that automatically notifies disconnection on drop.
pub struct ReconnectGuard {
    manager: Arc<ReconnectManager>,
}

impl ReconnectGuard {
    /// Create a new reconnection guard.
    pub fn new(manager: Arc<ReconnectManager>) -> Self {
        Self { manager }
    }
}

impl Drop for ReconnectGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        tokio::spawn(async move {
            manager.on_disconnected().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exponential_backoff() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            growth_factor: 2.0,
            jitter_factor: 0.0, // No jitter for predictable testing
            ..Default::default()
        };

        let manager = ReconnectManager::new(config);

        // First delay should be initial delay
        assert_eq!(manager.calculate_delay(), Duration::from_millis(100));

        // Simulate failures
        manager.retry_count.store(1, Ordering::Relaxed);
        assert_eq!(manager.calculate_delay(), Duration::from_millis(200));

        manager.retry_count.store(2, Ordering::Relaxed);
        assert_eq!(manager.calculate_delay(), Duration::from_millis(400));

        // Should cap at max delay
        manager.retry_count.store(10, Ordering::Relaxed);
        assert_eq!(manager.calculate_delay(), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = ReconnectConfig {
            circuit_breaker_threshold: Some(3),
            circuit_breaker_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let manager = ReconnectManager::new(config);

        // Simulate failures
        for _ in 0..3 {
            manager.on_connection_failed(&Error::internal("test")).await;
        }

        // Circuit should be open
        assert_eq!(manager.state().await, ConnectionState::CircuitOpen);
        assert!(!manager.should_reconnect().await);

        // Wait for circuit timeout
        sleep(Duration::from_millis(150)).await;

        // Circuit should close
        assert!(manager.should_reconnect().await);
    }

    #[tokio::test]
    async fn test_retry_limit() {
        let config = ReconnectConfig {
            max_retries: Some(3),
            ..Default::default()
        };

        let manager = ReconnectManager::new(config);

        // Should allow retries up to limit
        for i in 0..3 {
            assert!(manager.should_reconnect().await);
            manager.retry_count.store(i, Ordering::Relaxed);
        }

        // Should not allow beyond limit
        manager.retry_count.store(3, Ordering::Relaxed);
        assert!(!manager.should_reconnect().await);
    }

    #[tokio::test]
    async fn test_success_reset() {
        let config = ReconnectConfig {
            reset_on_success: true,
            ..Default::default()
        };

        let manager = ReconnectManager::new(config);

        // Simulate some failures
        manager.retry_count.store(5, Ordering::Relaxed);
        manager.failure_count.store(3, Ordering::Relaxed);

        // Successful connection should reset counters
        manager.on_connected().await;

        assert_eq!(manager.retry_count.load(Ordering::Relaxed), 0);
        assert_eq!(manager.failure_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_reconnect_with() {
        let manager = Arc::new(ReconnectManager::new(ReconnectConfig {
            initial_delay: Duration::from_millis(10),
            max_retries: Some(3),
            ..Default::default()
        }));

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        // Simulate connection that fails twice then succeeds
        let result = manager
            .reconnect_with(|| {
                let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed);
                async move {
                    if count < 2 {
                        Err(Error::internal("Connection failed"))
                    } else {
                        Ok(())
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
        assert_eq!(manager.state().await, ConnectionState::Connected);
    }
}
