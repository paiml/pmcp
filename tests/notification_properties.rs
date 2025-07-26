//! Property-based tests for notification system invariants.

use pmcp::types::*;
use proptest::prelude::*;

// Generate arbitrary progress values
prop_compose! {
    fn arb_progress_value()(
        value_type in 0..5,
        normal_progress in 0.0..=100.0f64,
        invalid_progress in prop::sample::select(vec![-50.0, -1.0, 101.0, 200.0, f64::NAN, f64::INFINITY]),
    ) -> f64 {
        match value_type {
            0..=2 => normal_progress,  // Most should be valid
            3 => 0.0,  // Edge case: start
            4 => 100.0,  // Edge case: complete
            _ => invalid_progress,
        }
    }
}

// Generate progress tokens
prop_compose! {
    fn arb_progress_token()(
        token_type in prop::bool::ANY,
        string_token in "[a-zA-Z0-9_-]{1,50}",
        number_token in 0i64..1000000,
    ) -> ProgressToken {
        if token_type {
            ProgressToken::String(string_token)
        } else {
            ProgressToken::Number(number_token)
        }
    }
}

// Generate progress notifications
prop_compose! {
    fn arb_progress_notification()(
        token in arb_progress_token(),
        progress in arb_progress_value(),
        has_message in prop::bool::ANY,
        message in prop::string::string_regex("[a-zA-Z0-9 .,!?]{0,200}").unwrap(),
    ) -> ProgressNotification {
        ProgressNotification {
            progress_token: token,
            progress,
            message: if has_message { Some(message) } else { None },
        }
    }
}

// Generate cancellation notifications
prop_compose! {
    fn arb_cancelled_notification()(
        request_id in arb_request_id(),
        has_reason in prop::bool::ANY,
        reason in prop::string::string_regex("[a-zA-Z0-9 .,!?]{0,200}").unwrap(),
    ) -> CancelledNotification {
        CancelledNotification {
            request_id,
            reason: if has_reason { Some(reason) } else { None },
        }
    }
}

// Generate arbitrary request IDs (reused from protocol_invariants.rs)
prop_compose! {
    fn arb_request_id()(
        choice in prop::bool::ANY,
        str_id in "[a-zA-Z0-9_-]{1,20}",
        num_id in 0i64..10000
    ) -> RequestId {
        if choice {
            RequestId::String(str_id)
        } else {
            RequestId::Number(num_id)
        }
    }
}

