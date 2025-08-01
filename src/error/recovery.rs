//! Enhanced error handling and recovery mechanisms.
//!
//! This module provides advanced error recovery strategies including retry policies,
//! fallback handlers, and circuit breakers for resilient error handling.

use crate::error::{Error, ErrorCode, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Error recovery strategy.
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry with fixed delay.
    RetryFixed { attempts: u32, delay: Duration },

    /// Retry with exponential backoff.
    RetryExponential {
        attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
    },

    /// Fallback to alternative handler.
    Fallback,

    /// Circuit breaker pattern.
    CircuitBreaker {
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
    },

    /// No recovery, fail immediately.
    FailFast,
}

/// Error recovery policy.
#[derive(Debug, Clone)]
pub struct RecoveryPolicy {
    /// Recovery strategy for each error code.
    strategies: HashMap<ErrorCode, RecoveryStrategy>,

    /// Default strategy if no specific one is defined.
    default_strategy: RecoveryStrategy,

    /// Whether to log recovery attempts.
    log_attempts: bool,
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        let mut strategies = HashMap::new();

        // Network errors get exponential backoff
        strategies.insert(
            ErrorCode::INTERNAL_ERROR,
            RecoveryStrategy::RetryExponential {
                attempts: 3,
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(5),
                multiplier: 2.0,
            },
        );

        // Request errors get fixed retry
        strategies.insert(
            ErrorCode::INVALID_REQUEST,
            RecoveryStrategy::RetryFixed {
                attempts: 2,
                delay: Duration::from_millis(500),
            },
        );

        Self {
            strategies,
            default_strategy: RecoveryStrategy::FailFast,
            log_attempts: true,
        }
    }
}

impl RecoveryPolicy {
    /// Create a new recovery policy.
    pub fn new(default_strategy: RecoveryStrategy) -> Self {
        Self {
            strategies: HashMap::new(),
            default_strategy,
            log_attempts: true,
        }
    }

    /// Add a strategy for a specific error code.
    pub fn add_strategy(&mut self, error_code: ErrorCode, strategy: RecoveryStrategy) {
        self.strategies.insert(error_code, strategy);
    }

    /// Get strategy for an error code.
    pub fn get_strategy(&self, error_code: &ErrorCode) -> &RecoveryStrategy {
        self.strategies
            .get(error_code)
            .unwrap_or(&self.default_strategy)
    }
}

/// Error recovery handler trait.
#[async_trait]
pub trait RecoveryHandler: Send + Sync {
    /// Handle error recovery.
    async fn recover(&self, error_msg: &str) -> Result<serde_json::Value>;
}

/// Default recovery handler.
pub struct DefaultRecoveryHandler;

#[async_trait]
impl RecoveryHandler for DefaultRecoveryHandler {
    async fn recover(&self, error_msg: &str) -> Result<serde_json::Value> {
        Err(Error::internal(error_msg))
    }
}

/// Fallback recovery handler.
pub struct FallbackHandler<F> {
    fallback: F,
}

impl<F> FallbackHandler<F> {
    /// Create a new fallback handler.
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

#[async_trait]
impl<F, Fut> RecoveryHandler for FallbackHandler<F>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = Result<serde_json::Value>> + Send,
{
    async fn recover(&self, _error_msg: &str) -> Result<serde_json::Value> {
        (self.fallback)().await
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for error recovery.
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<std::time::Instant>>>,
    config: CircuitBreakerConfig,
}

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit.
    pub failure_threshold: u32,

    /// Number of successes before closing circuit.
    pub success_threshold: u32,

    /// Timeout before attempting half-open state.
    pub timeout: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Check if the circuit allows requests.
    pub async fn allow_request(&self) -> bool {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.config.timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                        *self.success_count.write().await = 0;
                        info!("Circuit breaker transitioning to half-open");
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a success.
    pub async fn record_success(&self) {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                *self.failure_count.write().await = 0;
            },
            CircuitState::HalfOpen => {
                let mut success_count = self.success_count.write().await;
                *success_count += 1;

                if *success_count >= self.config.success_threshold {
                    *self.state.write().await = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    info!("Circuit breaker closed after successful recovery");
                }
            },
            CircuitState::Open => {
                // Shouldn't happen, but reset anyway
                *self.state.write().await = CircuitState::Closed;
                *self.failure_count.write().await = 0;
            },
        }
    }

