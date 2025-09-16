//! src/engine/mod.rs
//!
//! Defines the `Engine`, the core application logic service. It maintains
//! network state, generates telemetry, and applies the gossip protocol.

use crate::{
    config::Config,
    domain::{Identity, NetworkState, SignedMessage, TelemetryData},
    transport::{InboundMessage, TransportCommand},
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, watch};
use tokio::time;
use tokio_util::sync::CancellationToken;

pub mod protocol;

/// The core application logic actor.
pub struct Engine {
    identity: Identity,
    gossip_interval: Duration,
    gossip_factor: usize,
    node_ttl: Duration,
    network_state: NetworkState,
    known_peers: HashMap<crate::domain::NodeId, SocketAddr>,
    inbound_rx: mpsc::Receiver<InboundMessage>,
    transport_tx: mpsc::Sender<TransportCommand>,
    state_tx: watch::Sender<NetworkState>,
}

impl Engine {
    pub fn new(
        identity: Identity,
        config: Config,
        inbound_rx: mpsc::Receiver<InboundMessage>,
        transport_tx: mpsc::Sender<TransportCommand>,
        state_tx: watch::Sender<NetworkState>,
    ) -> Self {
        Self {
            identity,
            gossip_interval: Duration::from_millis(config.gossip_interval_ms),
            gossip_factor: config.gossip_factor,
            node_ttl: Duration::from_millis(config.node_ttl_ms),
            network_state: NetworkState::default(),
            known_peers: HashMap::new(),
            inbound_rx,
            transport_tx,
            state_tx,
        }
    }

    /// The main run loop for the `Engine` service.
    pub async fn run(mut self, shutdown_token: CancellationToken) {
        tracing::info!(node_id = %self.identity.node_id, "Engine service started");
        let mut gossip_timer = time::interval(self.gossip_interval);
        let mut cleanup_timer = time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Engine service received shutdown signal.");
                    break;
                },
                _ = gossip_timer.tick() => {
                    self.gossip_self_telemetry().await;
                },
                _ = cleanup_timer.tick() => {
                    self.cleanup_stale_nodes();
                },
                Some(inbound) = self.inbound_rx.recv() => {
                    self.handle_inbound_message(inbound).await;
                }
                else => {
                    tracing::info!("Channel closed. Engine service shutting down.");
                    break;
                }
            }
        }
    }

    async fn handle_inbound_message(&mut self, inbound: InboundMessage) {
        if let Err(e) = inbound.message.verify() {
            tracing::warn!(
                from = %inbound.peer_addr,
                error = %e,
                "Received message with invalid signature. Discarding."
            );
            return;
        }

        self.known_peers
            .insert(inbound.message.originator, inbound.peer_addr);

        let is_new = match self.network_state.nodes.get(&inbound.message.originator) {
            Some(existing) => inbound.message.message.timestamp_ms > existing.timestamp_ms,
            None => true,
        };

        if is_new {
            tracing::info!(originator = %inbound.message.originator, "Received new information");
            self.network_state.nodes.insert(
                inbound.message.originator,
                inbound.message.message.clone(),
            );
            self.publish_state();
            self.gossip_to_peers(inbound.message).await;
        }
    }

    async fn gossip_self_telemetry(&mut self) {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        let telemetry = TelemetryData {
            timestamp_ms,
            value: 100.0 + 50.0 * (timestamp_ms as f64 / 10000.0).sin(),
        };

        let signed_message = self.identity.sign(telemetry);
        tracing::info!("Generated new telemetry. Gossiping to peers...");

        self.network_state.nodes.insert(
            self.identity.node_id,
            signed_message.message.clone(),
        );
        self.publish_state();
        self.gossip_to_peers(signed_message).await;
    }

    async fn gossip_to_peers(&self, message: SignedMessage) {
        let peers_to_gossip_to =
            protocol::select_peers(&self.known_peers, message.originator, self.gossip_factor);

        if peers_to_gossip_to.is_empty() {
            tracing::warn!("No peers to gossip to. Is the network empty?");
            return;
        }

        for (node_id, addr) in peers_to_gossip_to {
            tracing::debug!(peer_id = %node_id, peer_addr = %addr, "Gossiping message");
            let command = TransportCommand::SendMessage(*addr, message.clone());
            if let Err(e) = self.transport_tx.send(command).await {
                tracing::error!(error = %e, "Failed to send command to transport service");
            }
        }
    }

    fn cleanup_stale_nodes(&mut self) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;
        let ttl_ms = self.node_ttl.as_millis() as u64;

        let stale_nodes: Vec<_> = self
            .network_state
            .nodes
            .iter()
            .filter(|(id, data)| {
                // Do not remove self
                **id != self.identity.node_id && (now_ms - data.timestamp_ms) > ttl_ms
            })
            .map(|(id, _)| *id)
            .collect();

        if !stale_nodes.is_empty() {
            tracing::info!(count = stale_nodes.len(), "Pruning stale nodes");
            for node_id in stale_nodes {
                self.network_state.nodes.remove(&node_id);
                self.known_peers.remove(&node_id);
            }
            self.publish_state();
        }
    }

    fn publish_state(&self) {
        let _ = self.state_tx.send(self.network_state.clone());
    }
}