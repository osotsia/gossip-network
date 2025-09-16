//! src/gossip.rs
//!
//! The Town Crier of our system.
//!
//! This module contains the core application logic. The `GossipEngine` actor is
//! responsible for:
//!   - Maintaining the current state of the network (a map of nodes and their latest telemetry).
//!   - Periodically generating its own telemetry data to share.
//!   - Processing incoming messages from the `P2PManager`: validating their signatures
//!     and checking if they represent new information.
//!   - Implementing the gossip protocol: deciding which peers to forward new information to.
//!   - Publishing state updates for the visualizer.

use crate::{
    crypto::{self, Identity},
    model::{NodeId, SignedMessage, TelemetryData},
    p2p::{Command as P2pCommand, InboundMessage},
};
use rand::{seq::SliceRandom, thread_rng};
use std::{collections::HashMap, net::SocketAddr, time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio::sync::{mpsc, watch};
use tokio::time;
use tokio_util::sync::CancellationToken;


/// A simplified representation of the network state for the visualizer.
/// It is designed to be easily serialized to JSON.
// NOTE: We derive a default here for initial state creation
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct NetworkState {
    pub nodes: HashMap<NodeId, TelemetryData>,
    // In a more complex scenario, you would also track peer connections here.
}

/// The GossipEngine actor. It owns the application state and drives the protocol.
pub struct GossipEngine {
    /// This node's cryptographic identity.
    identity: Identity,
    /// The application configuration.
    gossip_interval: Duration,
    /// A map of all known nodes and their latest telemetry data. This is the "state".
    network_state: NetworkState,
    /// A map tracking which peers we know at which network addresses.
    known_peers: HashMap<NodeId, SocketAddr>,
    /// A channel to receive inbound messages from the `P2PManager`.
    inbound_rx: mpsc::Receiver<InboundMessage>,
    /// A channel to send commands to the `P2PManager`.
    p2p_tx: mpsc::Sender<P2pCommand>,
    /// A broadcast channel to send state updates to the `Visualizer`.
    state_tx: watch::Sender<NetworkState>,
}

impl GossipEngine {
    /// Creates a new `GossipEngine`.
    pub fn new(
        identity: Identity,
        gossip_interval_ms: u64,
        inbound_rx: mpsc::Receiver<InboundMessage>,
        p2p_tx: mpsc::Sender<P2pCommand>,
        state_tx: watch::Sender<NetworkState>,
    ) -> Self {
        Self {
            identity,
            gossip_interval: Duration::from_millis(gossip_interval_ms),
            network_state: NetworkState::default(),
            known_peers: HashMap::new(),
            inbound_rx,
            p2p_tx,
            state_tx,
        }
    }

    /// The main run loop for the `GossipEngine` actor.
    ///
    /// This is the heart of the application's logic. It uses a `tokio::select!` loop
    /// to react to two main event sources:
    ///   1. A periodic timer tick, which triggers the node to create and gossip its own data.
    ///   2. Incoming messages from other nodes, delivered by the `P2PManager`.
    ///
    /// The metaphor here is a person at a party. They periodically share their own
    /// interesting story (timer tick) but also listen for new stories from others
    /// (inbound messages), deciding which ones are new and interesting enough to repeat.
    pub async fn run(mut self, shutdown_token: CancellationToken) { // <--- ADDED TOKEN
        log::info!("Gossip Engine started for node {}", self.identity.node_id);
        let mut gossip_timer = time::interval(self.gossip_interval);

        loop {
            // --- IMPROVEMENT: Robust Shutdown ---
            tokio::select! {
                // Add a branch to listen for the shutdown signal.
                _ = shutdown_token.cancelled() => {
                    log::info!("Gossip Engine received shutdown signal.");
                    break;
                },

                // Event 1: Timer tick for gossiping our own state
                _ = gossip_timer.tick() => {
                    self.gossip_self_telemetry().await;
                },

                // Event 2: A message received from a peer
                Some(inbound) = self.inbound_rx.recv() => {
                    self.handle_inbound_message(inbound).await;
                }

                else => {
                    log::info!("Inbound message channel closed. Gossip Engine shutting down.");
                    break;
                }
            }
        }
        // The loop will break on shutdown, and the function will return cleanly.
    }

    /// Handles a single message received from the network.
    async fn handle_inbound_message(&mut self, inbound: InboundMessage) {
        // The gatekeeper: first, verify the message's cryptographic signature.
        if let Err(e) = crypto::verify(&inbound.message) {
            log::warn!(
                "Received message with invalid signature from {}. Discarding. Error: {}",
                inbound.peer_addr,
                e
            );
            return;
        }

        // Add the peer to our known list so we can gossip back to them later.
        self.known_peers
            .insert(inbound.message.originator, inbound.peer_addr);

        // Check if this message is new information.
        // We only care about telemetry with a more recent timestamp.
        let is_new_information = match self
            .network_state
            .nodes
            .get(&inbound.message.originator)
        {
            Some(existing_data) => {
                inbound.message.message.timestamp_ms > existing_data.timestamp_ms
            }
            None => true, // We've never heard from this node before.
        };

        if is_new_information {
            log::info!(
                "Received new information from node {}.",
                inbound.message.originator
            );
            // Update our local state.
            self.network_state
                .nodes
                .insert(inbound.message.originator, inbound.message.message.clone());
            
            // Publish the new state for the visualizer.
            self.publish_state();

            // Propagate the new information to our other peers.
            self.gossip_to_peers(inbound.message).await;
        }
    }

    /// Creates this node's own telemetry and gossips it to peers.
    async fn gossip_self_telemetry(&mut self) {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        let telemetry = TelemetryData {
            timestamp_ms,
            // Simulate a sine wave for more interesting visual data.
            value: 100.0 + 50.0 * (timestamp_ms as f64 / 10000.0).sin(),
        };

        // --- IMPROVEMENT 3: Correct Cryptographic Signing ---
        // The signing process is now much cleaner. We pass the data we want to
        // sign directly to the `sign` function and receive a complete, valid
        // `SignedMessage` in return. No placeholder structs are needed.
        let signed_message = self.identity.sign(telemetry);

        log::info!("Generated new telemetry. Gossiping to peers...");

        // Update our own state first.
        self.network_state
            .nodes
            .insert(self.identity.node_id, signed_message.message.clone());

        self.publish_state();

        // Gossip the message to the network.
        self.gossip_to_peers(signed_message).await;
    }

    /// Forwards a message to a random subset of known peers. This is the core of the gossip protocol.
    async fn gossip_to_peers(&self, message: SignedMessage) {
        const GOSSIP_FACTOR: usize = 2; // Number of peers to gossip to.

        let mut rng = thread_rng();
        let peers_to_gossip_to: Vec<_> = self
            .known_peers
            .iter()
            // Don't send a message back to the node that just sent it to us.
            .filter(|(id, _)| **id != message.originator)
            .collect::<Vec<_>>()
            .choose_multiple(&mut rng, GOSSIP_FACTOR)
            .cloned()
            .collect();

        if peers_to_gossip_to.is_empty() {
            log.warn!("No peers to gossip to. Is the network empty?");
            return;
        }

        for (node_id, addr) in peers_to_gossip_to {
            log.debug!("Gossiping message to {} at {}", node_id, addr);
            let command = P2pCommand::SendMessage(*addr, message.clone());
            if let Err(e) = self.p2p_tx.send(command).await {
                log::error!("Failed to send command to P2P manager: {}", e);
            }
        }
    }
    
    /// Sends the current network state to the visualizer.
    fn publish_state(&self) {
        // `send` on a watch channel overwrites the previous value.
        // It returns an error only if there are no receivers, which is fine.
        let _ = self.state_tx.send(self.network_state.clone());
    }
}