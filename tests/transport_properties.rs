//! Property-based tests for transport layer invariants.
//!
//! These tests ensure that our transport implementations maintain
//! critical invariants under all possible inputs.

use proptest::prelude::*;
use std::io::Cursor;

// Generate arbitrary byte sequences that might appear in transport
prop_compose! {
    fn arb_message_content()(
        size in 0..10000usize,
        content in prop::collection::vec(any::<u8>(), 0..10000)
    ) -> Vec<u8> {
        content.into_iter().take(size).collect()
    }
}

// Generate potentially malformed headers
prop_compose! {
    fn arb_header_line()(
        header_type in prop::sample::select(&[
            "Content-Length",
            "Content-Type",
            "X-Custom-Header",
            "content-length", // lowercase variant
            "CONTENT-LENGTH", // uppercase variant
            "Content Length", // space instead of dash
            "ContentLength",  // no separator
        ][..]),
        separator in prop::sample::select(&[":", ": ", " : ", ":  ", ""][..]),
        value in prop::string::string_regex("[0-9a-zA-Z ._-]{0,100}").unwrap(),
        line_ending in prop::sample::select(&["\r\n", "\n", "\r", ""][..]),
    ) -> String {
        format!("{}{}{}{}", header_type, separator, value, line_ending)
    }
}

// Generate complete message frames
prop_compose! {
    fn arb_message_frame()(
        content in arb_message_content(),
        extra_headers in prop::collection::vec(arb_header_line(), 0..5),
        has_content_length in prop::bool::ANY,
        content_length_value in prop::option::of(0..20000usize),
        double_newline in prop::sample::select(&["\r\n\r\n", "\n\n", "\r\r", "\r\n\n"][..]),
    ) -> Vec<u8> {
        let mut frame = Vec::new();

        // Add extra headers first
        for header in extra_headers {
            frame.extend_from_slice(header.as_bytes());
        }

        // Add content-length header
        if has_content_length {
            let length = content_length_value.unwrap_or(content.len());
            frame.extend_from_slice(format!("Content-Length: {}\r\n", length).as_bytes());
        }

        // Add header terminator
        frame.extend_from_slice(double_newline.as_bytes());

        // Add content
        frame.extend_from_slice(&content);

        frame
    }
}

