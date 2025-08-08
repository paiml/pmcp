# Session Management Guide

## Overview

Session management in PMCP is crucial for maintaining stateful interactions between MCP clients and servers. This guide covers session persistence strategies, distributed session handling, security best practices, and performance tuning for production environments.

## Table of Contents

1. [Session Architecture](#session-architecture)
2. [Session Persistence Strategies](#session-persistence-strategies)
3. [Distributed Session Handling](#distributed-session-handling)
4. [Security Best Practices](#security-best-practices)
5. [Performance Tuning](#performance-tuning)
6. [Implementation Examples](#implementation-examples)

## Session Architecture

### Core Components

```rust
use pmcp::{Session, SessionId, SessionStore, SessionManager};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSession {
    pub id: SessionId,
    pub user_id: String,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub expires_at: SystemTime,
    pub data: HashMap<String, Value>,
    pub metadata: SessionMetadata,
}

#[derive(Clone, Debug)]
pub struct SessionMetadata {
    pub ip_address: IpAddr,
    pub user_agent: String,
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
}
```

### Session Lifecycle

```
┌─────────┐      ┌──────────┐      ┌────────┐      ┌─────────┐
│ Created │ ───> │ Active   │ ───> │ Idle   │ ───> │ Expired │
└─────────┘      └──────────┘      └────────┘      └─────────┘
                       ↑                 │
                       └─────────────────┘
                         Reactivation
```

## Session Persistence Strategies

### 1. In-Memory Sessions

Fastest but not persistent across restarts.

```rust
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
pub struct InMemorySessionStore {
    sessions: Arc<RwLock<HashMap<SessionId, UserSession>>>,
    config: SessionConfig,
}

impl InMemorySessionStore {
    pub fn new(config: SessionConfig) -> Self {
        let store = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        };
        
        // Start cleanup task
        store.start_cleanup_task();
        store
    }
    
    fn start_cleanup_task(&self) {
        let sessions = self.sessions.clone();
        let interval = self.config.cleanup_interval;
        
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            loop {
                timer.tick().await;
                
                let mut sessions = sessions.write().await;
                let now = SystemTime::now();
                
                sessions.retain(|_, session| {
                    session.expires_at > now
                });
            }
        });
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn get(&self, id: &SessionId) -> Result<Option<UserSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(id).cloned())
    }
    
    async fn set(&self, session: UserSession) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
        Ok(())
    }
    
    async fn delete(&self, id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id);
        Ok(())
    }
    
    async fn extend(&self, id: &SessionId, duration: Duration) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(id) {
            session.expires_at = SystemTime::now() + duration;
            session.last_accessed = SystemTime::now();
        }
        Ok(())
    }
}
```

### 2. Redis-Backed Sessions

Persistent and scalable for distributed systems.

```rust
use redis::{aio::ConnectionManager, AsyncCommands};

#[derive(Clone)]
pub struct RedisSessionStore {
    client: ConnectionManager,
    config: RedisSessionConfig,
    serializer: SessionSerializer,
}

impl RedisSessionStore {
    pub async fn new(redis_url: &str, config: RedisSessionConfig) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection = ConnectionManager::new(client).await?;
        
        Ok(Self {
            client: connection,
            config,
            serializer: SessionSerializer::new(),
        })
    }
    
    fn session_key(&self, id: &SessionId) -> String {
        format!("{}:{}", self.config.key_prefix, id)
    }
}

#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn get(&self, id: &SessionId) -> Result<Option<UserSession>> {
        let key = self.session_key(id);
        let data: Option<Vec<u8>> = self.client.get(&key).await?;
        
        match data {
            Some(bytes) => {
                let session = self.serializer.deserialize(&bytes)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }
    
    async fn set(&self, session: UserSession) -> Result<()> {
        let key = self.session_key(&session.id);
        let data = self.serializer.serialize(&session)?;
        let ttl = session.expires_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::from_secs(0))
            .as_secs() as usize;
        
        self.client.set_ex(&key, data, ttl).await?;
        Ok(())
    }
    
    async fn delete(&self, id: &SessionId) -> Result<()> {
        let key = self.session_key(id);
        self.client.del(&key).await?;
        Ok(())
    }
    
    async fn extend(&self, id: &SessionId, duration: Duration) -> Result<()> {
        let key = self.session_key(id);
        let ttl = duration.as_secs() as usize;
        self.client.expire(&key, ttl).await?;
        Ok(())
    }
}
```

### 3. Database-Backed Sessions

For long-term persistence with complex queries.

```rust
use sqlx::{PgPool, postgres::PgRow};

#[derive(Clone)]
pub struct PostgresSessionStore {
    pool: PgPool,
    config: DbSessionConfig,
}

impl PostgresSessionStore {
    pub async fn new(database_url: &str, config: DbSessionConfig) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        
        // Create sessions table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id VARCHAR(255) PRIMARY KEY,
                user_id VARCHAR(255) NOT NULL,
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                last_accessed TIMESTAMPTZ NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL,
                ip_address INET,
                user_agent TEXT
            );
            
            CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
            "#
        )
        .execute(&pool)
        .await?;
        
        Ok(Self { pool, config })
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn get(&self, id: &SessionId) -> Result<Option<UserSession>> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT * FROM sessions WHERE id = $1 AND expires_at > NOW()"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|r| r.into()))
    }
    
    async fn set(&self, session: UserSession) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, user_id, data, created_at, last_accessed, expires_at,
                ip_address, user_agent
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                data = EXCLUDED.data,
                last_accessed = EXCLUDED.last_accessed,
                expires_at = EXCLUDED.expires_at
            "#
        )
        .bind(&session.id.to_string())
        .bind(&session.user_id)
        .bind(serde_json::to_value(&session.data)?)
        .bind(session.created_at)
        .bind(session.last_accessed)
        .bind(session.expires_at)
        .bind(session.metadata.ip_address)
        .bind(&session.metadata.user_agent)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn delete(&self, id: &SessionId) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    async fn extend(&self, id: &SessionId, duration: Duration) -> Result<()> {
        sqlx::query(
            "UPDATE sessions SET expires_at = NOW() + $1, last_accessed = NOW() WHERE id = $2"
        )
        .bind(PgInterval::try_from(duration)?)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
```

## Distributed Session Handling

### Session Replication

```rust
use pmcp::cluster::{ClusterNode, ReplicationStrategy};

pub struct DistributedSessionManager {
    local_store: Arc<dyn SessionStore>,
    cluster: Arc<ClusterManager>,
    replication: ReplicationStrategy,
}

impl DistributedSessionManager {
    pub async fn new(
        local_store: Arc<dyn SessionStore>,
        cluster_config: ClusterConfig,
    ) -> Result<Self> {
        let cluster = ClusterManager::connect(cluster_config).await?;
        
        Ok(Self {
            local_store,
            cluster: Arc::new(cluster),
            replication: ReplicationStrategy::Quorum,
        })
    }
    
    pub async fn get_session(&self, id: &SessionId) -> Result<Option<UserSession>> {
        // Try local first
        if let Some(session) = self.local_store.get(id).await? {
            return Ok(Some(session));
        }
        
        // Query cluster nodes
        let node = self.cluster.get_node_for_session(id)?;
        if node.is_local() {
            return Ok(None);
        }
        
        // Fetch from remote node
        let session = node.fetch_session(id).await?;
        
        // Cache locally if found
        if let Some(ref s) = session {
            self.local_store.set(s.clone()).await?;
        }
        
        Ok(session)
    }
    
    pub async fn set_session(&self, session: UserSession) -> Result<()> {
        // Store locally
        self.local_store.set(session.clone()).await?;
        
        // Replicate based on strategy
        match self.replication {
            ReplicationStrategy::None => Ok(()),
            ReplicationStrategy::Leader => {
                if self.cluster.is_leader() {
                    self.replicate_to_followers(session).await
                } else {
                    self.forward_to_leader(session).await
                }
            }
            ReplicationStrategy::Quorum => {
                self.replicate_with_quorum(session).await
            }
            ReplicationStrategy::All => {
                self.replicate_to_all(session).await
            }
        }
    }
    
    async fn replicate_with_quorum(&self, session: UserSession) -> Result<()> {
        let nodes = self.cluster.get_replica_nodes();
        let quorum_size = (nodes.len() / 2) + 1;
        
        let futures: Vec<_> = nodes
            .iter()
            .map(|node| node.store_session(session.clone()))
            .collect();
        
        let results = futures::future::join_all(futures).await;
        
        let successes = results.iter().filter(|r| r.is_ok()).count();
        
        if successes >= quorum_size {
            Ok(())
        } else {
            Err(Error::ReplicationFailed)
        }
    }
}
```

### Sticky Sessions with Load Balancing

```rust
pub struct SessionAwareLoadBalancer {
    backends: Vec<Backend>,
    session_affinity: Arc<RwLock<HashMap<SessionId, usize>>>,
}

impl SessionAwareLoadBalancer {
    pub async fn route_request(&self, req: &Request) -> Result<Backend> {
        // Extract session ID from request
        if let Some(session_id) = extract_session_id(req) {
            // Check for existing affinity
            let affinity = self.session_affinity.read().await;
            if let Some(&backend_idx) = affinity.get(&session_id) {
                if self.backends[backend_idx].is_healthy() {
                    return Ok(self.backends[backend_idx].clone());
                }
            }
        }
        
        // Select backend using consistent hashing or round-robin
        let backend_idx = self.select_backend(req).await?;
        let backend = self.backends[backend_idx].clone();
        
        // Store affinity if session exists
        if let Some(session_id) = extract_session_id(req) {
            let mut affinity = self.session_affinity.write().await;
            affinity.insert(session_id, backend_idx);
        }
        
        Ok(backend)
    }
}
```

## Security Best Practices

### 1. Session Token Generation

```rust
use ring::rand::{SecureRandom, SystemRandom};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

pub struct SecureSessionIdGenerator {
    rng: SystemRandom,
    id_length: usize,
}

impl SecureSessionIdGenerator {
    pub fn new(id_length: usize) -> Self {
        Self {
            rng: SystemRandom::new(),
            id_length,
        }
    }
    
    pub fn generate(&self) -> SessionId {
        let mut bytes = vec![0u8; self.id_length];
        self.rng.fill(&mut bytes).expect("Failed to generate random bytes");
        
        SessionId(URL_SAFE_NO_PAD.encode(&bytes))
    }
}
```

### 2. Session Fixation Prevention

```rust
#[async_trait]
impl SessionManager for SecureSessionManager {
    async fn create_session(&self, user_id: &str) -> Result<UserSession> {
        // Always generate new session ID
        let session_id = self.id_generator.generate();
        
        // Invalidate any existing sessions for this user (optional)
        if self.config.single_session_per_user {
            self.invalidate_user_sessions(user_id).await?;
        }
        
        let session = UserSession {
            id: session_id,
            user_id: user_id.to_string(),
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            expires_at: SystemTime::now() + self.config.session_duration,
            data: HashMap::new(),
            metadata: self.collect_metadata(),
        };
        
        self.store.set(session.clone()).await?;
        
        Ok(session)
    }
    
    async fn regenerate_session_id(&self, old_id: &SessionId) -> Result<UserSession> {
        // Get existing session
        let mut session = self.store.get(old_id).await?
            .ok_or(Error::SessionNotFound)?;
        
        // Generate new ID
        session.id = self.id_generator.generate();
        
        // Store with new ID
        self.store.set(session.clone()).await?;
        
        // Delete old session
        self.store.delete(old_id).await?;
        
        Ok(session)
    }
}
```

### 3. Session Encryption

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

pub struct EncryptedSessionStore<S: SessionStore> {
    inner: S,
    cipher: Aes256Gcm,
}

impl<S: SessionStore> EncryptedSessionStore<S> {
    pub fn new(inner: S, encryption_key: &[u8; 32]) -> Self {
        let key = Key::from_slice(encryption_key);
        let cipher = Aes256Gcm::new(key);
        
        Self { inner, cipher }
    }
    
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let nonce = generate_nonce();
        let ciphertext = self.cipher
            .encrypt(&nonce, data)
            .map_err(|e| Error::EncryptionFailed)?;
        
        // Prepend nonce to ciphertext
        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }
    
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(Error::InvalidSessionData);
        }
        
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::DecryptionFailed)
    }
}

#[async_trait]
impl<S: SessionStore> SessionStore for EncryptedSessionStore<S> {
    async fn get(&self, id: &SessionId) -> Result<Option<UserSession>> {
        match self.inner.get(id).await? {
            Some(encrypted_session) => {
                let decrypted = self.decrypt(&encrypted_session.to_bytes())?;
                let session = UserSession::from_bytes(&decrypted)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }
    
    async fn set(&self, session: UserSession) -> Result<()> {
        let encrypted = self.encrypt(&session.to_bytes())?;
        let encrypted_session = UserSession::from_encrypted(encrypted);
        self.inner.set(encrypted_session).await
    }
}
```

### 4. CSRF Protection

```rust
pub struct CsrfProtectedSessionManager {
    session_manager: Arc<dyn SessionManager>,
    token_generator: CsrfTokenGenerator,
}

impl CsrfProtectedSessionManager {
    pub async fn verify_csrf_token(
        &self,
        session_id: &SessionId,
        provided_token: &str,
    ) -> Result<bool> {
        let session = self.session_manager.get_session(session_id).await?
            .ok_or(Error::SessionNotFound)?;
        
        let stored_token = session.data.get("csrf_token")
            .and_then(|v| v.as_str())
            .ok_or(Error::CsrfTokenMissing)?;
        
        // Constant-time comparison
        Ok(constant_time_eq(provided_token.as_bytes(), stored_token.as_bytes()))
    }
    
    pub async fn generate_csrf_token(&self, session_id: &SessionId) -> Result<String> {
        let token = self.token_generator.generate();
        
        let mut session = self.session_manager.get_session(session_id).await?
            .ok_or(Error::SessionNotFound)?;
        
        session.data.insert("csrf_token".to_string(), Value::String(token.clone()));
        self.session_manager.update_session(session).await?;
        
        Ok(token)
    }
}
```

## Performance Tuning

### 1. Session Caching Strategy

```rust
pub struct TieredSessionCache {
    l1_cache: Arc<LruCache<SessionId, UserSession>>, // In-process cache
    l2_cache: Arc<RedisSessionStore>,                // Redis cache
    l3_store: Arc<PostgresSessionStore>,             // Database
    config: CacheConfig,
}

impl TieredSessionCache {
    pub async fn get(&self, id: &SessionId) -> Result<Option<UserSession>> {
        // Check L1 cache
        if let Some(session) = self.l1_cache.get(id) {
            self.record_hit("l1");
            return Ok(Some(session.clone()));
        }
        
        // Check L2 cache
        if let Some(session) = self.l2_cache.get(id).await? {
            self.record_hit("l2");
            self.l1_cache.put(id.clone(), session.clone());
            return Ok(Some(session));
        }
        
        // Check L3 store
        if let Some(session) = self.l3_store.get(id).await? {
            self.record_hit("l3");
            // Populate caches
            self.l2_cache.set(session.clone()).await?;
            self.l1_cache.put(id.clone(), session.clone());
            return Ok(Some(session));
        }
        
        self.record_miss();
        Ok(None)
    }
}
```

### 2. Batch Operations

```rust
pub struct BatchSessionStore<S: SessionStore> {
    inner: S,
    batch_size: usize,
    flush_interval: Duration,
    buffer: Arc<RwLock<Vec<SessionOperation>>>,
}

impl<S: SessionStore> BatchSessionStore<S> {
    pub fn new(inner: S, batch_size: usize, flush_interval: Duration) -> Self {
        let store = Self {
            inner,
            batch_size,
            flush_interval,
            buffer: Arc::new(RwLock::new(Vec::new())),
        };
        
        store.start_flush_task();
        store
    }
    
    fn start_flush_task(&self) {
        let buffer = self.buffer.clone();
        let inner = self.inner.clone();
        let batch_size = self.batch_size;
        let flush_interval = self.flush_interval;
        
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(flush_interval);
            
            loop {
                timer.tick().await;
                
                let operations = {
                    let mut buffer = buffer.write().await;
                    if buffer.is_empty() {
                        continue;
                    }
                    std::mem::take(&mut *buffer)
                };
                
                // Process in batches
                for chunk in operations.chunks(batch_size) {
                    if let Err(e) = inner.batch_execute(chunk).await {
                        tracing::error!("Batch execution failed: {:?}", e);
                    }
                }
            }
        });
    }
}
```

### 3. Connection Pooling

```rust
pub struct PooledSessionStore {
    pool: bb8::Pool<SessionStoreManager>,
    config: PoolConfig,
}

impl PooledSessionStore {
    pub async fn new(config: PoolConfig) -> Result<Self> {
        let manager = SessionStoreManager::new(config.connection_string.clone());
        
        let pool = bb8::Pool::builder()
            .max_size(config.max_connections)
            .min_idle(Some(config.min_idle))
            .connection_timeout(config.connection_timeout)
            .idle_timeout(Some(config.idle_timeout))
            .build(manager)
            .await?;
        
        Ok(Self { pool, config })
    }
    
    pub async fn with_connection<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut SessionStoreConnection) -> Future<Output = Result<R>>,
    {
        let mut conn = self.pool.get().await?;
        f(&mut conn).await
    }
}
```

## Implementation Examples

### Complete Session Manager

```rust
use pmcp::{Server, ServerBuilder, SessionConfig};

pub async fn create_session_enabled_server() -> Result<Server> {
    // Configure session store
    let session_store = match std::env::var("SESSION_STORE") {
        Ok(s) if s == "redis" => {
            let redis_url = std::env::var("REDIS_URL")?;
            Box::new(RedisSessionStore::new(&redis_url, Default::default()).await?)
                as Box<dyn SessionStore>
        }
        Ok(s) if s == "postgres" => {
            let db_url = std::env::var("DATABASE_URL")?;
            Box::new(PostgresSessionStore::new(&db_url, Default::default()).await?)
                as Box<dyn SessionStore>
        }
        _ => Box::new(InMemorySessionStore::new(Default::default()))
            as Box<dyn SessionStore>
    };
    
    // Wrap with encryption if configured
    let session_store = if let Ok(key) = std::env::var("SESSION_ENCRYPTION_KEY") {
        let key_bytes = base64::decode(key)?;
        Box::new(EncryptedSessionStore::new(session_store, &key_bytes))
            as Box<dyn SessionStore>
    } else {
        session_store
    };
    
    // Create session manager
    let session_manager = SessionManager::new(
        session_store,
        SessionConfig {
            session_duration: Duration::from_hours(24),
            idle_timeout: Duration::from_hours(2),
            max_sessions_per_user: 5,
            secure_cookies: true,
            same_site: SameSite::Strict,
            cleanup_interval: Duration::from_minutes(15),
        },
    );
    
    // Build server with session support
    ServerBuilder::new("session-server", "1.0.0")
        .session_manager(session_manager)
        .middleware(SessionMiddleware::new())
        .middleware(CsrfProtectionMiddleware::new())
        .build()
}
```

### Testing Sessions

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let store = InMemorySessionStore::new(Default::default());
        let manager = SessionManager::new(store, Default::default());
        
        // Create session
        let session = manager.create_session("user123").await.unwrap();
        assert_eq!(session.user_id, "user123");
        
        // Retrieve session
        let retrieved = manager.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, session.id);
        
        // Update session data
        manager.set_session_data(
            &session.id,
            "key",
            Value::String("value".to_string())
        ).await.unwrap();
        
        // Extend session
        manager.extend_session(&session.id, Duration::from_hours(1)).await.unwrap();
        
        // Delete session
        manager.delete_session(&session.id).await.unwrap();
        let deleted = manager.get_session(&session.id).await.unwrap();
        assert!(deleted.is_none());
    }
    
    #[tokio::test]
    async fn test_session_expiry() {
        let config = SessionConfig {
            session_duration: Duration::from_millis(100),
            ..Default::default()
        };
        
        let store = InMemorySessionStore::new(config.clone());
        let manager = SessionManager::new(store, config);
        
        let session = manager.create_session("user123").await.unwrap();
        
        // Session should exist initially
        assert!(manager.get_session(&session.id).await.unwrap().is_some());
        
        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Session should be expired
        assert!(manager.get_session(&session.id).await.unwrap().is_none());
    }
}
```

## Monitoring and Observability

```rust
use prometheus::{IntCounter, Histogram, register_int_counter, register_histogram};

pub struct SessionMetrics {
    sessions_created: IntCounter,
    sessions_destroyed: IntCounter,
    session_cache_hits: IntCounter,
    session_cache_misses: IntCounter,
    session_duration: Histogram,
    session_size: Histogram,
}

impl SessionMetrics {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sessions_created: register_int_counter!(
                "sessions_created_total",
                "Total number of sessions created"
            )?,
            sessions_destroyed: register_int_counter!(
                "sessions_destroyed_total",
                "Total number of sessions destroyed"
            )?,
            session_cache_hits: register_int_counter!(
                "session_cache_hits_total",
                "Number of session cache hits"
            )?,
            session_cache_misses: register_int_counter!(
                "session_cache_misses_total",
                "Number of session cache misses"
            )?,
            session_duration: register_histogram!(
                "session_duration_seconds",
                "Session duration in seconds"
            )?,
            session_size: register_histogram!(
                "session_size_bytes",
                "Session data size in bytes"
            )?,
        })
    }
}
```

## Best Practices Summary

1. **Use appropriate storage backend** based on requirements
2. **Implement session encryption** for sensitive data
3. **Set reasonable expiration times** to balance security and UX
4. **Use secure session ID generation** with cryptographic randomness
5. **Implement CSRF protection** for state-changing operations
6. **Monitor session metrics** for performance and security
7. **Test session handling** under load and failure conditions
8. **Document session configuration** for operations teams
9. **Implement graceful session migration** for updates
10. **Provide session debugging tools** for troubleshooting