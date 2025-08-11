//! Event store and resumability support for MCP connections.
//!
//! This module provides event persistence and connection resumption capabilities,
//! allowing clients to reconnect and resume from where they left off.

use crate::error::{Error, Result};
use crate::shared::TransportMessage;
use crate::types::RequestId;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

/// Event store trait for persisting and retrieving events.
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store an event with its metadata.
    async fn store_event(&self, event: StoredEvent) -> Result<()>;

    /// Retrieve events since a given event ID.
    async fn get_events_since(
        &self,
        event_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>>;

    /// Get the latest event ID.
    async fn get_latest_event_id(&self) -> Result<Option<String>>;

    /// Clear events older than the specified timestamp.
    async fn clear_events_before(&self, timestamp: DateTime<Utc>) -> Result<usize>;

    /// Get resumption token for current state.
    async fn create_resumption_token(&self) -> Result<ResumptionToken>;

    /// Validate and retrieve state from resumption token.
    async fn validate_resumption_token(&self, token: &str) -> Result<Option<ResumptionState>>;
}

/// A stored event with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Unique event ID
    pub id: String,
    /// Timestamp when the event was stored
    pub timestamp: DateTime<Utc>,
    /// The actual message
    pub message: TransportMessage,
    /// Direction of the message (inbound/outbound)
    pub direction: MessageDirection,
    /// Associated session ID
    pub session_id: String,
    /// Sequence number within the session
    pub sequence: u64,
}

/// Direction of a message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageDirection {
    /// Message received from remote
    Inbound,
    /// Message sent to remote
    Outbound,
}

/// Resumption token for reconnection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumptionToken {
    /// Token identifier
    pub token: String,
    /// Session ID this token is for
    pub session_id: String,
    /// Last event ID processed
    pub last_event_id: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// State for resuming a connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumptionState {
    /// Session ID to resume
    pub session_id: String,
    /// Last event ID that was acknowledged
    pub last_event_id: String,
    /// Pending requests that weren't completed
    pub pending_requests: Vec<RequestId>,
    /// Sequence number to continue from
    pub next_sequence: u64,
}

