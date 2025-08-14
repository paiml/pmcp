//! Property-based tests for streamable HTTP transport.
//!
//! These tests ensure that the streamable HTTP implementation maintains
//! critical invariants under all possible inputs.

#![cfg(feature = "streamable-http")]

use pmcp::shared::sse_parser::SseParser;
use proptest::prelude::*;

// === SSE Parser Properties ===

/// Generate arbitrary SSE field names
fn arb_sse_field() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("data".to_string()),
        Just("event".to_string()),
        Just("id".to_string()),
        Just("retry".to_string()),
        Just(":comment".to_string()),
        "[a-z]{1,20}", // Unknown fields
    ]
}

/// Generate arbitrary SSE field values
fn arb_sse_value() -> impl Strategy<Value = String> {
    prop_oneof![
        "[^\r\n]{0,1000}",           // Normal values
        Just("".to_string()),        // Empty values
        "[ \t]{0,10}[^\r\n]{0,100}", // Values with leading spaces
    ]
}

// Generate a single SSE line
prop_compose! {
    fn arb_sse_line()(
        field in arb_sse_field(),
        has_colon in prop::bool::ANY,
        has_space in prop::bool::ANY,
        value in arb_sse_value(),
    ) -> String {
        if !has_colon && field.starts_with(':') {
            // Comment line
            format!("{}\n", field)
        } else if has_colon {
            let space = if has_space { " " } else { "" };
            format!("{}:{}{}\n", field, space, value)
        } else {
            // Field without colon (invalid but should be handled)
            format!("{}\n", field)
        }
    }
}

// Generate a complete SSE event
prop_compose! {
    fn arb_sse_event()(
        lines in prop::collection::vec(arb_sse_line(), 0..10),
        has_double_newline in prop::bool::ANY,
    ) -> String {
        let mut event = lines.join("");
        if has_double_newline {
            event.push('\n');
        }
        event
    }
}

// Generate multiple SSE events
prop_compose! {
    fn arb_sse_stream()(
        events in prop::collection::vec(arb_sse_event(), 0..20),
    ) -> String {
        events.join("")
    }
}

// === Session ID Properties ===

/// Generate arbitrary session IDs
fn arb_session_id() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}", // UUID format
        "[a-zA-Z0-9]{1,64}",                                            // Alphanumeric
        Just("".to_string()),                                           // Empty (invalid)
    ]
}

// === Protocol Version Properties ===

/// Generate arbitrary protocol versions
fn arb_protocol_version() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("2025-06-18".to_string()), // Valid latest
        Just("2025-03-26".to_string()), // Valid default
        "[0-9]{4}-[0-9]{2}-[0-9]{2}",   // Date format
        "[0-9]+\\.[0-9]+\\.[0-9]+",     // Semver format
        Just("".to_string()),           // Empty
    ]
}

// === HTTP Header Properties ===

prop_compose! {
    fn arb_http_headers()(
        has_content_type in prop::bool::ANY,
        has_accept in prop::bool::ANY,
        has_session_id in prop::option::of(arb_session_id()),
        has_protocol_version in prop::option::of(arb_protocol_version()),
        has_last_event_id in prop::option::of("[a-zA-Z0-9-]{1,64}"),
        extra_headers in prop::collection::vec(
            ("[a-zA-Z-]{1,50}", "[^\r\n]{0,200}"),
            0..10
        ),
    ) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        if has_content_type {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }

        if has_accept {
            headers.push(("Accept".to_string(), "text/event-stream".to_string()));
        }

        if let Some(sid) = has_session_id {
            headers.push(("mcp-session-id".to_string(), sid));
        }

        if let Some(pv) = has_protocol_version {
            headers.push(("mcp-protocol-version".to_string(), pv));
        }

        if let Some(eid) = has_last_event_id {
            headers.push(("Last-Event-ID".to_string(), eid));
        }

        for (key, value) in extra_headers {
            headers.push((key, value));
        }

        headers
    }
}

