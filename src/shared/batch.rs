//! Batch request handling for JSON-RPC 2.0.
//!
//! This module provides support for processing multiple JSON-RPC requests
//! in a single batch, as per the JSON-RPC 2.0 specification.

use crate::error::{Error, Result};
use crate::types::{JSONRPCRequest, JSONRPCResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A batch of JSON-RPC requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BatchRequest {
    /// A single request
    Single(JSONRPCRequest),
    /// Multiple requests in a batch
    Batch(Vec<JSONRPCRequest>),
}

/// A batch of JSON-RPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BatchResponse {
    /// A single response
    Single(JSONRPCResponse),
    /// Multiple responses in a batch
    Batch(Vec<JSONRPCResponse>),
}

impl BatchRequest {
    /// Parse a JSON value into a batch request.
    pub fn from_value(value: Value) -> Result<Self> {
        serde_json::from_value(value)
            .map_err(|e| Error::parse(format!("Invalid batch request format: {}", e)))
    }

    /// Convert the batch request to a JSON value.
    pub fn to_value(&self) -> Result<Value> {
        serde_json::to_value(self)
            .map_err(|e| Error::internal(format!("Failed to serialize batch request: {}", e)))
    }

    /// Check if this is a batch request (multiple requests).
    pub fn is_batch(&self) -> bool {
        matches!(self, Self::Batch(_))
    }

    /// Get the requests as a vector.
    pub fn into_requests(self) -> Vec<JSONRPCRequest> {
        match self {
            Self::Single(req) => vec![req],
            Self::Batch(reqs) => reqs,
        }
    }

    /// Get the number of requests in the batch.
    pub fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Batch(reqs) => reqs.len(),
        }
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(_) => false,
            Self::Batch(reqs) => reqs.is_empty(),
        }
    }
}

impl BatchResponse {
    /// Create a batch response from a vector of responses.
    ///
    /// # Panics
    ///
    /// Panics if the vector has exactly 1 element but `next()` returns `None` (which should never happen).
    pub fn from_responses(responses: Vec<JSONRPCResponse>) -> Self {
        match responses.len() {
            0 => Self::Batch(vec![]),
            1 => Self::Single(responses.into_iter().next().unwrap()),
            _ => Self::Batch(responses),
        }
    }

    /// Convert the batch response to a JSON value.
    pub fn to_value(&self) -> Result<Value> {
        serde_json::to_value(self)
            .map_err(|e| Error::internal(format!("Failed to serialize batch response: {}", e)))
    }

    /// Get the responses as a vector.
    pub fn into_responses(self) -> Vec<JSONRPCResponse> {
        match self {
            Self::Single(resp) => vec![resp],
            Self::Batch(resps) => resps,
        }
    }
}

/// Process a batch of requests.
///
/// This function takes a batch request and a handler function, processes each
/// request (potentially in parallel), and returns a batch response.
pub async fn process_batch_request<F, Fut>(batch: BatchRequest, handler: F) -> Result<BatchResponse>
where
    F: Fn(JSONRPCRequest) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = JSONRPCResponse> + Send + 'static,
{
    let requests = batch.into_requests();

    // Empty batch should return empty array
    if requests.is_empty() {
        return Ok(BatchResponse::Batch(vec![]));
    }

    // Process all requests
    // Process requests in parallel while maintaining order
    let responses = if requests.len() > 1 {
        // Use parallel processing for multiple requests
        let config = crate::utils::parallel_batch::ParallelBatchConfig::default();
        crate::utils::parallel_batch::process_batch_parallel(requests, handler, config).await?
    } else {
        // For single request, process directly
        let mut responses = Vec::with_capacity(requests.len());
        for request in requests {
            let response = handler(request).await;
            responses.push(response);
        }
        responses
    };

    Ok(BatchResponse::from_responses(responses))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{jsonrpc::ResponsePayload, RequestId};
    use serde_json::json;

    #[test]
    fn test_single_request_parsing() {
        let json = json!({
            "jsonrpc": "2.0",
            "method": "test",
            "params": {"value": 42},
            "id": 1
        });

        let batch = BatchRequest::from_value(json).unwrap();
        assert!(!batch.is_batch());
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn test_batch_request_parsing() {
        let json = json!([
            {
                "jsonrpc": "2.0",
                "method": "test1",
                "id": 1
            },
            {
                "jsonrpc": "2.0",
                "method": "test2",
                "id": 2
            }
        ]);

        let batch = BatchRequest::from_value(json).unwrap();
        assert!(batch.is_batch());
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_empty_batch() {
        let json = json!([]);
        let batch = BatchRequest::from_value(json).unwrap();
        assert!(batch.is_batch());
        assert!(batch.is_empty());
    }

    #[tokio::test]
    async fn test_process_batch() {
        let batch = BatchRequest::Batch(vec![
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "test1".to_string(),
                params: None,
                id: RequestId::from(1i64),
            },
            JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                method: "test2".to_string(),
                params: None,
                id: RequestId::from(2i64),
            },
        ]);

        let result = process_batch_request(batch, |req| async move {
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id.clone(),
                payload: ResponsePayload::Result(json!({
                    "method": req.method,
                    "success": true
                })),
            }
        })
        .await
        .unwrap();

        let responses = result.into_responses();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].id, RequestId::from(1i64));
        assert_eq!(responses[1].id, RequestId::from(2i64));
    }
}