/// In-memory event store implementation.
pub struct InMemoryEventStore {
    events: Arc<RwLock<VecDeque<StoredEvent>>>,
    tokens: Arc<RwLock<HashMap<String, ResumptionState>>>,
    max_events: usize,
    max_age: chrono::Duration,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store.
    pub fn new(max_events: usize, max_age: chrono::Duration) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
            max_events,
            max_age,
        }
    }

    /// Clean up old events based on max_events and max_age.
    fn cleanup(&self) {
        let mut events = self.events.write();

        // Remove old events beyond max_events
        while events.len() > self.max_events {
            events.pop_front();
        }

        // Remove events older than max_age
        let cutoff = Utc::now() - self.max_age;
        while let Some(event) = events.front() {
            if event.timestamp < cutoff {
                events.pop_front();
            } else {
                break;
            }
        }
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn store_event(&self, event: StoredEvent) -> Result<()> {
        {
            let mut events = self.events.write();
            events.push_back(event);
        }
        self.cleanup();
        Ok(())
    }

    async fn get_events_since(
        &self,
        event_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<StoredEvent>> {
        let events = self.events.read();

        // Find the index of the event with the given ID
        let start_idx = events
            .iter()
            .position(|e| e.id == event_id)
            .map(|idx| idx + 1)
            .unwrap_or(0);

        let limit = limit.unwrap_or(usize::MAX);

        Ok(events.iter().skip(start_idx).take(limit).cloned().collect())
    }

    async fn get_latest_event_id(&self) -> Result<Option<String>> {
        let events = self.events.read();
        Ok(events.back().map(|e| e.id.clone()))
    }

    async fn clear_events_before(&self, timestamp: DateTime<Utc>) -> Result<usize> {
        let mut events = self.events.write();
        let initial_len = events.len();

        while let Some(event) = events.front() {
            if event.timestamp < timestamp {
                events.pop_front();
            } else {
                break;
            }
        }

        Ok(initial_len - events.len())
    }

    async fn create_resumption_token(&self) -> Result<ResumptionToken> {
        let token_id = Uuid::new_v4().to_string();
        let events = self.events.read();

        let last_event = events
            .back()
            .ok_or_else(|| Error::Internal("No events to create resumption token".into()))?;

        let state = ResumptionState {
            session_id: last_event.session_id.clone(),
            last_event_id: last_event.id.clone(),
            pending_requests: Vec::new(),
            next_sequence: last_event.sequence + 1,
        };

        self.tokens.write().insert(token_id.clone(), state);

        Ok(ResumptionToken {
            token: token_id,
            session_id: last_event.session_id.clone(),
            last_event_id: last_event.id.clone(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            metadata: HashMap::new(),
        })
    }

    async fn validate_resumption_token(&self, token: &str) -> Result<Option<ResumptionState>> {
        let tokens = self.tokens.read();
        Ok(tokens.get(token).cloned())
    }
}

/// Manager for handling connection resumption.
pub struct ResumptionManager {
    event_store: Arc<dyn EventStore>,
    session_id: String,
    sequence_counter: Arc<RwLock<u64>>,
}

impl ResumptionManager {
    /// Create a new resumption manager.
    pub fn new(event_store: Arc<dyn EventStore>, session_id: String) -> Self {
        Self {
            event_store,
            session_id,
            sequence_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Record an outbound message.
    pub async fn record_outbound(&self, message: TransportMessage) -> Result<()> {
        let sequence = {
            let mut counter = self.sequence_counter.write();
            let seq = *counter;
            *counter += 1;
            seq
        };

        let event = StoredEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            message,
            direction: MessageDirection::Outbound,
            session_id: self.session_id.clone(),
            sequence,
        };

        self.event_store.store_event(event).await
    }

    /// Record an inbound message.
    pub async fn record_inbound(&self, message: TransportMessage) -> Result<()> {
        let sequence = {
            let mut counter = self.sequence_counter.write();
            let seq = *counter;
            *counter += 1;
            seq
        };

        let event = StoredEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            message,
            direction: MessageDirection::Inbound,
            session_id: self.session_id.clone(),
            sequence,
        };

        self.event_store.store_event(event).await
    }

    /// Create a resumption token for the current state.
    pub async fn create_token(&self) -> Result<ResumptionToken> {
        self.event_store.create_resumption_token().await
    }

    /// Resume from a token, returning events since last acknowledgement.
    pub async fn resume_from_token(&self, token: &str) -> Result<Vec<StoredEvent>> {
        let state = self
            .event_store
            .validate_resumption_token(token)
            .await?
            .ok_or_else(|| Error::Internal("Invalid or expired resumption token".into()))?;

        // Update sequence counter
        *self.sequence_counter.write() = state.next_sequence;

        // Get events since the last acknowledged event
        self.event_store
            .get_events_since(&state.last_event_id, None)
            .await
    }

    /// Get pending events that need to be resent.
    pub async fn get_pending_events(&self, since_event_id: &str) -> Result<Vec<StoredEvent>> {
        let events = self
            .event_store
            .get_events_since(since_event_id, None)
            .await?;

        // Filter for outbound events that might need resending
        Ok(events
            .into_iter()
            .filter(|e| e.direction == MessageDirection::Outbound)
            .collect())
    }
}

/// Configuration for event store behavior.
#[derive(Debug, Clone)]
pub struct EventStoreConfig {
    /// Maximum number of events to keep
    pub max_events: usize,
    /// Maximum age of events to keep
    pub max_age: chrono::Duration,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
    /// Cleanup interval
    pub cleanup_interval: std::time::Duration,
}

impl Default for EventStoreConfig {
    fn default() -> Self {
        Self {
            max_events: 10000,
            max_age: chrono::Duration::hours(24),
            auto_cleanup: true,
            cleanup_interval: std::time::Duration::from_secs(300),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_event_store() {
        let store = InMemoryEventStore::new(100, chrono::Duration::hours(1));

        // Store an event
        let event = StoredEvent {
            id: "test-1".to_string(),
            timestamp: Utc::now(),
            message: TransportMessage::Request {
                id: RequestId::String("req-1".to_string()),
                request: crate::types::Request::Client(Box::new(
                    crate::types::ClientRequest::Initialize(crate::types::InitializeParams {
                        protocol_version: crate::types::ProtocolVersion::default(),
                        capabilities: crate::types::ClientCapabilities::default(),
                        client_info: crate::types::Implementation {
                            name: "test".to_string(),
                            version: "1.0.0".to_string(),
                        },
                    }),
                )),
            },
            direction: MessageDirection::Outbound,
            session_id: "session-1".to_string(),
            sequence: 0,
        };

        store.store_event(event.clone()).await.unwrap();

        // Retrieve events
        let events = store
            .get_events_since("non-existent", Some(10))
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "test-1");

        // Get latest event ID
        let latest_id = store.get_latest_event_id().await.unwrap();
        assert_eq!(latest_id, Some("test-1".to_string()));
    }

    #[tokio::test]
    async fn test_resumption_manager() {
        let store = Arc::new(InMemoryEventStore::new(100, chrono::Duration::hours(1)));
        let manager = ResumptionManager::new(store.clone(), "session-1".to_string());

        // Record some messages
        let msg = TransportMessage::Request {
            id: RequestId::String("req-1".to_string()),
            request: crate::types::Request::Client(Box::new(
                crate::types::ClientRequest::Initialize(crate::types::InitializeParams {
                    protocol_version: crate::types::ProtocolVersion::V2024_11_05,
                    capabilities: crate::types::ClientCapabilities::default(),
                    client_info: crate::types::Implementation {
                        name: "test".to_string(),
                        version: "1.0.0".to_string(),
                    },
                }),
            )),
        };

        manager.record_outbound(msg.clone()).await.unwrap();
        manager.record_inbound(msg).await.unwrap();

        // Create resumption token
        let token = manager.create_token().await.unwrap();
        assert!(!token.token.is_empty());

        // Resume from token
        let events = manager.resume_from_token(&token.token).await.unwrap();
        assert!(events.is_empty()); // No events after the last one
    }
}