    /// Record a failure.
    pub async fn record_failure(&self) {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;

                if *failure_count >= self.config.failure_threshold {
                    *self.state.write().await = CircuitState::Open;
                    *self.last_failure_time.write().await = Some(std::time::Instant::now());
                    warn!("Circuit breaker opened after {} failures", failure_count);
                }
            },
            CircuitState::HalfOpen => {
                *self.state.write().await = CircuitState::Open;
                *self.last_failure_time.write().await = Some(std::time::Instant::now());
                *self.failure_count.write().await = 1;
                warn!("Circuit breaker reopened after failure in half-open state");
            },
            CircuitState::Open => {
                // Already open, update last failure time
                *self.last_failure_time.write().await = Some(std::time::Instant::now());
            },
        }
    }
}

/// Error recovery executor.
pub struct RecoveryExecutor {
    policy: RecoveryPolicy,
    handlers: HashMap<String, Arc<dyn RecoveryHandler>>,
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
}

impl RecoveryExecutor {
    /// Create a new recovery executor.
    pub fn new(policy: RecoveryPolicy) -> Self {
        Self {
            policy,
            handlers: HashMap::new(),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a recovery handler.
    pub fn register_handler(&mut self, name: String, handler: Arc<dyn RecoveryHandler>) {
        self.handlers.insert(name, handler);
    }

    /// Execute with recovery.
    pub async fn execute_with_recovery<F, Fut>(
        &self,
        operation_id: &str,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        match operation().await {
            Ok(result) => {
                // Record success if using circuit breaker
                if let Some(cb) = self.circuit_breakers.read().await.get(operation_id) {
                    cb.record_success().await;
                }
                Ok(result)
            },
            Err(error) => {
                let error_code = error.error_code().unwrap_or(ErrorCode::INTERNAL_ERROR);
                let strategy = self.policy.get_strategy(&error_code);

                match strategy {
                    RecoveryStrategy::RetryFixed { attempts, delay } => {
                        self.retry_fixed(error, *attempts, *delay, operation).await
                    },
                    RecoveryStrategy::RetryExponential {
                        attempts,
                        initial_delay,
                        max_delay,
                        multiplier,
                    } => {
                        self.retry_exponential(
                            error,
                            *attempts,
                            *initial_delay,
                            *max_delay,
                            *multiplier,
                            operation,
                        )
                        .await
                    },
                    RecoveryStrategy::Fallback => {
                        self.fallback(&error.to_string(), operation_id).await
                    },
                    RecoveryStrategy::CircuitBreaker {
                        failure_threshold,
                        success_threshold,
                        timeout,
                    } => {
                        self.circuit_breaker(
                            error,
                            operation_id,
                            *failure_threshold,
                            *success_threshold,
                            *timeout,
                            operation,
                        )
                        .await
                    },
                    RecoveryStrategy::FailFast => Err(error),
                }
            },
        }
    }

    async fn retry_fixed<F, Fut>(
        &self,
        error: Error,
        attempts: u32,
        delay: Duration,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        let mut last_error = error;

        for attempt in 1..=attempts {
            if self.policy.log_attempts {
                debug!(
                    "Retry attempt {} of {} after {:?}",
                    attempt, attempts, delay
                );
            }

            tokio::time::sleep(delay).await;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = e;
                    if self.policy.log_attempts {
                        warn!("Retry attempt {} failed: {}", attempt, last_error);
                    }
                },
            }
        }

        Err(last_error)
    }

