//! Standard I/O transport implementation.
//!
//! This transport uses stdin/stdout for communication, with length-prefixed
//! framing to ensure message boundaries are preserved.

use crate::error::{Result, TransportError};
use crate::shared::transport::{Transport, TransportMessage};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

/// Line-delimited JSON framing header.
const CONTENT_LENGTH_HEADER: &str = "Content-Length: ";

/// stdio transport for MCP communication.
///
/// Uses length-prefixed framing compatible with the TypeScript SDK.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::shared::StdioTransport;
///
/// # async fn example() -> pmcp::Result<()> {
/// let transport = StdioTransport::new();
/// // Use with Client or Server
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct StdioTransport {
    stdin: Mutex<BufReader<tokio::io::Stdin>>,
    stdout: Mutex<tokio::io::Stdout>,
    closed: std::sync::atomic::AtomicBool,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new() -> Self {
        Self {
            stdin: Mutex::new(BufReader::new(tokio::io::stdin())),
            stdout: Mutex::new(tokio::io::stdout()),
            closed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Parse a content-length header.
    fn parse_content_length(line: &str) -> Option<usize> {
        line.strip_prefix(CONTENT_LENGTH_HEADER)
            .and_then(|content| content.trim().parse().ok())
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(TransportError::ConnectionClosed.into());
        }

        let mut stdout = self.stdout.lock().await;

        // Serialize message to JSON
        let json_bytes = match &message {
            TransportMessage::Request { id, request } => {
                let jsonrpc_request = crate::shared::create_request(id.clone(), request.clone());
                serde_json::to_vec(&jsonrpc_request).map_err(|e| {
                    TransportError::InvalidMessage(format!("Failed to serialize request: {}", e))
                })?
            },
            TransportMessage::Response(response) => serde_json::to_vec(response).map_err(|e| {
                TransportError::InvalidMessage(format!("Failed to serialize response: {}", e))
            })?,
            TransportMessage::Notification(notification) => {
                let jsonrpc_notification = crate::shared::create_notification(notification.clone());
                serde_json::to_vec(&jsonrpc_notification).map_err(|e| {
                    TransportError::InvalidMessage(format!(
                        "Failed to serialize notification: {}",
                        e
                    ))
                })?
            },
        };

        // Write content-length header
        let header = format!("{}{}\r\n\r\n", CONTENT_LENGTH_HEADER, json_bytes.len());
        stdout
            .write_all(header.as_bytes())
            .await
            .map_err(TransportError::from)?;

        // Write message payload
        stdout
            .write_all(&json_bytes)
            .await
            .map_err(TransportError::from)?;

        // Always flush stdio
        stdout.flush().await.map_err(TransportError::from)?;

        drop(stdout);
        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(TransportError::ConnectionClosed.into());
        }

        let mut stdin = self.stdin.lock().await;
        let mut line = String::new();
        let mut content_length = None;

        // Read headers until we find content-length
        loop {
            line.clear();
            let bytes_read = stdin
                .read_line(&mut line)
                .await
                .map_err(TransportError::from)?;

            if bytes_read == 0 {
                // EOF reached
                self.closed
                    .store(true, std::sync::atomic::Ordering::Release);
                return Err(TransportError::ConnectionClosed.into());
            }

            let line = line.trim();

            if line.is_empty() {
                // End of headers
                break;
            }

            if let Some(length) = Self::parse_content_length(line) {
                content_length = Some(length);
            }
        }

        let content_length = content_length.ok_or_else(|| {
            TransportError::InvalidMessage("Missing Content-Length header".to_string())
        })?;

        // Read the message body
        let mut buffer = vec![0u8; content_length];
        stdin
            .read_exact(&mut buffer)
            .await
            .map_err(TransportError::from)?;

        drop(stdin);

        // Parse JSON to determine message type
        let json_value: serde_json::Value = serde_json::from_slice(&buffer)
            .map_err(|e| TransportError::InvalidMessage(format!("Invalid JSON: {}", e)))?;

        // Determine message type based on JSON structure
        if json_value.get("method").is_some() {
            if json_value.get("id").is_some() {
                // It's a request
                #[allow(clippy::redundant_clone)]
                let request: crate::types::JSONRPCRequest<serde_json::Value> =
                    serde_json::from_value(json_value.clone()).map_err(|e| {
                        TransportError::InvalidMessage(format!("Invalid request: {}", e))
                    })?;

                // Parse the method to determine if it's client or server request
                let parsed_request = crate::shared::parse_request(request).map_err(|e| {
                    TransportError::InvalidMessage(format!("Invalid request: {}", e))
                })?;

                Ok(TransportMessage::Request {
                    id: parsed_request.0,
                    request: parsed_request.1,
                })
            } else {
                // It's a notification
                let parsed_notification =
                    crate::shared::parse_notification(json_value).map_err(|e| {
                        TransportError::InvalidMessage(format!("Invalid notification: {}", e))
                    })?;

                Ok(TransportMessage::Notification(parsed_notification))
            }
        } else if json_value.get("result").is_some() || json_value.get("error").is_some() {
            // It's a response
            let response: crate::types::JSONRPCResponse = serde_json::from_value(json_value)
                .map_err(|e| TransportError::InvalidMessage(format!("Invalid response: {}", e)))?;

            Ok(TransportMessage::Response(response))
        } else {
            Err(TransportError::InvalidMessage("Unknown message type".to_string()).into())
        }
    }

    async fn close(&mut self) -> Result<()> {
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);

        // Flush any pending output
        let mut stdout = self.stdout.lock().await;
        stdout.flush().await.map_err(TransportError::from)?;
        drop(stdout);

        Ok(())
    }

    fn is_connected(&self) -> bool {
        !self.closed.load(std::sync::atomic::Ordering::Acquire)
    }

    fn transport_type(&self) -> &'static str {
        "stdio"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_length_valid() {
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length: 42"),
            Some(42)
        );
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length: 0"),
            Some(0)
        );
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length: 999999"),
            Some(999_999)
        );
    }

    #[test]
    fn parse_content_length_invalid() {
        assert_eq!(
            StdioTransport::parse_content_length("Content-Type: application/json"),
            None
        );
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length: abc"),
            None
        );
        assert_eq!(StdioTransport::parse_content_length(""), None);
    }

    #[tokio::test]
    async fn transport_properties() {
        let transport = StdioTransport::new();
        assert!(transport.is_connected());
        assert_eq!(transport.transport_type(), "stdio");
    }
}
