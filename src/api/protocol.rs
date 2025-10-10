//! src/api/protocol.rs
//!
//! Defines the data contract for the WebSocket API, ensuring a clear separation
//! between backend state and the frontend's data model.

use crate::domain::{NetworkState, NodeId, NodeInfo};
// NEW: Import Deserialize
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A structured message sent from the server to a WebSocket client.
/// This enum represents all possible communications, allowing for strong typing
/// on both the backend and frontend.
#[derive(Debug, Serialize, Deserialize)] // MODIFICATION: Added Deserialize
#[serde(tag = "type", content = "payload")]
pub enum WebSocketMessage {
    #[serde(rename = "snapshot")]
    Snapshot(SnapshotPayload),
    #[serde(rename = "update")]
    Update(UpdatePayload),
}

/// The initial state payload sent to a client upon connection.
#[derive(Debug, Serialize, Deserialize)] // MODIFICATION: Added Deserialize
pub struct SnapshotPayload {
    pub self_id: NodeId,
    pub nodes: HashMap<NodeId, NodeInfo>,
    pub active_connections: Vec<NodeId>,
}

impl From<&NetworkState> for SnapshotPayload {
    fn from(state: &NetworkState) -> Self {
        Self {
            self_id: state.self_id.unwrap_or_default(),
            nodes: state.nodes.clone(),
            active_connections: state.active_connections.clone(),
        }
    }
}

/// An incremental update payload, describing a specific change in the network state.
#[derive(Debug, Serialize, Deserialize)] // MODIFICATION: Added Deserialize
#[serde(tag = "event", content = "data")]
pub enum UpdatePayload {
    #[serde(rename = "node_added")]
    NodeAdded { id: NodeId, info: NodeInfo },
    #[serde(rename = "node_updated")]
    NodeUpdated { id: NodeId, info: NodeInfo },
    #[serde(rename = "node_removed")]
    NodeRemoved { id: NodeId },
    #[serde(rename = "connection_status")]
    ConnectionStatus {
        peer_id: NodeId,
        is_connected: bool,
    },
    #[serde(rename = "animate_edge")]
    AnimateEdge { from_peer: NodeId },
}