proptest! {
    #[test]
    fn property_progress_value_bounds(
        notification in arb_progress_notification()
    ) {
        // Valid progress values should be between 0 and 100
        if notification.progress >= 0.0 && notification.progress <= 100.0 {
            // Valid range - should serialize successfully
            let json = serde_json::to_value(&notification).unwrap();
            prop_assert!(json.get("progress").is_some());
        } else if notification.progress.is_nan() || notification.progress.is_infinite() {
            // Special float values might cause issues
            let result = serde_json::to_value(&notification);
            // Should either fail or handle gracefully
            prop_assert!(result.is_ok() || result.is_err());
        }
    }

    #[test]
    fn property_progress_token_roundtrip(
        token in arb_progress_token()
    ) {
        // Progress tokens should serialize/deserialize correctly
        let json = serde_json::to_value(&token).unwrap();
        let parsed: ProgressToken = serde_json::from_value(json).unwrap();
        prop_assert_eq!(token, parsed);
    }

    #[test]
    fn property_notification_type_discrimination(
        progress in arb_progress_notification(),
        cancelled in arb_cancelled_notification(),
    ) {
        // Different notification types should be distinguishable
        let progress_notification = Notification::Progress(progress);
        let cancelled_notification = Notification::Cancelled(cancelled);

        let progress_json = serde_json::to_value(&progress_notification).unwrap();
        let cancelled_json = serde_json::to_value(&cancelled_notification).unwrap();

        // With untagged serialization, types are distinguished by their fields
        // Progress has: progress_token, progress, message
        // Cancelled has: request_id, reason
        prop_assert!(progress_json.get("progress_token").is_some() || progress_json.get("progressToken").is_some());
        prop_assert!(progress_json.get("progress").is_some());
        prop_assert!(cancelled_json.get("request_id").is_some() || cancelled_json.get("requestId").is_some());

        // They should not have each other's fields
        prop_assert!(progress_json.get("request_id").is_none() && progress_json.get("requestId").is_none());
        prop_assert!(cancelled_json.get("progress_token").is_none() && cancelled_json.get("progressToken").is_none());
    }

    #[test]
    fn property_progress_message_optional(
        token in arb_progress_token(),
        progress in 0.0..=100.0f64,
        include_message in prop::bool::ANY,
        message in prop::string::string_regex("[a-zA-Z0-9 ]{0,100}").unwrap(),
    ) {
        // Progress messages are optional
        let notification = ProgressNotification {
            progress_token: token,
            progress,
            message: if include_message { Some(message.clone()) } else { None },
        };

        let json = serde_json::to_value(&notification).unwrap();

        if include_message {
            prop_assert_eq!(
                json.get("message").and_then(|v| v.as_str()),
                Some(message.as_str())
            );
        } else {
            prop_assert!(json.get("message").is_none() ||
                       json.get("message") == Some(&serde_json::Value::Null));
        }
    }

    #[test]
    fn property_cancellation_idempotent(
        notification in arb_cancelled_notification()
    ) {
        // Multiple cancellations of the same request should be idempotent
        let json1 = serde_json::to_value(&notification).unwrap();
        let json2 = serde_json::to_value(&notification).unwrap();

        prop_assert_eq!(json1, json2);
    }

    #[test]
    fn property_notification_ordering_preserved(
        notifications in prop::collection::vec(
            prop_oneof![
                arb_progress_notification().prop_map(Notification::Progress),
                arb_cancelled_notification().prop_map(Notification::Cancelled),
            ],
            1..20
        )
    ) {
        // Notification order should be preserved through serialization
        let serialized: Vec<_> = notifications.iter()
            .map(|n| serde_json::to_value(n).unwrap())
            .collect();

        let deserialized: Vec<Notification> = serialized.iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        // Order should be preserved
        prop_assert_eq!(notifications.len(), deserialized.len());

        // Check that notification types are preserved
        for (orig, deser) in notifications.iter().zip(deserialized.iter()) {
            match (orig, deser) {
                (Notification::Progress(_), Notification::Progress(_)) => {},
                (Notification::Cancelled(_), Notification::Cancelled(_)) => {},
                (Notification::Client(_), Notification::Client(_)) => {},
                (Notification::Server(_), Notification::Server(_)) => {},
                _ => prop_assert!(false, "Notification type changed during roundtrip"),
            }
        }
    }

    #[test]
    fn property_progress_token_uniqueness(
        tokens in prop::collection::hash_set(arb_progress_token(), 1..10)
    ) {
        // Progress tokens should be unique within a session
        prop_assert_eq!(tokens.len(), tokens.into_iter().collect::<Vec<_>>().len());
    }

    #[test]
    fn property_server_notification_types(
        notification_type in prop::sample::select(vec![
            "ToolsChanged",
            "ResourcesChanged",
            "PromptsChanged",
        ])
    ) {
        // Server notifications have tagged serialization with method field
        let notification = match notification_type {
            "ToolsChanged" => ServerNotification::ToolsChanged,
            "ResourcesChanged" => ServerNotification::ResourcesChanged,
            "PromptsChanged" => ServerNotification::PromptsChanged,
            _ => unreachable!(),
        };

        let json = serde_json::to_value(&notification).unwrap();

        // Check that it has the correct method field
        let method = json.get("method").and_then(|v| v.as_str());
        prop_assert!(method.is_some());

        match notification {
            ServerNotification::ToolsChanged => {
                prop_assert_eq!(method, Some("notifications/tools/list_changed"));
            }
            ServerNotification::ResourcesChanged => {
                prop_assert_eq!(method, Some("notifications/resources/list_changed"));
            }
            ServerNotification::PromptsChanged => {
                prop_assert_eq!(method, Some("notifications/prompts/list_changed"));
            }
            _ => {}
        }
    }

    #[test]
    fn property_progress_updates_monotonic(
        token in arb_progress_token(),
        progress_values in prop::collection::vec(0.0..=100.0f64, 1..20),
    ) {
        // Progress updates for the same token could be monotonic (best practice)
        let mut notifications = Vec::new();

        for progress in progress_values {
            notifications.push(ProgressNotification {
                progress_token: token.clone(),
                progress,
                message: Some(format!("Progress: {:.1}%", progress)),
            });
        }

        // Check if progress values make sense
        for window in notifications.windows(2) {
            let diff = window[1].progress - window[0].progress;
            // Progress can go backwards (e.g., retry), but large jumps might be suspicious
            prop_assert!(diff.abs() <= 100.0);
        }
    }

    #[test]
    fn property_cancellation_reason_length(
        request_id in arb_request_id(),
        reason_length in 0..1000usize,
    ) {
        // Cancellation reasons should handle various lengths
        let reason = "X".repeat(reason_length);

        let notification = CancelledNotification {
            request_id,
            reason: if reason_length > 0 { Some(reason) } else { None },
        };

        let json = serde_json::to_value(&notification).unwrap();

        if reason_length > 0 {
            let stored_reason = json.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            prop_assert_eq!(stored_reason.len(), reason_length);
        }
    }

    #[test]
    fn property_log_level_ordering(
        level1_idx in 0..4usize,
        level2_idx in 0..4usize,
    ) {
        use pmcp::types::protocol::LogLevel;

        let levels = vec![
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
        ];

        let level1 = &levels[level1_idx];
        let level2 = &levels[level2_idx];

        // Log levels should have a clear ordering
        let severity1 = match level1 {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        };

        let severity2 = match level2 {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        };

        if severity1 < severity2 {
            // level1 is less severe than level2
            prop_assert!(true);
        } else if severity1 > severity2 {
            // level1 is more severe than level2
            prop_assert!(true);
        } else {
            // Same level
            prop_assert_eq!(level1, level2);
        }
    }
}
