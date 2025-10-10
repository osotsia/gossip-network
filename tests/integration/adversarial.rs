//! tests/component/engine.rs
//!
//! In-memory component tests for the `Engine` service.

use gossip_network::{
    config::Config,
    domain::{GossipPayload, Identity, NetworkState, SignedMessage, TelemetryData},
    engine::Engine,
    transport::{ConnectionEvent, InboundMessage, TransportCommand},
};
use std::{net::SocketAddr, time::Duration};
use test_log::test;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time;

struct EngineHarness {
    _identity: Identity,
    transport_rx: mpsc::Receiver<TransportCommand>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    _conn_event_tx: mpsc::Sender<ConnectionEvent>,
    state_rx: watch::Receiver<NetworkState>,
    shutdown_token: tokio_util::sync::CancellationToken,
}

/// Helper function to wait for the engine's state to meet a condition.
async fn wait_for_state_change<F>(harness: &mut EngineHarness, predicate: F)
where
    F: Fn(&NetworkState) -> bool,
{
    time::timeout(Duration::from_secs(1), async {
        loop {
            if predicate(&harness.state_rx.borrow()) {
                return;
            }
            harness.state_rx.changed().await.unwrap();
        }
    })
    .await
    .expect("Timeout waiting for state change");
}

fn setup_engine_harness(config: Config) -> EngineHarness {
    let identity = Identity::from_file(config.identity_path.clone()).unwrap();
    let (transport_tx, transport_rx) = mpsc::channel(10);
    let (inbound_tx, inbound_rx) = mpsc::channel(10);
    let (state_tx, state_rx) = watch::channel(NetworkState::default());
    let (conn_event_tx, conn_event_rx) = mpsc::channel(10);
    let (animation_tx, _) = broadcast::channel(10);

    let engine = Engine::new(
        identity.clone(),
        config,
        inbound_rx,
        conn_event_rx,
        transport_tx,
        state_tx,
        animation_tx,
    );

    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let engine_token = shutdown_token.clone();
    tokio::spawn(engine.run(engine_token));

    EngineHarness {
        _identity: identity,
        transport_rx,
        inbound_tx,
        _conn_event_tx: conn_event_tx,
        state_rx,
        shutdown_token,
    }
}

fn create_test_message(identity: &Identity, timestamp_ms: u64) -> SignedMessage {
    identity.sign(GossipPayload {
        telemetry: TelemetryData { timestamp_ms, value: 42.0 },
        community_id: 1,
    })
}

#[test(tokio::test)]
async fn test_engine_prunes_stale_nodes_from_all_maps() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config {
        identity_path: temp_dir.path().join("id.key"),
        node_ttl_ms: 1000,
        ..Config::default()
    };
    let mut harness = setup_engine_harness(config);
    time::pause();

    let peer_identity = Identity::new();
    let peer_addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
    let message = create_test_message(&peer_identity, time::Instant::now().elapsed().as_millis() as u64);

    harness.inbound_tx.send(InboundMessage { peer_addr, message }).await.unwrap();

    wait_for_state_change(&mut harness, |state| state.nodes.len() == 1).await;
    // MODIFICATION: Introduce a scope to limit the lifetime of the `state` borrow.
    {
        let state = harness.state_rx.borrow();
        assert!(state.nodes.contains_key(&peer_identity.node_id), "Peer should be added to state");
    } // `state` is dropped here, releasing the immutable borrow.

    harness.inbound_tx.send(InboundMessage {
        peer_addr: "127.0.0.1:9999".parse().unwrap(),
        message: create_test_message(&Identity::new(), 0)
    }).await.unwrap();
    assert!(harness.transport_rx.try_recv().is_ok(), "Engine should know peer address to gossip");

    time::advance(Duration::from_secs(62)).await;

    wait_for_state_change(&mut harness, |state| state.nodes.is_empty()).await;
    let final_state = harness.state_rx.borrow();
    assert!(final_state.nodes.is_empty(), "Stale peer should be pruned from node_info");
    
    harness.inbound_tx.send(InboundMessage {
        peer_addr: "127.0.0.1:9999".parse().unwrap(),
        message: create_test_message(&Identity::new(), 0)
    }).await.unwrap();
    assert!(harness.transport_rx.try_recv().is_err(), "Engine should not gossip to a pruned peer");

    harness.shutdown_token.cancel();
}

#[test(tokio::test)]
async fn test_engine_state_freeze_via_timestamp_attack() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config { identity_path: temp_dir.path().join("id.key"), ..Default::default() };
    let mut harness = setup_engine_harness(config);

    let attacker_identity = Identity::new();
    let attacker_addr: SocketAddr = "127.0.0.1:6666".parse().unwrap();

    let future_message = create_test_message(&attacker_identity, u64::MAX);
    harness.inbound_tx.send(InboundMessage { peer_addr: attacker_addr, message: future_message }).await.unwrap();

    wait_for_state_change(&mut harness, |state| !state.nodes.is_empty()).await;
    {
        let state = harness.state_rx.borrow();
        assert_eq!(state.nodes.get(&attacker_identity.node_id).unwrap().telemetry.timestamp_ms, u64::MAX);
    }
    
    let valid_message = create_test_message(&attacker_identity, 1000);
    harness.inbound_tx.send(InboundMessage { peer_addr: attacker_addr, message: valid_message }).await.unwrap();
    time::sleep(Duration::from_millis(10)).await;

    let final_state = harness.state_rx.borrow().clone();
    assert_eq!(final_state.nodes.get(&attacker_identity.node_id).unwrap().telemetry.timestamp_ms, u64::MAX,
        "Engine should reject the new message as it is older than the future-dated one");

    harness.shutdown_token.cancel();
}

#[test(tokio::test)]
async fn test_engine_routing_table_poisoning() {
    time::pause();
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config { identity_path: temp_dir.path().join("id.key"), ..Default::default() };
    let mut harness = setup_engine_harness(config);

    let honest_peer_id = Identity::new();
    let malicious_peer_addr: SocketAddr = "127.0.0.1:6666".parse().unwrap();

    let message_from_a = create_test_message(&honest_peer_id, 1000);
    harness.inbound_tx.send(InboundMessage {
        peer_addr: malicious_peer_addr,
        message: message_from_a,
    }).await.unwrap();
    time::sleep(Duration::from_millis(10)).await;

    let another_peer_id = Identity::new();
    let another_peer_addr: SocketAddr = "127.0.0.1:7777".parse().unwrap();
    let trigger_message = create_test_message(&another_peer_id, 2000);

    harness.inbound_tx.send(InboundMessage {
        peer_addr: another_peer_addr,
        message: trigger_message,
    }).await.unwrap();

    let command = time::timeout(Duration::from_secs(1), harness.transport_rx.recv()).await
        .expect("Engine should have sent a gossip command")
        .unwrap();

    let TransportCommand::SendMessage(addr, msg) = command;
    assert_eq!(addr, malicious_peer_addr, "Address should be the malicious peer's address");
    assert_eq!(msg.originator, another_peer_id.node_id, "Message should be the trigger message");
    
    harness.shutdown_token.cancel();
}