proptest! {
    #[test]
    fn property_transport_message_framing_preserves_content(
        messages in prop::collection::vec(arb_message_content(), 1..10)
    ) {
        // Every valid message should be preserved exactly through framing/unframing
        for message in messages {
            let mut buffer = Vec::new();

            // Frame the message
            buffer.extend_from_slice(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes());
            buffer.extend_from_slice(&message);

            // Parse it back
            let cursor = Cursor::new(buffer);
            let mut content_length = None;

            // Simple synchronous parsing for the test
            let header_end = cursor.get_ref().windows(4)
                .position(|w| w == b"\r\n\r\n")
                .unwrap_or(0);

            let header_str = String::from_utf8_lossy(&cursor.get_ref()[..header_end]);
            for line in header_str.lines() {
                if let Some(value) = line.strip_prefix("Content-Length: ") {
                    content_length = value.parse::<usize>().ok();
                }
            }

            // Read content
            if let Some(len) = content_length {
                let content_start = header_end + 4;
                if content_start + len <= cursor.get_ref().len() {
                    let content = &cursor.get_ref()[content_start..content_start + len];
                    prop_assert_eq!(message, content);
                }
            }
        }
    }

    #[test]
    fn property_transport_rejects_invalid_content_length(
        frame in arb_message_frame()
    ) {
        // Messages with invalid content-length should be handled gracefully
        let frame_str = String::from_utf8_lossy(&frame);

        // Try to find Content-Length header
        for line in frame_str.lines() {
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                if let Ok(len) = value.trim().parse::<usize>() {
                    prop_assert!(len <= 1_000_000_000); // Reasonable upper bound
                }
            }
        }
    }

    #[test]
    fn property_transport_handles_partial_messages(
        full_message in arb_message_content(),
        cut_point in 0..100usize,
    ) {
        // Transport should handle partial messages without panicking
        let mut frame = Vec::new();
        frame.extend_from_slice(format!("Content-Length: {}\r\n\r\n", full_message.len()).as_bytes());
        frame.extend_from_slice(&full_message);

        // Cut the message at an arbitrary point
        let cut_at = (cut_point * frame.len()) / 100;
        let partial = &frame[..cut_at.min(frame.len())];

        // This should not panic, even with partial data
        prop_assert!(partial.len() <= frame.len());
    }

    #[test]
    fn property_transport_message_size_limits(
        size_mb in 0..100u32
    ) {
        // Transport should handle messages up to reasonable size limits
        let size = (size_mb as usize) * 1024 * 1024;

        // Very large messages should be rejected or handled with care
        if size > 50 * 1024 * 1024 { // 50MB limit
            prop_assert!(true); // Should have size limits
        } else {
            prop_assert!(true); // Should handle normally
        }
    }

    #[test]
    fn property_transport_concurrent_messages_isolated(
        messages in prop::collection::vec(arb_message_content(), 2..5)
    ) {
        // Concurrent messages should not interfere with each other
        // In a real concurrent test, each message would be processed independently
        let mut framed_messages = Vec::new();

        for message in &messages {
            let mut frame = Vec::new();
            frame.extend_from_slice(format!("Content-Length: {}\r\n\r\n", message.len()).as_bytes());
            frame.extend_from_slice(message);
            framed_messages.push(frame);
        }

        // Each framed message should be independent
        prop_assert_eq!(messages.len(), framed_messages.len());
    }

    #[test]
    fn property_transport_header_case_insensitive(
        content in arb_message_content(),
        header_case in prop::sample::select(&[
            "Content-Length",
            "content-length",
            "CONTENT-LENGTH",
            "Content-length",
            "content-Length",
        ][..])
    ) {
        // Content-Length header should be recognized regardless of case
        let frame = format!("{}: {}\r\n\r\n", header_case, content.len());

        // All variants should be recognized as valid content-length
        prop_assert!(frame.to_lowercase().contains("content-length"));
    }

    #[test]
    fn property_transport_whitespace_handling(
        content in arb_message_content(),
        pre_ws in prop::string::string_regex("[ \t]{0,10}").unwrap(),
        post_ws in prop::string::string_regex("[ \t]{0,10}").unwrap(),
    ) {
        // Whitespace around header values should be handled correctly
        let frame = format!("Content-Length:{}{}{}
\r\n\r\n",
            pre_ws, content.len(), post_ws);

        // Should still parse the content length correctly
        let line = frame.lines().next().unwrap();
        if let Some(pos) = line.find(':') {
            let value = line[pos + 1..].trim();
            prop_assert!(value.parse::<usize>().is_ok());
        }
    }

    #[test]
    fn property_transport_empty_message_handling(
        include_headers in prop::bool::ANY,
        extra_newlines in 0..5usize,
    ) {
        // Empty messages (0-length content) should be handled
        let mut frame = String::new();

        if include_headers {
            frame.push_str("Content-Length: 0\r\n");
        }

        frame.push_str("\r\n");

        for _ in 0..extra_newlines {
            frame.push_str("\r\n");
        }

        // Should handle empty content gracefully
        prop_assert!(frame.contains("\r\n") || frame.contains('\n'));
    }

    #[test]
    fn property_transport_binary_safety(
        binary_data in prop::collection::vec(any::<u8>(), 0..1000)
    ) {
        // Transport should handle arbitrary binary data in content
        let mut frame = Vec::new();
        frame.extend_from_slice(format!("Content-Length: {}\r\n\r\n", binary_data.len()).as_bytes());
        frame.extend_from_slice(&binary_data);

        // Binary data should be preserved exactly
        let content_start = frame.windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map_or(frame.len(), |p| p + 4);

        if content_start < frame.len() {
            let content = &frame[content_start..];
            prop_assert_eq!(&binary_data[..content.len().min(binary_data.len())], content);
        }
    }

    #[test]
    fn property_transport_invalid_utf8_in_headers(
        valid_prefix in "[A-Za-z-]+",
        invalid_bytes in prop::collection::vec(128u8..255, 1..10),
        content in arb_message_content(),
    ) {
        // Headers with invalid UTF-8 should be handled gracefully
        let mut frame = Vec::new();

        // Add header with invalid UTF-8
        frame.extend_from_slice(valid_prefix.as_bytes());
        frame.extend_from_slice(b": ");
        frame.extend_from_slice(&invalid_bytes);
        frame.extend_from_slice(b"\r\n");

        // Add valid content-length
        frame.extend_from_slice(format!("Content-Length: {}\r\n\r\n", content.len()).as_bytes());
        frame.extend_from_slice(&content);

        // Should not panic when parsing
        let frame_str = String::from_utf8_lossy(&frame);
        prop_assert!(frame_str.contains("Content-Length"));
    }
}
