//! Server-Sent Events (SSE) parser for MCP HTTP transport.
//!
//! This module provides a robust SSE parser compatible with the
//! `EventSource` specification, similar to eventsource-parser in TypeScript.

use std::collections::HashMap;
use std::fmt;

/// SSE event parsed from the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Event ID for resumption
    pub id: Option<String>,
    /// Event type/name
    pub event: Option<String>,
    /// Event data
    pub data: String,
    /// Retry interval in milliseconds
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Create a new SSE event with data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("Hello, world!");
    /// assert_eq!(event.data, "Hello, world!");
    /// assert!(event.id.is_none());
    /// assert!(event.event.is_none());
    /// ```
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            id: None,
            event: None,
            data: data.into(),
            retry: None,
        }
    }

    /// Set the event ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_id("msg-123");
    /// assert_eq!(event.id, Some("msg-123".to_string()));
    /// ```
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the event type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_event("custom");
    /// assert_eq!(event.event, Some("custom".to_string()));
    /// ```
    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Set the retry interval.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_retry(3000);
    /// assert_eq!(event.retry, Some(3000));
    /// ```
    pub fn with_retry(mut self, retry: u64) -> Self {
        self.retry = Some(retry);
        self
    }
}

impl fmt::Display for SseEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(id) = &self.id {
            writeln!(f, "id: {}", id)?;
        }
        if let Some(event) = &self.event {
            writeln!(f, "event: {}", event)?;
        }
        if let Some(retry) = self.retry {
            writeln!(f, "retry: {}", retry)?;
        }

        // Split data by newlines and write each line
        for line in self.data.lines() {
            writeln!(f, "data: {}", line)?;
        }

        writeln!(f)?; // Empty line to end event
        Ok(())
    }
}

/// SSE parser state machine.
#[derive(Debug)]
pub struct SseParser {
    buffer: String,
    current_event: EventBuilder,
    last_event_id: Option<String>,
}

impl SseParser {
    /// Create a new SSE parser.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    /// assert!(parser.last_event_id().is_none());
    /// ```
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            current_event: EventBuilder::new(),
            last_event_id: None,
        }
    }

    /// Feed data to the parser and get parsed events.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    /// 
    /// // Simple event
    /// let events = parser.feed("data: Hello\n\n");
    /// assert_eq!(events.len(), 1);
    /// assert_eq!(events[0].data, "Hello");
    ///
    /// // Event with ID
    /// let events = parser.feed("id: 123\ndata: World\n\n");
    /// assert_eq!(events[0].id, Some("123".to_string()));
    /// assert_eq!(events[0].data, "World");
    ///
    /// // Multi-line data
    /// let events = parser.feed("data: Line 1\ndata: Line 2\n\n");
    /// assert_eq!(events[0].data, "Line 1\nLine 2");
    ///
    /// // Custom event type
    /// let events = parser.feed("event: ping\ndata: pong\n\n");
    /// assert_eq!(events[0].event, Some("ping".to_string()));
    /// ```
    pub fn feed(&mut self, data: &str) -> Vec<SseEvent> {
        self.buffer.push_str(data);
        let mut events = Vec::new();

        while let Some(line_end) = self.buffer.find('\n') {
            let line = if line_end > 0 && self.buffer.chars().nth(line_end - 1) == Some('\r') {
                self.buffer[..line_end - 1].to_string()
            } else {
                self.buffer[..line_end].to_string()
            };

            if let Some(event) = self.process_line(&line) {
                events.push(event);
            }

            self.buffer.drain(..=line_end);
        }

        events
    }

    /// Process a single line and potentially emit an event.
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Empty line dispatches the event
        if line.is_empty() {
            return self.dispatch_event();
        }

        // Comment line (starts with :)
        if line.starts_with(':') {
            return None;
        }

        // Parse field and value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            // Remove leading space from value if present
            let value = value.strip_prefix(' ').unwrap_or(value);
            (field, value)
        } else {
            // Field without value
            (line, "")
        };

        // Process field
        match field {
            "event" => {
                self.current_event.event = Some(value.to_string());
            },
            "data" => {
                if self.current_event.data.is_empty() {
                    self.current_event.data = value.to_string();
                } else {
                    self.current_event.data.push('\n');
                    self.current_event.data.push_str(value);
                }
            },
            "id" => {
                if !value.contains('\0') {
                    self.current_event.id = Some(value.to_string());
                    self.last_event_id = Some(value.to_string());
                }
            },
            "retry" => {
                if let Ok(retry) = value.parse::<u64>() {
                    self.current_event.retry = Some(retry);
                }
            },
            _ => {
                // Unknown field, ignore
            },
        }

        None
    }

    /// Dispatch the current event if it has data.
    fn dispatch_event(&mut self) -> Option<SseEvent> {
        if self.current_event.data.is_empty() {
            // No data, don't dispatch
            self.current_event = EventBuilder::new();
            return None;
        }

        let event = SseEvent {
            id: self
                .current_event
                .id
                .clone()
                .or_else(|| self.last_event_id.clone()),
            event: self.current_event.event.clone(),
            data: self.current_event.data.clone(),
            retry: self.current_event.retry,
        };

        self.current_event = EventBuilder::new();
        Some(event)
    }

    /// Get the last event ID seen.
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Reset the parser state.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.current_event = EventBuilder::new();
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SSE events during parsing.
#[derive(Debug, Clone)]
struct EventBuilder {
    id: Option<String>,
    event: Option<String>,
    data: String,
    retry: Option<u64>,
}

