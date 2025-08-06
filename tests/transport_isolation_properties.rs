//! Property tests for transport isolation functionality.

use pmcp::shared::protocol::{Protocol, ProtocolOptions, TransportId};
use pmcp::types::{JSONRPCResponse, RequestId};
use proptest::prelude::*;
use std::collections::HashSet;

/// Generate arbitrary request IDs.
fn arb_request_id() -> impl Strategy<Value = RequestId> {
    prop_oneof![
        (0i64..1000).prop_map(RequestId::from),
        "[a-z0-9]{8}".prop_map(RequestId::from),
    ]
}

/// Generate arbitrary transport IDs.
fn arb_transport_id() -> impl Strategy<Value = TransportId> {
    "[a-z0-9]{16}".prop_map(TransportId::from_string)
}

proptest! {
    /// Test that transport IDs are unique.
    #[test]
    fn prop_transport_ids_are_unique(
        _iterations in 0..100usize
    ) {
        let mut ids = HashSet::new();
        for _ in 0..100 {
            let id = TransportId::new();
            prop_assert!(!ids.contains(&id), "Transport ID collision detected");
            ids.insert(id);
        }
    }

    /// Test that requests from different transports are isolated.
    #[test]
    fn prop_transport_isolation(
        request_ids in prop::collection::hash_set(arb_request_id(), 1..10),
        transport_id1 in arb_transport_id(),
        transport_id2 in arb_transport_id(),
    ) {
        // Skip if transport IDs are the same
        prop_assume!(transport_id1 != transport_id2);

        // Convert to vec for iteration
        let request_ids: Vec<_> = request_ids.into_iter().collect();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create two protocols with different transport IDs
            let mut protocol1 = Protocol::with_transport_id(
                ProtocolOptions::default(),
                transport_id1.clone()
            );
            let mut protocol2 = Protocol::with_transport_id(
                ProtocolOptions::default(),
                transport_id2.clone()
            );

            // Register requests from both transports
            let mut receivers1 = Vec::new();
            let mut receivers2 = Vec::new();

            for id in &request_ids {
                receivers1.push(protocol1.register_request(id.clone()));
                receivers2.push(protocol2.register_request(id.clone()));
            }

            // Complete requests for protocol1
            for id in &request_ids {
                let response = JSONRPCResponse::success(
                    id.clone(),
                    serde_json::json!("from_transport1")
                );
                let completed = protocol1.complete_request_for_transport(
                    id,
                    response,
                    &transport_id1
                ).unwrap();
                prop_assert!(completed, "Request should be completed for matching transport");
            }

            // Try to complete requests for protocol2 with wrong transport ID
            for id in &request_ids {
                let response = JSONRPCResponse::success(
                    id.clone(),
                    serde_json::json!("from_wrong_transport")
                );
                let completed = protocol2.complete_request_for_transport(
                    id,
                    response,
                    &transport_id1  // Wrong transport ID
                ).unwrap();
                prop_assert!(!completed, "Request should not be completed for wrong transport");
            }

            // Complete requests for protocol2 with correct transport ID
            for id in &request_ids {
                let response = JSONRPCResponse::success(
                    id.clone(),
                    serde_json::json!("from_transport2")
                );
                let completed = protocol2.complete_request_for_transport(
                    id,
                    response,
                    &transport_id2
                ).unwrap();
                prop_assert!(completed, "Request should be completed for matching transport");
            }

            // Give async tasks time to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Verify responses went to correct receivers
            let expected1 = serde_json::json!("from_transport1");
            for mut rx in receivers1 {
                match rx.try_recv() {
                    Ok(response) => {
                        prop_assert_eq!(
                            response.result(),
                            Some(&expected1)
                        );
                    }
                    Err(_) => prop_assert!(false, "Transport1 should have received response"),
                }
            }

            let expected2 = serde_json::json!("from_transport2");
            for mut rx in receivers2 {
                match rx.try_recv() {
                    Ok(response) => {
                        prop_assert_eq!(
                            response.result(),
                            Some(&expected2)
                        );
                    }
                    Err(_) => prop_assert!(false, "Transport2 should have received response"),
                }
            }
            Ok(())
        })?;
    }

    /// Test that the same request ID can be used by different transports.
    #[test]
    fn prop_same_request_id_different_transports(
        request_id in arb_request_id(),
        num_transports in 2..5usize,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut protocols = Vec::new();
            let mut transport_ids = Vec::new();
            let mut receivers = Vec::new();

            // Create multiple protocols with different transport IDs
            for i in 0..num_transports {
                let transport_id = TransportId::from_string(format!("transport_{}", i));
                let mut protocol = Protocol::with_transport_id(
                    ProtocolOptions::default(),
                    transport_id.clone()
                );

                // Register the same request ID on each transport
                let rx = protocol.register_request(request_id.clone());
                receivers.push(rx);

                transport_ids.push(transport_id);
                protocols.push(protocol);
            }

            // Complete the request for each transport
            for (i, (protocol, transport_id)) in protocols.iter_mut()
                .zip(transport_ids.iter())
                .enumerate()
            {
                let response = JSONRPCResponse::success(
                    request_id.clone(),
                    serde_json::json!(format!("response_{}", i))
                );

                let completed = protocol.complete_request_for_transport(
                    &request_id,
                    response,
                    transport_id
                ).unwrap();

                prop_assert!(completed, "Request should be completed for transport {}", i);
            }

            // Give async tasks time to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Verify each transport got its own response
            for (i, mut rx) in receivers.into_iter().enumerate() {
                match rx.try_recv() {
                    Ok(response) => {
                        let expected = serde_json::json!(format!("response_{}", i));
                        prop_assert_eq!(
                            response.result(),
                            Some(&expected)
                        );
                    }
                    Err(_) => prop_assert!(false, "Transport {} should have received response", i),
                }
            }
            Ok(())
        })?;
    }

    /// Test protocol options are preserved across transport instances.
    #[test]
    fn prop_protocol_options_preserved(
        enforce_strict in any::<bool>(),
        methods in prop::collection::vec("[a-z]+", 0..5),
    ) {
        let options = ProtocolOptions {
            enforce_strict_capabilities: enforce_strict,
            debounced_notification_methods: methods.clone(),
        };

        let protocol = Protocol::new(options.clone());
        prop_assert_eq!(
            protocol.options().enforce_strict_capabilities,
            enforce_strict
        );
        prop_assert_eq!(
            &protocol.options().debounced_notification_methods,
            &methods
        );

        // Test with specific transport ID
        let transport_id = TransportId::new();
        let protocol2 = Protocol::with_transport_id(options.clone(), transport_id);
        prop_assert_eq!(
            protocol2.options().enforce_strict_capabilities,
            enforce_strict
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_transport_operations() {
        let mut handles = Vec::new();

        // Spawn multiple transport simulations
        for i in 0..10 {
            let handle = tokio::spawn(async move {
                let transport_id = TransportId::from_string(format!("transport_{}", i));
                let mut protocol =
                    Protocol::with_transport_id(ProtocolOptions::default(), transport_id.clone());

                // Register and complete multiple requests
                for j in 0..5 {
                    let request_id = RequestId::from(format!("req_{}_{}", i, j));
                    let rx = protocol.register_request(request_id.clone());

                    let response = JSONRPCResponse::success(
                        request_id.clone(),
                        serde_json::json!({"transport": i, "request": j}),
                    );

                    protocol.complete_request(&request_id, response).unwrap();

                    // Small delay to simulate real operations
                    tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

                    // Verify response
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    let mut rx = rx;
                    match rx.try_recv() {
                        Ok(resp) => {
                            assert_eq!(
                                resp.result(),
                                Some(&serde_json::json!({"transport": i, "request": j}))
                            );
                        },
                        Err(e) => panic!("Failed to receive response for transport {}: {:?}", i, e),
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all transports to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }
}
