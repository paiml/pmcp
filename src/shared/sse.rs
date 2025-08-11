//! SSE (Server-Sent Events) parsing logic.

use crate::error::{Result};

/// Represents a single Server-Sent Event.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SseEvent {
    /// The event ID, used for resumability.
    pub id: Option<String>,
    /// The event type. Defaults to "message".
    pub event: String,
    /// The event data.
    pub data: String,
}

/// A parser for Server-Sent Events.
#[derive(Debug)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    /// Creates a new `SseParser`.
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Parses a chunk of bytes and returns a vector of `SseEvent`s.
    pub fn parse(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        loop {
            // Find the end of an event, marked by double newline.
            let end_of_event_pos = self.buffer.windows(4).position(|window| window == b"\n\n");

            if let Some(pos) = end_of_event_pos {
                let event_bytes = &self.buffer[..pos];
                
                let mut id: Option<String> = None;
                let mut event_type = "message".to_string();
                let mut data = String::new();

                for line in event_bytes.split(|&b| b == b'\n') {
                    let line_str = String::from_utf8_lossy(line);
                    if let Some(id_val) = line_str.strip_prefix("id:") {
                        id = Some(id_val.trim().to_string());
                    } else if let Some(event_val) = line_str.strip_prefix("event:") {
                        event_type = event_val.trim().to_string();
                    } else if let Some(data_val) = line_str.strip_prefix("data:") {
                        if !data.is_empty() {
                            data.push('\n');
                        }
                        data.push_str(data_val.trim());
                    }
                }

                if !data.is_empty() {
                    events.push(SseEvent {
                        id,
                        event: event_type,
                        data,
                    });
                }
                
                // Drain the processed event from the buffer.
                self.buffer.drain(..pos + 4);
            } else {
                break;
            }
        }

        events
    }
}
