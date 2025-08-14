//! SSE (Server-Sent Events) parsing logic.

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
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    /// Track the last event ID for resumability
    last_event_id: Option<String>,
}

impl SseParser {
    /// Creates a new `SseParser`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the last event ID seen (for resumability)
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Parses a chunk of bytes and returns a vector of `SseEvent`s.
    pub fn parse(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        loop {
            // Find the end of an event, marked by double newline (\n\n or \r\n\r\n)
            let end_pos = self.find_event_boundary();

            if let Some((pos, boundary_len)) = end_pos {
                let event_bytes = &self.buffer[..pos];

                let mut id: Option<String> = None;
                let mut event_type = String::new();
                let mut data_lines = Vec::new();

                // Split lines handling both \n and \r\n
                let lines = split_lines(event_bytes);

                for line in lines {
                    let line_str = String::from_utf8_lossy(line);

                    // Handle field:value format
                    if let Some(colon_pos) = line_str.find(':') {
                        let field = &line_str[..colon_pos];
                        let value = if colon_pos + 1 < line_str.len() {
                            // Skip optional space after colon
                            if line_str.chars().nth(colon_pos + 1) == Some(' ') {
                                &line_str[colon_pos + 2..]
                            } else {
                                &line_str[colon_pos + 1..]
                            }
                        } else {
                            ""
                        };

                        match field {
                            "id" => {
                                id = Some(value.to_string());
                                self.last_event_id = Some(value.to_string());
                            },
                            "event" => {
                                event_type = value.to_string();
                            },
                            "data" => {
                                data_lines.push(value.to_string());
                            },
                            "retry" => {
                                // Ignore retry field for now
                            },
                            _ if field.starts_with(':') => {
                                // Comment, ignore
                            },
                            _ => {
                                // Unknown field, ignore
                            },
                        }
                    } else if line_str.starts_with(':') {
                        // Comment line, ignore
                    }
                }

                // Only emit event if we have data
                if !data_lines.is_empty() {
                    let data = data_lines.join("\n");
                    events.push(SseEvent {
                        id,
                        event: if event_type.is_empty() {
                            "message".to_string()
                        } else {
                            event_type
                        },
                        data,
                    });
                }

                // Drain the processed event from the buffer
                self.buffer.drain(..pos + boundary_len);
            } else {
                break;
            }
        }

        events
    }

    /// Find the position of event boundary (\n\n or \r\n\r\n)
    fn find_event_boundary(&self) -> Option<(usize, usize)> {
        // Look for \n\n
        if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
            return Some((pos, 2));
        }

        // Look for \r\n\r\n
        if let Some(pos) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            return Some((pos, 4));
        }

        // Look for \r\r (rare but possible)
        if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\r\r") {
            return Some((pos, 2));
        }

        None
    }
}

/// Split bytes into lines, handling both \n and \r\n line endings
fn split_lines(data: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;

    while i < data.len() {
        if i + 1 < data.len() && data[i] == b'\r' && data[i + 1] == b'\n' {
            // CRLF
            lines.push(&data[start..i]);
            start = i + 2;
            i += 2;
        } else if data[i] == b'\n' || data[i] == b'\r' {
            // LF or CR
            lines.push(&data[start..i]);
            start = i + 1;
            i += 1;
        } else {
            i += 1;
        }
    }

    // Add remaining data if any
    if start < data.len() {
        lines.push(&data[start..]);
    }

    lines
}