    async fn retry_exponential<F, Fut>(
        &self,
        error: Error,
        attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        let mut last_error = error;
        let mut current_delay = initial_delay;

        for attempt in 1..=attempts {
            if self.policy.log_attempts {
                debug!(
                    "Exponential retry attempt {} of {} after {:?}",
                    attempt, attempts, current_delay
                );
            }

            tokio::time::sleep(current_delay).await;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = e;
                    if self.policy.log_attempts {
                        warn!(
                            "Exponential retry attempt {} failed: {}",
                            attempt, last_error
                        );
                    }

                    // Calculate next delay
                    let next_delay = Duration::from_secs_f64(
                        (current_delay.as_secs_f64() * multiplier).min(max_delay.as_secs_f64()),
                    );
                    current_delay = next_delay;
                },
            }
        }

        Err(last_error)
    }

    async fn fallback(&self, error_msg: &str, operation_id: &str) -> Result<serde_json::Value> {
        if let Some(handler) = self.handlers.get(operation_id) {
            if self.policy.log_attempts {
                info!("Using fallback handler for operation: {}", operation_id);
            }
            handler.recover(error_msg).await
        } else {
            if self.policy.log_attempts {
                error!(
                    "No fallback handler registered for operation: {}",
                    operation_id
                );
            }
            Err(Error::internal(error_msg))
        }
    }

    async fn circuit_breaker<F, Fut>(
        &self,
        _error: Error,
        operation_id: &str,
        failure_threshold: u32,
        success_threshold: u32,
        timeout: Duration,
        operation: F,
    ) -> Result<serde_json::Value>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<serde_json::Value>>,
    {
        // Get or create circuit breaker
        let cb = {
            let mut breakers = self.circuit_breakers.write().await;
            breakers
                .entry(operation_id.to_string())
                .or_insert_with(|| {
                    Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
                        failure_threshold,
                        success_threshold,
                        timeout,
                    }))
                })
                .clone()
        };

        if !cb.allow_request().await {
            if self.policy.log_attempts {
                warn!("Circuit breaker open for operation: {}", operation_id);
            }
            return Err(Error::protocol(
                ErrorCode::INTERNAL_ERROR,
                "Circuit breaker is open",
            ));
        }

        match operation().await {
            Ok(result) => {
                cb.record_success().await;
                Ok(result)
            },
            Err(e) => {
                cb.record_failure().await;
                Err(e)
            },
        }
    }
}

/// Helper function to create a retry handler.
pub async fn with_retry<F, Fut>(
    attempts: u32,
    delay: Duration,
    operation: F,
) -> Result<serde_json::Value>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<serde_json::Value>>,
{
    let mut last_error = None;

    for attempt in 0..attempts {
        if attempt > 0 {
            tokio::time::sleep(delay).await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
            },
        }
    }

    Err(last_error.unwrap_or_else(|| Error::internal("No attempts made")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_fixed() {
        let policy = RecoveryPolicy::default();
        let executor = RecoveryExecutor::new(policy);

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = executor
            .retry_fixed(
                Error::internal("test"),
                3,
                Duration::from_millis(10),
                || {
                    let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed);
                    async move {
                        if count < 2 {
                            Err(Error::internal("retry"))
                        } else {
                            Ok(serde_json::json!({"success": true}))
                        }
                    }
                },
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
        };

        let cb = CircuitBreaker::new(config);

        // Initially closed
        assert!(cb.allow_request().await);

        // Record failures to open circuit
        cb.record_failure().await;
        cb.record_failure().await;

        // Should be open now
        assert!(!cb.allow_request().await);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be half-open
        assert!(cb.allow_request().await);

        // Success should start closing
        cb.record_success().await;
        cb.record_success().await;

        // Should be closed again
        assert!(cb.allow_request().await);
    }

    #[tokio::test]
    async fn test_with_retry() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = with_retry(3, Duration::from_millis(10), || {
            let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed);
            async move {
                if count < 2 {
                    Err(Error::internal("retry"))
                } else {
                    Ok(serde_json::json!({"success": true}))
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
    }
}
