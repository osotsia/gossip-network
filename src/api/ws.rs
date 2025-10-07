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
    // FIX: Subscribe to the sender to get a new Receiver for this client.
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
            // --- Branch 1: Handle state changes for nodes and connections ---
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

            // --- Branch 2: Handle animation trigger events ---
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

            // --- Branch 3: Handle client-side messages (e.g., close) ---
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