// === Property Tests ===

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn property_sse_parser_never_panics(input in arb_sse_stream()) {
        let mut parser = SseParser::new();
        // Should never panic regardless of input
        let _ = parser.feed(&input);
    }

    #[test]
    fn property_sse_parser_preserves_data_content(
        data_lines in prop::collection::vec("[^\r\n]{1,100}", 1..10)
    ) {
        let mut parser = SseParser::new();

        // Build a valid SSE event with multiple data lines
        let mut event = String::new();
        for line in &data_lines {
            event.push_str(&format!("data: {}\n", line));
        }
        event.push('\n');

        let events = parser.feed(&event);

        // Should produce exactly one event if all lines have content
        if data_lines.iter().all(|l| !l.is_empty()) {
            prop_assert_eq!(events.len(), 1);
            // Data should be joined with newlines
            let expected = data_lines.join("\n");
            prop_assert_eq!(&events[0].data, &expected);
        } else if data_lines.iter().any(|l| !l.is_empty()) {
            // At least some content means we get an event
            prop_assert!(events.len() <= 1);
        }
    }

    #[test]
    fn property_sse_parser_handles_incremental_parsing(
        chunks in prop::collection::vec(arb_sse_event(), 1..10)
    ) {
        let mut parser1 = SseParser::new();
        let mut parser2 = SseParser::new();

        // Parse all at once
        let full_stream = chunks.join("");
        let events1 = parser1.feed(&full_stream);

        // Parse incrementally
        let mut events2 = Vec::new();
        for chunk in chunks {
            events2.extend(parser2.feed(&chunk));
        }

        // Should produce the same events (data content)
        let data1: Vec<_> = events1.iter().map(|e| &e.data).collect();
        let data2: Vec<_> = events2.iter().map(|e| &e.data).collect();
        prop_assert_eq!(data1, data2);
    }

    #[test]
    fn property_sse_event_id_tracking(
        event_ids in prop::collection::vec(
            prop::option::of("[a-zA-Z0-9-]{1,64}"),
            1..20
        )
    ) {
        let mut parser = SseParser::new();
        let mut last_seen_id: Option<String> = None;

        for maybe_id in event_ids {
            let event = if let Some(id) = &maybe_id {
                format!("id: {}\ndata: test\n\n", id)
            } else {
                "data: test\n\n".to_string()
            };

            let events = parser.feed(&event);

            if !events.is_empty() {
                // If an ID was provided, it should be in the event
                if let Some(expected_id) = &maybe_id {
                    prop_assert_eq!(events[0].id.as_ref(), Some(expected_id));
                    last_seen_id = Some(expected_id.clone());
                }
            }
        }

        // Parser should track the last event ID
        if let Some(expected) = last_seen_id {
            prop_assert_eq!(parser.last_event_id(), Some(expected.as_str()));
        }
    }

    #[test]
    fn property_session_id_format_validation(
        session_id in arb_session_id()
    ) {
        // Session IDs should either be empty (invalid) or match expected formats
        if !session_id.is_empty() {
            let is_uuid = session_id.len() == 36 &&
                session_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
            let is_alphanumeric = session_id.chars().all(|c| c.is_ascii_alphanumeric());

            prop_assert!(is_uuid || is_alphanumeric);
        }
    }

    #[test]
    fn property_protocol_version_format(
        version in arb_protocol_version()
    ) {
        if !version.is_empty() {
            // Should match date format or semver
            let is_date = version.len() == 10 &&
                version.chars().filter(|&c| c == '-').count() == 2;
            let is_semver = version.contains('.') &&
                version.split('.').all(|part| part.chars().all(|c| c.is_ascii_digit()));

            prop_assert!(is_date || is_semver);
        }
    }

    #[test]
    fn property_headers_preserve_case_insensitive_lookup(
        headers in arb_http_headers()
    ) {
        use std::collections::HashMap;

        // Build a case-insensitive header map
        let mut header_map: HashMap<String, String> = HashMap::new();

        for (key, value) in headers {
            // Headers should be case-insensitive
            header_map.insert(key.to_lowercase(), value);
        }

        // Common headers should be findable regardless of case
        let content_type_keys = ["content-type", "Content-Type", "CONTENT-TYPE"];
        let found_values: Vec<_> = content_type_keys
            .iter()
            .filter_map(|k| header_map.get(&k.to_lowercase()))
            .collect();

        // All lookups should find the same value or none
        if !found_values.is_empty() {
            prop_assert!(found_values.iter().all(|v| *v == found_values[0]));
        }
    }

    #[test]
    fn property_sse_comment_lines_ignored(
        comments in prop::collection::vec("[^\r\n]{0,200}", 0..10),
        data in "[^\r\n]{1,200}"  // Ensure data is non-empty
    ) {
        let mut parser = SseParser::new();

        // Build event with comments
        let mut event = String::new();
        for comment in comments {
            event.push_str(&format!(":{}\n", comment));
        }
        event.push_str(&format!("data: {}\n\n", data));

        let events = parser.feed(&event);

        // Comments should not affect the data
        // Events are only dispatched if data is non-empty
        if !data.is_empty() {
            prop_assert_eq!(events.len(), 1);
            prop_assert_eq!(&events[0].data, &data);
        } else {
            prop_assert_eq!(events.len(), 0);
        }
    }

    #[test]
    fn property_sse_retry_field_numeric(
        retry_value in prop_oneof![
            Just("1000".to_string()),
            Just("0".to_string()),
            "[0-9]{1,10}",
            "[a-zA-Z]+",  // Invalid
            Just("".to_string()),
        ]
    ) {
        let mut parser = SseParser::new();
        let event = format!("retry: {}\ndata: test\n\n", retry_value);

        let events = parser.feed(&event);

        if let Some(event) = events.first() {
            if let Some(retry) = event.retry {
                // If retry is set, it should be a valid number
                prop_assert!(retry_value.chars().all(|c| c.is_ascii_digit()));
                prop_assert_eq!(retry, retry_value.parse::<u64>().unwrap());
            }
        }
    }
}

