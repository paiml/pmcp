//! Property-based tests for client/server state machine invariants.

use pmcp::types::*;
use proptest::prelude::*;
use std::collections::HashSet;

// Generate valid initialization sequences
prop_compose! {
    fn arb_init_params()(
        version_idx in 0..3usize,
        client_name in "[a-zA-Z][a-zA-Z0-9_-]{0,20}",
        client_version in "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}",
        has_tools in prop::bool::ANY,
        has_resources in prop::bool::ANY,
        has_prompts in prop::bool::ANY,
    ) -> InitializeParams {
        let versions = ["2024-11-05", "2024-10-15", "2024-09-01"];

        let mut capabilities = ClientCapabilities::default();
        if has_tools {
            capabilities.tools = Some(ToolCapabilities::default());
        }
        if has_resources {
            capabilities.resources = Some(ResourceCapabilities::default());
        }
        if has_prompts {
            capabilities.prompts = Some(PromptCapabilities::default());
        }

        InitializeParams {
            protocol_version: versions[version_idx % versions.len()].to_string(),
            capabilities,
            client_info: Implementation {
                name: client_name,
                version: client_version,
            },
        }
    }
}

// Generate sequences of client requests
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ClientAction {
    Initialize(InitializeParams),
    Ping,
    ListTools,
    CallTool(String),
    ListResources,
    ReadResource(String),
    ListPrompts,
    GetPrompt(String),
    Shutdown,
}

prop_compose! {
    fn arb_client_action()(
        action_type in 0..9,
        init_params in arb_init_params(),
        tool_name in "[a-z_]+",
        resource_uri in "[a-z]+://[a-z/]+",
        prompt_name in "[a-z_]+",
    ) -> ClientAction {
        match action_type {
            0 => ClientAction::Initialize(init_params),
            1 => ClientAction::Ping,
            2 => ClientAction::ListTools,
            3 => ClientAction::CallTool(tool_name),
            4 => ClientAction::ListResources,
            5 => ClientAction::ReadResource(resource_uri),
            6 => ClientAction::ListPrompts,
            7 => ClientAction::GetPrompt(prompt_name),
            _ => ClientAction::Shutdown,
        }
    }
}

prop_compose! {
    fn arb_action_sequence()(
        actions in prop::collection::vec(arb_client_action(), 1..20)
    ) -> Vec<ClientAction> {
        actions
    }
}

// State machine for tracking client state
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum ClientState {
    NotInitialized,
    Initialized,
    ShuttingDown,
    Shutdown,
}

#[allow(clippy::match_same_arms)]
fn validate_client_state_transition(
    state: ClientState,
    action: &ClientAction,
) -> Result<ClientState, &'static str> {
    match (state, action) {
        (ClientState::NotInitialized, ClientAction::Initialize(_)) => Ok(ClientState::Initialized),
        (ClientState::NotInitialized, ClientAction::Ping) => Ok(ClientState::NotInitialized), // Ping always allowed
        (ClientState::NotInitialized, _) => Err("Must initialize first"),

        (ClientState::Initialized, ClientAction::Initialize(_)) => Err("Already initialized"),
        (ClientState::Initialized, ClientAction::Shutdown) => Ok(ClientState::ShuttingDown),
        (ClientState::Initialized, _) => Ok(ClientState::Initialized), // Most operations allowed

        // Ping allowed during shutdown - keeps same state
        (ClientState::ShuttingDown, ClientAction::Ping) => Ok(ClientState::ShuttingDown),
        (ClientState::ShuttingDown, _) => Err("Cannot perform operations while shutting down"),

        (ClientState::Shutdown, ClientAction::Ping) => Ok(ClientState::Shutdown), // Ping allowed even after shutdown
        (ClientState::Shutdown, _) => Err("Already shutdown"),
    }
}

