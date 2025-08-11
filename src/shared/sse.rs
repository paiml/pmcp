//! SSE (Server-Sent Events) parsing logic.

use crate::error::{Error, Result, TransportError};
use crate::shared::TransportMessage;

/// SSE (Server-Sent Events) parsing logic.
#[derive(Debug)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    /// Creates a new `SseParser`.
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Parses a chunk of bytes and returns a vector of `TransportMessage`s.
    pub fn parse(&mut self, chunk: &[u8]) -> Result<Vec<TransportMessage>> {
        self.buffer.extend_from_slice(chunk);
        let mut messages = Vec::new();

        loop {
            let buffer_str = String::from_utf8_lossy(&self.buffer);
            if let Some(end_of_event) = buffer_str.find("\n\n") {
                let event_str = &buffer_str[..end_of_event];
                let mut data = String::new();
                for line in event_str.lines() {
                    if let Some(line_data) = line.strip_prefix("data: ") {
                        data.push_str(line_data);
                    }
                }

                if !data.is_empty() {
                    let message: TransportMessage = serde_json::from_str(&data)
                        .map_err(|e| Error::Transport(TransportError::Deserialization(e.to_string())))?;
                    messages.push(message);
                }

                self.buffer.drain(..end_of_event + 2);
            } else {
                break;
            }
        }

        Ok(messages)
    }
}