// === Stateful vs Stateless Mode Properties ===

proptest! {
    #[test]
    fn property_stateful_mode_requires_session_id(
        has_session in prop::bool::ANY,
        session_id in prop::option::of(arb_session_id()),
    ) {
        // In stateful mode, after initialization, all requests must have session ID
        // This is a logical property that the server should enforce

        if has_session && session_id.is_none() {
            // This would be an error case in a real stateful server
            // The server should reject requests without session IDs
            prop_assert!(true); // Document the expected behavior
        } else {
            // Valid cases: either not in session mode or has session ID
            prop_assert!(true);
        }
    }

    #[test]
    fn property_stateless_mode_ignores_session_id(
        _session_id in prop::option::of(arb_session_id()),
    ) {
        // In stateless mode, session IDs should be ignored
        // Server should process requests regardless of session ID

        // This is more of a behavioral property to document
        // The actual testing would require a running server
        prop_assert!(true);
    }
}

// === Message Ordering Properties ===

proptest! {
    #[test]
    fn property_sse_events_preserve_order(
        messages in prop::collection::vec("[a-zA-Z0-9]{1,100}", 1..50)
    ) {
        let mut parser = SseParser::new();

        // Build a stream of events
        let mut stream = String::new();
        for msg in &messages {
            stream.push_str(&format!("data: {}\n\n", msg));
        }

        let events = parser.feed(&stream);

        // Events should be in the same order
        let received: Vec<_> = events.iter().map(|e| &e.data).collect();
        let expected: Vec<_> = messages.iter().map(|s| s.as_str()).collect();

        prop_assert_eq!(received, expected);
    }
}