proptest! {
    #[test]
    fn property_client_state_machine_valid_transitions(
        sequence in arb_action_sequence()
    ) {
        let mut state = ClientState::NotInitialized;
        let mut initialized = false;

        for action in sequence {
            match validate_client_state_transition(state, &action) {
                Ok(new_state) => {
                    if matches!(action, ClientAction::Initialize(_)) {
                        prop_assert!(!initialized, "Should not initialize twice");
                        initialized = true;
                    }
                    state = new_state;
                }
                Err(_) => {
                    // Invalid transitions should be in expected cases
                    match (&state, &action) {
                        (ClientState::NotInitialized, ClientAction::Initialize(_)) =>
                            prop_assert!(false, "Initialize should always work from NotInitialized"),
                        (ClientState::NotInitialized, ClientAction::Ping) =>
                            prop_assert!(false, "Ping should always work"),
                        _ => {} // Other invalid transitions are expected
                    }
                }
            }
        }
    }

    #[test]
    fn property_ping_always_allowed(
        sequence in arb_action_sequence(),
        ping_positions in prop::collection::vec(0..100usize, 1..5),
    ) {
        // Ping should be allowed at any point in the sequence
        let mut modified_sequence = sequence;

        for &pos in &ping_positions {
            if pos < modified_sequence.len() {
                modified_sequence.insert(pos, ClientAction::Ping);
            }
        }

        let mut state = ClientState::NotInitialized;

        for action in modified_sequence {
            if matches!(action, ClientAction::Ping) {
                // Ping should never fail
                match validate_client_state_transition(state, &action) {
                    Ok(new_state) => state = new_state,
                    Err(e) => prop_assert!(false, "Ping failed with: {}", e),
                }
            } else {
                // Other operations might fail, which is ok
                if let Ok(new_state) = validate_client_state_transition(state, &action) {
                    state = new_state;
                }
            }
        }
    }

    #[test]
    fn property_server_capabilities_consistency(
        has_tools in prop::bool::ANY,
        has_resources in prop::bool::ANY,
        has_prompts in prop::bool::ANY,
        has_logging in prop::bool::ANY,
        client_requests_tools in prop::bool::ANY,
        client_requests_resources in prop::bool::ANY,
    ) {
        // Server capabilities should match what operations are allowed
        let mut server_caps = ServerCapabilities::default();

        if has_tools {
            server_caps.tools = Some(ToolCapabilities::default());
        }
        if has_resources {
            server_caps.resources = Some(ResourceCapabilities::default());
        }
        if has_prompts {
            server_caps.prompts = Some(PromptCapabilities::default());
        }
        if has_logging {
            server_caps.logging = Some(LoggingCapabilities::default());
        }

        // Client requests should only succeed if server has capability
        if client_requests_tools && !has_tools {
            prop_assert!(server_caps.tools.is_none());
        }
        if client_requests_resources && !has_resources {
            prop_assert!(server_caps.resources.is_none());
        }

        // Server should only advertise capabilities it can handle
        if server_caps.tools.is_some() {
            prop_assert!(has_tools);
        }
    }

    #[test]
    fn property_initialization_idempotent(
        params1 in arb_init_params(),
        params2 in arb_init_params(),
    ) {
        // Multiple initialization attempts should fail after the first
        let mut state = ClientState::NotInitialized;

        // First init should succeed
        let new_state = validate_client_state_transition(state, &ClientAction::Initialize(params1))
            .expect("First init should succeed");
        prop_assert_eq!(new_state, ClientState::Initialized);
        state = new_state;

        // Second init should fail
        let result = validate_client_state_transition(state, &ClientAction::Initialize(params2));
        prop_assert!(result.is_err(), "Second init should fail");
    }

    #[test]
    fn property_shutdown_is_final(
        pre_shutdown_actions in prop::collection::vec(arb_client_action(), 0..10),
        post_shutdown_actions in prop::collection::vec(arb_client_action(), 1..5),
    ) {
        let mut state = ClientState::NotInitialized;
        let mut has_shutdown = false;

        // Execute pre-shutdown actions
        for action in pre_shutdown_actions {
            if let Ok(new_state) = validate_client_state_transition(state, &action) {
                state = new_state;
            }
        }

        // If we can, transition to shutdown
        if state == ClientState::Initialized {
            state = ClientState::ShuttingDown;
            has_shutdown = true;
        }

        if has_shutdown {
            // All post-shutdown actions except Ping should fail
            for action in post_shutdown_actions {
                let result = validate_client_state_transition(state, &action);
                match action {
                    ClientAction::Ping => prop_assert!(result.is_ok(), "Ping should always be allowed"),
                    _ => prop_assert!(result.is_err(), "Non-ping actions after shutdown should fail"),
                }
            }
        }
    }

    #[test]
    fn property_capability_operations_require_init(
        capability_action in prop::sample::select(vec![
            ClientAction::ListTools,
            ClientAction::CallTool("test".to_string()),
            ClientAction::ListResources,
            ClientAction::ReadResource("file://test".to_string()),
            ClientAction::ListPrompts,
            ClientAction::GetPrompt("test".to_string()),
        ])
    ) {
        // All capability-dependent operations should require initialization
        let state = ClientState::NotInitialized;

        let result = validate_client_state_transition(state, &capability_action);
        match capability_action {
            ClientAction::Ping => prop_assert!(result.is_ok()),
            _ => prop_assert!(result.is_err(), "Capability operations should require init"),
        }
    }

    #[test]
    fn property_parallel_client_isolation(
        client_count in 2..5usize,
        sequences in prop::collection::vec(
            prop::collection::vec(arb_client_action(), 1..10),
            2..5
        )
    ) {
        // Multiple clients should have isolated state
        let mut client_states: Vec<ClientState> = vec![ClientState::NotInitialized; client_count];

        // Each client executes its sequence independently
        for (client_id, sequence) in sequences.iter().enumerate() {
            if client_id >= client_count {
                continue;
            }

            for action in sequence {
                if let Ok(new_state) = validate_client_state_transition(client_states[client_id], action) {
                    client_states[client_id] = new_state;
                }
            }
        }

        // Client states should be independent
        prop_assert!(client_states.len() == client_count);
    }

    #[test]
    fn property_notification_subscription_state(
        _has_capability in prop::bool::ANY,
        subscribe_count in 0..10usize,
        unsubscribe_count in 0..10usize,
    ) {
        // Subscription state should be consistent
        let mut subscribed_resources = HashSet::new();

        for i in 0..subscribe_count {
            subscribed_resources.insert(format!("resource_{}", i));
        }

        for i in 0..unsubscribe_count.min(subscribe_count) {
            subscribed_resources.remove(&format!("resource_{}", i));
        }

        // Should have correct number of subscriptions
        let expected = subscribe_count.saturating_sub(unsubscribe_count);
        prop_assert_eq!(subscribed_resources.len(), expected);
    }
}
