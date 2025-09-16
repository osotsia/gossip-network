//! src/engine/mod.rs
//!
//! Defines the `Engine`, the core application logic service. It maintains
//! network state, generates telemetry, and applies the gossip protocol.

use crate::{
    config::Config,
    domain::{GossipPayload, Identity, NetworkState, NodeInfo, SignedMessage, TelemetryData},
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
    config: Config,
    gossip_interval: Duration,
    node_ttl: Duration,
    // The canonical state of the network from this node's perspective.
    node_info: HashMap<crate::domain::NodeId, NodeInfo>,
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
            gossip_interval: Duration::from_millis(config.gossip_interval_ms),
            node_ttl: Duration::from_millis(config.node_ttl_ms),
            identity,
            config,
            node_info: HashMap::new(),
            known_peers: HashMap::new(),
            inbound_rx,
            transport_tx,
            state_tx,
        }
    }

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
            tracing::warn!(error = %e, "Received message with invalid signature. Discarding.");
            return;
        }

        self.known_peers.insert(inbound.message.originator, inbound.peer_addr);

        let is_new = match self.node_info.get(&inbound.message.originator) {
            Some(existing) => inbound.message.message.telemetry.timestamp_ms > existing.telemetry.timestamp_ms,
            None => true,
        };

        if is_new {
            tracing::info!(originator = %inbound.message.originator, "Received new information");
            let node_info = NodeInfo {
                telemetry: inbound.message.message.telemetry.clone(),
                community_id: inbound.message.message.community_id,
            };
            self.node_info.insert(inbound.message.originator, node_info);

            self.publish_state();
            self.gossip_to_peers(inbound.message).await;
        }
    }

    async fn gossip_self_telemetry(&mut self) {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        let payload = GossipPayload {
            telemetry: TelemetryData {
                timestamp_ms,
                value: 100.0 + 50.0 * (timestamp_ms as f64 / 10000.0).sin(),
            },
            community_id: self.config.community_id,
        };

        let signed_message = self.identity.sign(payload);
        tracing::info!("Generated new telemetry. Gossiping to peers...");

        let node_info = NodeInfo {
            telemetry: signed_message.message.telemetry.clone(),
            community_id: signed_message.message.community_id,
        };
        self.node_info.insert(self.identity.node_id, node_info);

        self.publish_state();
        self.gossip_to_peers(signed_message).await;
    }

    async fn gossip_to_peers(&self, message: SignedMessage) {
        let peers_to_gossip_to =
            protocol::select_peers(&self.known_peers, message.originator, self.config.gossip_factor);

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
            .node_info
            .iter()
            .filter(|(id, data)| {
                **id != self.identity.node_id && (now_ms - data.telemetry.timestamp_ms) > ttl_ms
            })
            .map(|(id, _)| *id)
            .collect();

        if !stale_nodes.is_empty() {
            tracing::info!(count = stale_nodes.len(), "Pruning stale nodes");
            for node_id in stale_nodes {
                self.node_info.remove(&node_id);
                self.known_peers.remove(&node_id);
            }
            self.publish_state();
        }
    }

    fn publish_state(&self) {
        // FIX: The `edges` field should represent the node's current set of
        // known peers, not the static bootstrap list from the configuration.
        // This provides an accurate view of the node's connections for the visualizer.
        let current_edges: Vec<_> = self.known_peers.keys().cloned().collect();
        
        let state = NetworkState {
            self_id: Some(self.identity.node_id),
            nodes: self.node_info.clone(),
            edges: current_edges,
        };
        
        // The send operation remains the same.
        let _ = self.state_tx.send(state);
    }
}