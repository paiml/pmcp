#[cfg(feature = "streamable-http")]
mod sse_parser_tests {
    use pmcp::shared::sse_parser::SseParser;

    #[test]
    fn test_simple_message() {
        let mut parser = SseParser::new();
        let data = b"data: hello world\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].id, None);
    }

    #[test]
    fn test_message_with_id() {
        let mut parser = SseParser::new();
        let data = b"id: 123\ndata: test message\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "test message");
        assert_eq!(events[0].id, Some("123".to_string()));
    }

    #[test]
    fn test_custom_event_type() {
        let mut parser = SseParser::new();
        let data = b"event: custom\ndata: custom data\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("custom".to_string()));
        assert_eq!(events[0].data, "custom data");
    }

    #[test]
    fn test_multiline_data() {
        let mut parser = SseParser::new();
        let data = b"data: line1\ndata: line2\ndata: line3\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2\nline3");
    }

    #[test]
    fn test_crlf_line_endings() {
        let mut parser = SseParser::new();
        let data = b"id: 456\r\ndata: windows style\r\n\r\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("456".to_string()));
        assert_eq!(events[0].data, "windows style");
    }

    #[test]
    fn test_mixed_line_endings() {
        let mut parser = SseParser::new();
        let data = b"id: 789\ndata: mixed\r\ndata: endings\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("789".to_string()));
        assert_eq!(events[0].data, "mixed\nendings");
    }

    #[test]
    fn test_comment_ignored() {
        let mut parser = SseParser::new();
        let data = b": this is a comment\ndata: actual data\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "actual data");
    }

    #[test]
    fn test_partial_parsing() {
        let mut parser = SseParser::new();

        // Send partial data
        let events1 = parser.feed(std::str::from_utf8(b"data: partial").unwrap());
        assert_eq!(events1.len(), 0); // No complete event yet

        // Complete the event
        let events2 = parser.feed(std::str::from_utf8(b" message\n\n").unwrap());
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "partial message");
    }

    #[test]
    fn test_multiple_events() {
        let mut parser = SseParser::new();
        let data = b"data: event1\n\ndata: event2\n\nid: 3\ndata: event3\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, "event1");
        assert_eq!(events[1].data, "event2");
        assert_eq!(events[2].data, "event3");
        assert_eq!(events[2].id, Some("3".to_string()));
    }

    #[test]
    fn test_json_data() {
        let mut parser = SseParser::new();
        let json = r#"{"method":"ping","id":1}"#;
        let data = format!("data: {}\n\n", json);

        let events = parser.feed(data.as_str());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, json);
    }

    #[test]
    fn test_empty_data_ignored() {
        let mut parser = SseParser::new();
        let data = b"event: empty\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 0); // No data, so no event
    }

    #[test]
    fn test_space_after_colon() {
        let mut parser = SseParser::new();
        let data = b"data:no space\n\ndata: with space\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "no space");
        assert_eq!(events[1].data, "with space");
    }

    #[test]
    fn test_last_event_id_tracking() {
        let mut parser = SseParser::new();

        // Parse first event with ID
        let data1 = b"id: 100\ndata: first\n\n";
        let events1 = parser.feed(std::str::from_utf8(data1).unwrap());
        assert_eq!(events1.len(), 1);
        assert_eq!(parser.last_event_id(), Some("100"));

        // Parse event without ID - should keep last ID
        let data2 = b"data: second\n\n";
        let events2 = parser.feed(std::str::from_utf8(data2).unwrap());
        assert_eq!(events2.len(), 1);
        assert_eq!(parser.last_event_id(), Some("100"));

        // Parse event with new ID
        let data3 = b"id: 200\ndata: third\n\n";
        let events3 = parser.feed(std::str::from_utf8(data3).unwrap());
        assert_eq!(events3.len(), 1);
        assert_eq!(parser.last_event_id(), Some("200"));
    }

    #[test]
    fn test_retry_field_ignored() {
        let mut parser = SseParser::new();
        let data = b"retry: 1000\ndata: message\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "message");
    }

    #[test]
    fn test_unknown_field_ignored() {
        let mut parser = SseParser::new();
        let data = b"unknown: field\ndata: message\n\n";

        let events = parser.feed(std::str::from_utf8(data).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "message");
    }
}
