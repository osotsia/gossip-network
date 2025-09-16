//! tests/integration/network.rs
//!
//! Full end-to-end integration test for the gossip network. This test spins
//! up multiple real nodes, connects them, and verifies that state is correctly
//! propagated through the gossip protocol.

use futures::{SinkExt, StreamExt};
use gossip_network::{domain::NetworkState, App};
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

mod common;

async fn connect_with_retry(
    ws_url: String,
    max_retries: u32,
    delay: Duration,
) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    let mut retries = 0;
    loop {
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((socket, _response)) => return socket,
            Err(e) => {
                retries += 1;
                if retries > max_retries {
                    panic!(
                        "Failed to connect to WebSocket at {} after {} retries: {}",
                        ws_url, max_retries, e
                    );
                }
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_state_propagation_across_two_nodes() {
    // --- 1. Setup ---
    // Create temporary directories for each node's state.
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    // Create a master shutdown token for the test.
    let shutdown_token = CancellationToken::new();

    // --- 2. Configure and Spawn Node A (The Bootstrap Node) ---
    let config_a = common::setup_test_config(&dir_a, vec![]);
    // Pre-bind a listener to an ephemeral port to discover the address.
    // This mitigates a race condition where the port could be grabbed by another process.
    let listener_a = std::net::TcpListener::bind(config_a.p2p_addr).unwrap();
    let addr_a = listener_a.local_addr().unwrap();
    drop(listener_a); // Drop the listener so the app can bind to it.

    let mut config_a_with_addr = config_a.clone();
    config_a_with_addr.p2p_addr = addr_a;

    let app_a = App::new(config_a_with_addr).unwrap();
    let shutdown_a = shutdown_token.clone();
    tokio::spawn(async move {
        let _ = app_a.run().await;
        shutdown_a.cancel(); // Signal shutdown if it exits prematurely.
    });

    // --- 3. Configure and Spawn Node B (Connects to Node A) ---
    // Node B uses Node A's address as its bootstrap peer.
    let config_b = common::setup_test_config(&dir_b, vec![addr_a]);
    let listener_b = std::net::TcpListener::bind(config_b.visualizer.unwrap().bind_addr).unwrap();
    let viz_addr_b = listener_b.local_addr().unwrap(); // Get visualizer address
    drop(listener_b);

    let mut config_b_with_addr = config_b.clone();
    config_b_with_addr.visualizer.as_mut().unwrap().bind_addr = viz_addr_b;

    let app_b = App::new(config_b_with_addr).unwrap();
    let shutdown_b = shutdown_token.clone();
    tokio::spawn(async move {
        let _ = app_b.run().await;
        shutdown_b.cancel();
    });

    // --- 4. Connect to Node B's Visualizer WebSocket with Retry ---
    let ws_url = format!("ws://{}", viz_addr_b);
    let mut ws_client = connect_with_retry(ws_url, 20, Duration::from_millis(100)).await;

    // --- 5. Verify State Propagation ---
    let test_timeout = Duration::from_secs(5);
    let result = timeout(test_timeout, async {
        loop {
            let msg = ws_client.next().await.unwrap().unwrap();
            if let Message::Text(text) = msg {
                let state: NetworkState = serde_json::from_str(&text).unwrap();
                // We are waiting for Node B's state to contain info about Node A.
                // There will be 2 nodes total (A and B).
                if state.nodes.len() == 2 {
                    println!("SUCCESS: Node B received state from Node A.");
                    return;
                }
            }
        }
    })
    .await;

    // --- 6. Cleanup ---
    shutdown_token.cancel();
    // Give a moment for graceful shutdown.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Assert that the test did not time out.
    assert!(
        result.is_ok(),
        "Test timed out: Node B did not receive Node A's state."
    );
}