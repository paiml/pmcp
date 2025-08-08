# Middleware Composition Guide

## Overview

PMCP's middleware system provides a powerful way to intercept, modify, and extend the behavior of MCP protocol operations. This guide covers advanced middleware composition patterns, best practices, and performance considerations.

## Table of Contents

1. [Understanding Middleware](#understanding-middleware)
2. [Custom Middleware Implementation](#custom-middleware-implementation)
3. [Middleware Ordering](#middleware-ordering)
4. [Performance Implications](#performance-implications)
5. [Real-World Examples](#real-world-examples)

## Understanding Middleware

Middleware in PMCP operates as a chain of interceptors that process requests and responses bidirectionally:

```
Client Request → [MW1] → [MW2] → [MW3] → Server Handler
                   ↓       ↓       ↓          ↓
Client Response ← [MW1] ← [MW2] ← [MW3] ← Server Response
```

### Core Concepts

- **Request Phase**: Middleware processes incoming requests before they reach handlers
- **Response Phase**: Middleware processes outgoing responses before they reach clients
- **Short-circuiting**: Middleware can terminate the chain early
- **Context Propagation**: Middleware can pass data through the chain

## Custom Middleware Implementation

### Basic Middleware Structure

```rust
use pmcp::{Middleware, Request, Response, Result, async_trait};
use std::sync::Arc;

#[derive(Clone)]
pub struct CustomMiddleware {
    config: Arc<MiddlewareConfig>,
}

#[async_trait]
impl Middleware for CustomMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        // Pre-processing
        let modified_req = self.transform_request(req)?;
        
        // Call next middleware
        let response = next.process_request(modified_req).await?;
        
        // Post-processing
        self.transform_response(response)
    }
}
```

### Stateful Middleware

```rust
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RateLimitMiddleware {
    limits: Arc<RwLock<HashMap<String, RateLimit>>>,
    config: RateLimitConfig,
}

impl RateLimitMiddleware {
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            config: RateLimitConfig {
                requests_per_second,
                burst_size,
                window: Duration::from_secs(1),
            },
        }
    }
}

#[async_trait]
impl Middleware for RateLimitMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        let client_id = extract_client_id(&req);
        
        // Check rate limit
        let mut limits = self.limits.write().await;
        let limit = limits.entry(client_id.clone())
            .or_insert_with(|| RateLimit::new(self.config.clone()));
        
        if !limit.check_and_update() {
            return Err(Error::RateLimitExceeded);
        }
        
        // Proceed with request
        next.process_request(req).await
    }
}
```

### Error Handling Middleware

```rust
#[derive(Clone)]
pub struct ErrorHandlingMiddleware {
    error_reporter: Arc<dyn ErrorReporter>,
    retry_policy: RetryPolicy,
}

#[async_trait]
impl Middleware for ErrorHandlingMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        let mut attempts = 0;
        let max_attempts = self.retry_policy.max_attempts;
        
        loop {
            attempts += 1;
            
            match next.process_request(req.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) if attempts < max_attempts && e.is_retryable() => {
                    // Log retry attempt
                    tracing::warn!(
                        attempt = attempts,
                        max_attempts = max_attempts,
                        error = ?e,
                        "Retrying request after error"
                    );
                    
                    // Apply backoff
                    let delay = self.retry_policy.calculate_delay(attempts);
                    tokio::time::sleep(delay).await;
                    continue;
                }
                Err(e) => {
                    // Report error
                    self.error_reporter.report(&e).await;
                    return Err(e);
                }
            }
        }
    }
}
```

## Middleware Ordering

The order in which middleware is applied is crucial for correct behavior and optimal performance.

### Recommended Ordering Pattern

```rust
use pmcp::{ServerBuilder, Server};

fn configure_server() -> Server {
    ServerBuilder::new("my-server", "1.0.0")
        // 1. Observability (outermost)
        .middleware(TracingMiddleware::new())
        .middleware(MetricsMiddleware::new())
        
        // 2. Security
        .middleware(AuthenticationMiddleware::new())
        .middleware(AuthorizationMiddleware::new())
        
        // 3. Rate Limiting / Throttling
        .middleware(RateLimitMiddleware::new(100, 1000))
        
        // 4. Caching
        .middleware(CacheMiddleware::new())
        
        // 5. Request/Response transformation
        .middleware(ValidationMiddleware::new())
        .middleware(CompressionMiddleware::new())
        
        // 6. Error handling (innermost)
        .middleware(ErrorHandlingMiddleware::new())
        
        .build()
        .unwrap()
}
```

### Ordering Principles

1. **Observability First**: Tracing and metrics should wrap all operations
2. **Security Early**: Authentication/authorization should happen before expensive operations
3. **Rate Limiting Before Processing**: Prevent resource exhaustion
4. **Caching After Security**: Only cache authorized requests
5. **Error Handling Last**: Catch errors from all other middleware

### Dynamic Middleware Composition

```rust
pub struct ConditionalMiddleware<M: Middleware> {
    inner: M,
    condition: Box<dyn Fn(&Request) -> bool + Send + Sync>,
}

impl<M: Middleware> ConditionalMiddleware<M> {
    pub fn new<F>(middleware: M, condition: F) -> Self
    where
        F: Fn(&Request) -> bool + Send + Sync + 'static,
    {
        Self {
            inner: middleware,
            condition: Box::new(condition),
        }
    }
}

#[async_trait]
impl<M: Middleware + Clone> Middleware for ConditionalMiddleware<M> {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        if (self.condition)(&req) {
            self.inner.process_request(req, next).await
        } else {
            next.process_request(req).await
        }
    }
}

// Usage
let conditional_cache = ConditionalMiddleware::new(
    CacheMiddleware::new(),
    |req| req.method() == "get_resource"
);
```

## Performance Implications

### Benchmarking Middleware

```rust
use std::time::Instant;

#[derive(Clone)]
pub struct BenchmarkMiddleware {
    name: String,
    threshold_ms: u64,
}

#[async_trait]
impl Middleware for BenchmarkMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        let start = Instant::now();
        
        let response = next.process_request(req).await?;
        
        let duration = start.elapsed();
        
        if duration.as_millis() > self.threshold_ms as u128 {
            tracing::warn!(
                middleware = %self.name,
                duration_ms = duration.as_millis(),
                "Middleware exceeded performance threshold"
            );
        }
        
        // Add timing header
        response.with_header("X-Processing-Time", duration.as_millis().to_string())
    }
}
```

### Performance Guidelines

1. **Minimize Allocations**: Reuse buffers and avoid unnecessary clones
2. **Async-Aware**: Use async operations properly, avoid blocking
3. **Lazy Evaluation**: Only compute expensive values when needed
4. **Connection Pooling**: Reuse connections in middleware that makes external calls
5. **Caching Strategy**: Cache at appropriate levels to reduce redundant work

### Zero-Cost Abstractions

```rust
// Compile-time middleware composition using generics
pub struct MiddlewareStack<M1, M2, M3> {
    m1: M1,
    m2: M2,
    m3: M3,
}

impl<M1, M2, M3> MiddlewareStack<M1, M2, M3>
where
    M1: Middleware,
    M2: Middleware,
    M3: Middleware,
{
    pub fn new(m1: M1, m2: M2, m3: M3) -> Self {
        Self { m1, m2, m3 }
    }
}

// This approach avoids dynamic dispatch overhead
```

## Real-World Examples

### Example 1: Authentication with JWT

```rust
use jsonwebtoken::{decode, DecodingKey, Validation};

#[derive(Clone)]
pub struct JwtAuthMiddleware {
    secret: Arc<Vec<u8>>,
    validation: Validation,
}

impl JwtAuthMiddleware {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            secret: Arc::new(secret.to_vec()),
            validation: Validation::default(),
        }
    }
    
    fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(&self.secret),
            &self.validation,
        )?;
        Ok(token_data.claims)
    }
}

#[async_trait]
impl Middleware for JwtAuthMiddleware {
    async fn process_request(
        &self,
        mut req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        // Extract token from Authorization header
        let token = req.headers()
            .get("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| Error::Unauthorized)?;
        
        // Verify token
        let claims = self.verify_token(token)?;
        
        // Add claims to request context
        req.set_context("user_id", claims.sub);
        req.set_context("roles", claims.roles);
        
        next.process_request(req).await
    }
}
```

### Example 2: Request/Response Logging

```rust
use tracing::{info, warn, error};

#[derive(Clone)]
pub struct LoggingMiddleware {
    log_level: LogLevel,
    include_body: bool,
    max_body_size: usize,
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        let request_id = Uuid::new_v4();
        let method = req.method().to_string();
        let start = Instant::now();
        
        // Log request
        info!(
            request_id = %request_id,
            method = %method,
            "Processing request"
        );
        
        if self.include_body {
            let body = self.truncate_body(req.body());
            info!(
                request_id = %request_id,
                body = %body,
                "Request body"
            );
        }
        
        // Process request
        let response = match next.process_request(req).await {
            Ok(resp) => {
                let duration = start.elapsed();
                info!(
                    request_id = %request_id,
                    method = %method,
                    duration_ms = duration.as_millis(),
                    status = "success",
                    "Request completed"
                );
                Ok(resp)
            }
            Err(e) => {
                let duration = start.elapsed();
                error!(
                    request_id = %request_id,
                    method = %method,
                    duration_ms = duration.as_millis(),
                    error = ?e,
                    "Request failed"
                );
                Err(e)
            }
        };
        
        response
    }
}
```

### Example 3: Circuit Breaker

```rust
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Clone)]
pub struct CircuitBreakerMiddleware {
    state: Arc<RwLock<CircuitState>>,
    config: CircuitBreakerConfig,
    failure_count: Arc<AtomicU32>,
    last_failure_time: Arc<AtomicU64>,
}

#[derive(Clone, Copy, PartialEq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreakerMiddleware {
    pub fn new(
        failure_threshold: u32,
        reset_timeout: Duration,
        half_open_requests: u32,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            config: CircuitBreakerConfig {
                failure_threshold,
                reset_timeout,
                half_open_requests,
            },
            failure_count: Arc::new(AtomicU32::new(0)),
            last_failure_time: Arc::new(AtomicU64::new(0)),
        }
    }
    
    async fn should_allow_request(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let last_failure = self.last_failure_time.load(Ordering::Relaxed);
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                if now - last_failure > self.config.reset_timeout.as_secs() {
                    drop(state);
                    let mut state = self.state.write().await;
                    *state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited requests in half-open state
                self.failure_count.load(Ordering::Relaxed) < self.config.half_open_requests
            }
        }
    }
    
    async fn record_success(&self) {
        let mut state = self.state.write().await;
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            self.failure_count.store(0, Ordering::Relaxed);
        }
    }
    
    async fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        if failures >= self.config.failure_threshold {
            let mut state = self.state.write().await;
            *state = CircuitState::Open;
            
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            self.last_failure_time.store(now, Ordering::Relaxed);
        }
    }
}

#[async_trait]
impl Middleware for CircuitBreakerMiddleware {
    async fn process_request(
        &self,
        req: Request,
        next: Box<dyn Middleware>,
    ) -> Result<Response> {
        if !self.should_allow_request().await {
            return Err(Error::ServiceUnavailable("Circuit breaker open"));
        }
        
        match next.process_request(req).await {
            Ok(response) => {
                self.record_success().await;
                Ok(response)
            }
            Err(e) if e.is_transient() => {
                self.record_failure().await;
                Err(e)
            }
            Err(e) => Err(e),
        }
    }
}
```

## Testing Middleware

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::test_helpers::{MockMiddleware, MockRequest};
    
    #[tokio::test]
    async fn test_rate_limit_middleware() {
        let middleware = RateLimitMiddleware::new(10, 20);
        let next = MockMiddleware::new();
        
        // Should allow first 20 requests (burst)
        for i in 0..20 {
            let req = MockRequest::new().with_id(i);
            let result = middleware.process_request(req, Box::new(next.clone())).await;
            assert!(result.is_ok());
        }
        
        // 21st request should be rate limited
        let req = MockRequest::new().with_id(21);
        let result = middleware.process_request(req, Box::new(next)).await;
        assert!(matches!(result, Err(Error::RateLimitExceeded)));
    }
    
    #[tokio::test]
    async fn test_middleware_ordering() {
        let mut builder = ServerBuilder::new("test", "1.0.0");
        
        // Track middleware execution order
        let order = Arc::new(RwLock::new(Vec::new()));
        
        for i in 0..3 {
            let order_clone = order.clone();
            builder = builder.middleware(OrderTrackingMiddleware::new(i, order_clone));
        }
        
        let server = builder.build().unwrap();
        server.handle_request(MockRequest::new()).await.unwrap();
        
        let execution_order = order.read().await;
        assert_eq!(*execution_order, vec![0, 1, 2, 2, 1, 0]); // In and out
    }
}
```

## Best Practices

1. **Keep Middleware Focused**: Each middleware should have a single responsibility
2. **Make Middleware Reusable**: Design for composition and configuration
3. **Handle Errors Gracefully**: Don't swallow errors unless intentional
4. **Document Side Effects**: Be clear about what the middleware modifies
5. **Test in Isolation**: Unit test middleware independently
6. **Monitor Performance**: Add metrics to track middleware overhead
7. **Version Compatibility**: Ensure middleware works across protocol versions

## Common Pitfalls

1. **Blocking Operations**: Using blocking I/O in async middleware
2. **Unbounded Growth**: Not limiting caches or buffers
3. **Order Dependencies**: Middleware that breaks when reordered
4. **Silent Failures**: Swallowing errors without logging
5. **Resource Leaks**: Not cleaning up connections or handles
6. **Race Conditions**: Improper synchronization in shared state

## Conclusion

Middleware composition in PMCP provides a powerful abstraction for cross-cutting concerns. By following the patterns and best practices outlined in this guide, you can build robust, performant, and maintainable middleware chains that enhance your MCP server's capabilities without sacrificing clarity or performance.