impl EventBuilder {
    fn new() -> Self {
        Self {
            id: None,
            event: None,
            data: String::new(),
            retry: None,
        }
    }
}

/// SSE stream builder for creating SSE responses.
#[derive(Debug)]
pub struct SseStream {
    events: Vec<SseEvent>,
}

impl SseStream {
    /// Create a new SSE stream builder.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to the stream.
    pub fn event(mut self, event: SseEvent) -> Self {
        self.events.push(event);
        self
    }

    /// Add a simple data event.
    pub fn data(self, data: impl Into<String>) -> Self {
        self.event(SseEvent::new(data))
    }

    /// Add a typed event with data.
    pub fn typed_event(self, event_type: impl Into<String>, data: impl Into<String>) -> Self {
        self.event(SseEvent::new(data).with_event(event_type))
    }

    /// Add a comment line.
    pub fn comment(self, _comment: impl Into<String>) -> Self {
        // Comments are not stored as events, they're just for keep-alive
        // In a real implementation, we'd write this directly to the stream
        self
    }

    /// Build the SSE stream as a string.
    pub fn build(self) -> String {
        self.events
            .into_iter()
            .map(|e| e.to_string())
            .collect::<String>()
    }
}

impl Default for SseStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for SSE connections.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Reconnection retry interval in milliseconds
    pub retry: u64,
    /// Maximum buffer size for incomplete lines
    pub max_buffer_size: usize,
    /// Enable compression
    pub compression: bool,
    /// Custom headers
    pub headers: HashMap<String, String>,
}

impl Default for SseConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        headers.insert("Connection".to_string(), "keep-alive".to_string());

        Self {
            retry: 3000,
            max_buffer_size: 1024 * 1024, // 1MB
            compression: false,
            headers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parser_simple() {
        let mut parser = SseParser::new();

        let input = "data: hello world\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].id, None);
    }

    #[test]
    fn test_sse_parser_with_event_type() {
        let mut parser = SseParser::new();

        let input = "event: message\ndata: hello\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event, Some("message".to_string()));
    }

    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();

        let input = "data: line 1\ndata: line 2\ndata: line 3\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_sse_parser_with_id() {
        let mut parser = SseParser::new();

        let input = "id: 123\ndata: test\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("123".to_string()));
        assert_eq!(parser.last_event_id(), Some("123"));
    }

    #[test]
    fn test_sse_parser_with_retry() {
        let mut parser = SseParser::new();

        let input = "retry: 5000\ndata: test\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn test_sse_parser_comments() {
        let mut parser = SseParser::new();

        let input = ": this is a comment\ndata: actual data\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "actual data");
    }

    #[test]
    fn test_sse_parser_incremental() {
        let mut parser = SseParser::new();

        // Feed data incrementally
        let events1 = parser.feed("data: par");
        assert_eq!(events1.len(), 0);

        let events2 = parser.feed("tial\ndata: more");
        assert_eq!(events2.len(), 0);

        let events3 = parser.feed("\n\n");
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0].data, "partial\nmore");
    }

    #[test]
    fn test_sse_stream_builder() {
        let stream = SseStream::new()
            .data("simple message")
            .typed_event("ping", "pong")
            .event(SseEvent::new("complex").with_id("42").with_retry(1000))
            .build();

        assert!(stream.contains("data: simple message"));
        assert!(stream.contains("event: ping"));
        assert!(stream.contains("data: pong"));
        assert!(stream.contains("id: 42"));
        assert!(stream.contains("retry: 1000"));
    }

    #[test]
    fn test_sse_event_display() {
        let event = SseEvent::new("test data")
            .with_id("123")
            .with_event("message")
            .with_retry(3000);

        let output = event.to_string();
        assert!(output.contains("id: 123"));
        assert!(output.contains("event: message"));
        assert!(output.contains("retry: 3000"));
        assert!(output.contains("data: test data"));
    }
}
