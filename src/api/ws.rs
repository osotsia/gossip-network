//! src/api/ws.rs
//!
//! Handles WebSocket connection logic for the visualizer API.

use crate::{
    api::{
        protocol::{SnapshotPayload, UpdatePayload, WebSocketMessage},
        ApiState,
    },
    domain::NetworkState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::stream::StreamExt;
use std::collections::HashSet;
use tokio::sync::broadcast::error::RecvError;

/// The handler for WebSocket upgrade requests.
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Calculates the difference between two network states and produces a list of updates.
fn calculate_delta(old: &NetworkState, new: &NetworkState) -> Vec<UpdatePayload> {
    let mut updates = Vec::new();
    for (id, new_info) in &new.nodes {
        match old.nodes.get(id) {
            Some(old_info) if old_info != new_info => {
                updates.push(UpdatePayload::NodeUpdated {
                    id: *id,
                    info: new_info.clone(),
                });
            }
            None => {
                updates.push(UpdatePayload::NodeAdded {
                    id: *id,
                    info: new_info.clone(),
                });
            }
            _ => {}
        }
    }
    for id in old.nodes.keys() {
        if !new.nodes.contains_key(id) {
            updates.push(UpdatePayload::NodeRemoved { id: *id });
        }
    }
    let old_conns: HashSet<_> = old.active_connections.iter().collect();
    let new_conns: HashSet<_> = new.active_connections.iter().collect();
    for &peer_id in old_conns.difference(&new_conns) {
        updates.push(UpdatePayload::ConnectionStatus {
            peer_id: *peer_id,
            is_connected: false,
        });
    }
    for &peer_id in new_conns.difference(&old_conns) {
        updates.push(UpdatePayload::ConnectionStatus {
            peer_id: *peer_id,
            is_connected: true,
        });
    }
    updates
}

/// Manages a single WebSocket connection, sending an initial state snapshot
/// and broadcasting subsequent delta updates.
async fn handle_socket(mut socket: WebSocket, state: ApiState) {
    tracing::info!("New WebSocket client connected.");
    let mut state_rx = state.state_rx.clone();
    let mut anim_rx = state.animation_tx.subscribe();

    // --- Wait for the first valid state before sending a snapshot ---
    let mut last_sent_state;
    loop {
        let current_state = state_rx.borrow().clone();
        if current_state.self_id.is_some() {
            let snapshot_msg = WebSocketMessage::Snapshot(SnapshotPayload::from(&current_state));
            let initial_json =
                serde_json::to_string(&snapshot_msg).expect("Failed to serialize initial state");

            if socket.send(Message::Text(initial_json)).await.is_err() {
                tracing::warn!("Failed to send initial state to WebSocket client. Closing.");
                return;
            }
            last_sent_state = current_state;
            break;
        }
        if state_rx.changed().await.is_err() {
            tracing::info!("State channel closed before initial state was ready. Disconnecting client.");
            return;
        }
    }

    // --- Watch for changes and broadcast delta updates ---
    loop {
        tokio::select! {
            result = state_rx.changed() => {
                if result.is_err() {
                    tracing::info!("State channel closed. Disconnecting client.");
                    break;
                }
                let new_state = state_rx.borrow().clone();
                if new_state.self_id.is_none() { continue; }
                let updates = calculate_delta(&last_sent_state, &new_state);

                if !updates.is_empty() {
                    for update in updates {
                        let update_msg = WebSocketMessage::Update(update);
                        let json = match serde_json::to_string(&update_msg) {
                            Ok(j) => j,
                            Err(e) => {
                                tracing::error!(error = %e, "Failed to serialize update");
                                continue;
                            }
                        };
                        if socket.send(Message::Text(json)).await.is_err() {
                            tracing::info!("WebSocket client disconnected during state update.");
                            return;
                        }
                    }
                }
                last_sent_state = new_state;
            },
            result = anim_rx.recv() => {
                match result {
                    Ok(peer_id) => {
                        let update_payload = UpdatePayload::AnimateEdge { from_peer: peer_id };
                        let update_msg = WebSocketMessage::Update(update_payload);
                        let json = serde_json::to_string(&update_msg).expect("Failed to serialize animation event");
                        if socket.send(Message::Text(json)).await.is_err() {
                            tracing::info!("WebSocket client disconnected during animation update.");
                            return;
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        tracing::warn!("Animation event channel lagged. Client may miss some animations.");
                    }
                    Err(RecvError::Closed) => {
                        tracing::info!("Animation event channel closed. Disconnecting client.");
                        break;
                    }
                }
            },
            Some(Ok(msg)) = socket.next() => {
                if let Message::Close(_) = msg {
                    tracing::info!("WebSocket client sent close message.");
                    break;
                }
            },
            else => {
                tracing::info!("WebSocket connection closed or a channel dropped.");
                break;
            }
        }
    }
}

// --- NEW: Unit tests for API logic ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NodeId, NodeInfo, TelemetryData};

    // Helper to create a dummy NodeId for testing.
    fn create_node_id(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    // Helper to create dummy NodeInfo.
    fn create_node_info(timestamp_ms: u64) -> NodeInfo {
        NodeInfo {
            telemetry: TelemetryData { timestamp_ms, value: 0.0 },
            community_id: 0,
        }
    }

    #[test]
    fn delta_detects_node_added() {
        let node1 = create_node_id(1);
        let old_state = NetworkState::default();
        let mut new_state = NetworkState::default();
        new_state.nodes.insert(node1, create_node_info(100));

        let delta = calculate_delta(&old_state, &new_state);
        assert_eq!(delta.len(), 1);
        assert!(matches!(delta[0], UpdatePayload::NodeAdded { .. }));
    }

    #[test]
    fn delta_detects_node_removed() {
        let node1 = create_node_id(1);
        let mut old_state = NetworkState::default();
        old_state.nodes.insert(node1, create_node_info(100));
        let new_state = NetworkState::default();

        let delta = calculate_delta(&old_state, &new_state);
        assert_eq!(delta.len(), 1);
        assert!(matches!(delta[0], UpdatePayload::NodeRemoved { .. }));
    }

    #[test]
    fn delta_detects_node_updated() {
        let node1 = create_node_id(1);
        let mut old_state = NetworkState::default();
        old_state.nodes.insert(node1, create_node_info(100));
        let mut new_state = NetworkState::default();
        new_state.nodes.insert(node1, create_node_info(200)); // Newer timestamp

        let delta = calculate_delta(&old_state, &new_state);
        assert_eq!(delta.len(), 1);
        assert!(matches!(delta[0], UpdatePayload::NodeUpdated { .. }));
    }

    #[test]
    fn delta_detects_connection_added_and_removed() {
        let node1 = create_node_id(1);
        let node2 = create_node_id(2);
        let node3 = create_node_id(3);

        let mut old_state = NetworkState::default();
        old_state.active_connections = vec![node1, node2];

        let mut new_state = NetworkState::default();
        new_state.active_connections = vec![node2, node3];

        let delta = calculate_delta(&old_state, &new_state);
        assert_eq!(delta.len(), 2);

        let disconnected = delta.iter().find(|d| matches!(d, UpdatePayload::ConnectionStatus { is_connected: false, .. }));
        let connected = delta.iter().find(|d| matches!(d, UpdatePayload::ConnectionStatus { is_connected: true, .. }));

        assert!(disconnected.is_some());
        assert!(connected.is_some());

        if let Some(UpdatePayload::ConnectionStatus { peer_id, .. }) = disconnected {
            assert_eq!(*peer_id, node1);
        }
        if let Some(UpdatePayload::ConnectionStatus { peer_id, .. }) = connected {
            assert_eq!(*peer_id, node3);
        }
    }

    #[test]
    fn delta_is_empty_when_states_are_identical() {
        let node1 = create_node_id(1);
        let mut state = NetworkState::default();
        state.nodes.insert(node1, create_node_info(100));
        state.active_connections.push(node1);

        let delta = calculate_delta(&state, &state.clone());
        assert!(delta.is_empty());
    }
}