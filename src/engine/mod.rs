//! src/engine/mod.rs
//!
//! Defines the `Engine`, the core application logic service. It maintains
//! network state, generates telemetry, and applies the gossip protocol.

use crate::{
    config::Config,
    domain::{GossipPayload, Identity, NetworkState, NodeId, NodeInfo, SignedMessage, TelemetryData},
    // MODIFICATION: Import ConnectionEvent
    transport::{ConnectionEvent, InboundMessage, TransportCommand},
};
use std::{
    // MODIFICATION: Import HashSet
    collections::{HashMap, HashSet},
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, mpsc, watch}; // MODIFICATION: Import broadcast
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
    // NEW: State for tracking active P2P connections reported by Transport.
    active_peer_addrs: HashSet<SocketAddr>,
    inbound_rx: mpsc::Receiver<InboundMessage>,
    // NEW: Receiver for connection events.
    conn_event_rx: mpsc::Receiver<ConnectionEvent>,
    transport_tx: mpsc::Sender<TransportCommand>,
    state_tx: watch::Sender<NetworkState>,
    // NEW: Sender for animation events.
    animation_tx: broadcast::Sender<NodeId>,
}

impl Engine {
    pub fn new(
        identity: Identity,
        config: Config,
        inbound_rx: mpsc::Receiver<InboundMessage>,
        conn_event_rx: mpsc::Receiver<ConnectionEvent>,
        transport_tx: mpsc::Sender<TransportCommand>,
        state_tx: watch::Sender<NetworkState>,
        // NEW: Accept animation event sender.
        animation_tx: broadcast::Sender<NodeId>,
    ) -> Self {
        Self {
            gossip_interval: Duration::from_millis(config.gossip_interval_ms),
            node_ttl: Duration::from_millis(config.node_ttl_ms),
            identity,
            config,
            node_info: HashMap::new(),
            known_peers: HashMap::new(),
            active_peer_addrs: HashSet::new(),
            inbound_rx,
            conn_event_rx,
            transport_tx,
            state_tx,
            animation_tx,
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
                },
                Some(event) = self.conn_event_rx.recv() => {
                    self.handle_connection_event(event);
                }
                else => {
                    tracing::info!("Channel closed. Engine service shutting down.");
                    break;
                }
            }
        }
    }

    fn handle_connection_event(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::PeerConnected { peer_addr } => {
                if self.active_peer_addrs.insert(peer_addr) {
                    tracing::debug!(peer_addr = %peer_addr, "Peer connection established");
                    self.publish_state();
                }
            }
            ConnectionEvent::PeerDisconnected { peer_addr } => {
                if self.active_peer_addrs.remove(&peer_addr) {
                    tracing::debug!(peer_addr = %peer_addr, "Peer connection lost");
                    self.publish_state();
                }
            }
        }
    }

    async fn handle_inbound_message(&mut self, inbound: InboundMessage) {
        if let Err(e) = inbound.message.verify() {
            tracing::warn!(error = %e, "Received message with invalid signature. Discarding.");
            return;
        }

        // Before checking for newness, find the NodeId of the immediate peer who sent this message.
        // This requires a reverse lookup in our `known_peers` map.
        let peer_node_id = self
            .known_peers
            .iter()
            .find(|(_, &addr)| addr == inbound.peer_addr)
            .map(|(id, _)| *id);

        // Update the known peer's address. This is crucial for the reverse lookup above.
        self.known_peers
            .insert(inbound.message.originator, inbound.peer_addr);

        let is_new = match self.node_info.get(&inbound.message.originator) {
            Some(existing) => {
                inbound.message.message.telemetry.timestamp_ms
                    > existing.telemetry.timestamp_ms
            }
            None => true,
        };

        if is_new {
            tracing::info!(originator = %inbound.message.originator, "Received new information");
            let node_info = NodeInfo {
                telemetry: inbound.message.message.telemetry.clone(),
                community_id: inbound.message.message.community_id,
            };
            self.node_info
                .insert(inbound.message.originator, node_info);
            
            // NEW: If we found the peer's NodeId, send an animation event.
            if let Some(id) = peer_node_id {
                if self.animation_tx.send(id).is_err() {
                    tracing::trace!(peer_id = %id, "No active API listeners for animation event.");
                } else {
                    tracing::debug!(peer_id = %id, "Sent animation event for incoming gossip.");
                }
            }

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
        tracing::debug!("Generated new telemetry. Gossiping to peers...");

        let node_info = NodeInfo {
            telemetry: signed_message.message.telemetry.clone(),
            community_id: signed_message.message.community_id,
        };
        self.node_info
            .insert(self.identity.node_id, node_info);

        self.publish_state();

        self.gossip_to_peers(signed_message.clone()).await;

        for &addr in &self.config.bootstrap_peers {
            let command = TransportCommand::SendMessage(addr, signed_message.clone());
            if let Err(e) = self.transport_tx.send(command).await {
                tracing::error!(error = %e, "Failed to send command to transport service for bootstrap peer");
            }
        }
    }

    async fn gossip_to_peers(&self, message: SignedMessage) {
        let peers_to_gossip_to = protocol::select_peers(
            &self.known_peers,
            message.originator,
            self.config.gossip_factor,
        );

        if peers_to_gossip_to.is_empty() {
            tracing::debug!("No known peers to gossip to yet.");
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
        let active_connections = self
            .known_peers
            .iter()
            .filter(|(_, &addr)| self.active_peer_addrs.contains(&addr))
            .map(|(id, _)| *id)
            .collect();

        let state = NetworkState {
            self_id: Some(self.identity.node_id),
            nodes: self.node_info.clone(),
            active_connections,
        };

        if let Ok(json_state) = serde_json::to_string(&state) {
            tracing::debug!(payload = %json_state, "Publishing state update to API");
        }
        let _ = self.state_tx.send(state);
    }
}