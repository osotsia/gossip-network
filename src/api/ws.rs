//! src/api/ws.rs
//!
//! Handles WebSocket connection logic for the visualizer API.

use crate::api::ApiState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};

/// The handler for WebSocket upgrade requests.
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Manages a single WebSocket connection, sending an initial state snapshot
/// and broadcasting subsequent updates.
async fn handle_socket(mut socket: WebSocket, state: ApiState) {
    tracing::info!("New WebSocket client connected.");
    let mut state_rx = state.state_rx.clone();

    // Send initial state snapshot.
    let initial_state = state_rx.borrow().clone();
    let initial_json =
        serde_json::to_string(&initial_state).expect("Failed to serialize initial state");

    if socket.send(Message::Text(initial_json)).await.is_err() {
        tracing::warn!("Failed to send initial state to WebSocket client. Closing.");
        return;
    }

    // Watch for changes and broadcast updates.
    loop {
        tokio::select! {
            Ok(_) = state_rx.changed() => {
                let new_state = state_rx.borrow().clone();
                let new_json = match serde_json::to_string(&new_state) {
                    Ok(json) => json,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to serialize new state");
                        continue;
                    }
                };

                if socket.send(Message::Text(new_json)).await.is_err() {
                    tracing::info!("WebSocket client disconnected.");
                    break;
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Close(_) = msg {
                    tracing::info!("WebSocket client sent close message.");
                    break;
                }
            }
            else => {
                tracing::info!("WebSocket connection closed or state channel dropped.");
                break;
            }
        }
    }
}