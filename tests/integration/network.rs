//! tests/integration/network.rs
//!
//! Full end-to-end integration test for the gossip network. This test spins
//! up multiple real nodes, connects them, and verifies that state is correctly
//! propagated through the gossip protocol on the "happy path".

use crate::common::harness::{self, TestNode};
use std::time::Duration;
use test_log::test;

#[test(tokio::test(flavor = "multi_thread", worker_threads = 4))]
async fn test_state_propagation_across_two_nodes() {
    let test_timeout = Duration::from_secs(10);
    let result = tokio::time::timeout(test_timeout, async {
        // --- 1. Setup ---
        // Generate a single trusted Certificate Authority for this test network.
        let trusted_certs = harness::generate_certs("localhost");

        // --- 2. Spawn Node A (The Bootstrap Node) ---
        let node_a = TestNode::spawn(vec![], &trusted_certs)
            .await
            .expect("Failed to spawn node A");

        // --- 3. Spawn Node B (Connects to Node A) ---
        let node_b = TestNode::spawn(vec![node_a.p2p_addr], &trusted_certs)
            .await
            .expect("Failed to spawn node B");

        // --- 4. Connect a WebSocket client to Node B to observe its state ---
        let mut ws_client_b = node_b.ws_client().await.expect("Failed to connect ws client to B");

        // --- 5. Verify State Propagation ---
        // Wait for Node B's state to contain 2 nodes (itself and Node A).
        let final_state = harness::wait_for_state(
            &mut ws_client_b,
            |state| state.nodes.len() == 2,
            Duration::from_secs(5),
        ).await.expect("Failed to observe state propagation");

        assert_eq!(final_state.nodes.len(), 2, "Node B should know about 2 nodes");

        // --- 6. Cleanup ---
        node_a.shutdown();
        node_b.shutdown();
    }).await;

    assert!(result.is_ok(), "Test timed out");
}