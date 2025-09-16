//! src/visualizer.rs
//!
//! The Broadcast Studio of our system.
//!
//! This module is responsible for serving the Svelte frontend and providing the
//! real-time WebSocket API for visualization. It is a read-only component that
//! subscribes to state updates from the `GossipEngine`.
//!
//! The main components are:
//!   - An Axum web server to handle HTTP requests.
//!   - A route to serve static files (the compiled Svelte app).
//!   - A WebSocket handler that manages client connections and streams state updates.

use crate::gossip::NetworkState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::watch;
use tower_http::services::ServeDir;
use tokio_util::sync::CancellationToken;

/// The shared state accessible by all Axum handlers.
/// We wrap the receiver in an `Arc` to allow for shared, thread-safe access.
#[derive(Clone)]
struct AppState {
    state_rx: watch::Receiver<NetworkState>,
}

/// The main entrypoint for the visualizer.
///
/// This function sets up the Axum router with its routes and starts the server.
/// It takes a `watch::Receiver` to get live updates from the `GossipEngine`.
pub async fn run_visualizer(
    bind_addr: SocketAddr,
    state_rx: watch::Receiver<NetworkState>,
    // --- IMPROVEMENT: Robust Shutdown ---
    shutdown_token: CancellationToken,
) -> anyhow::Result<()> {
    let app_state = AppState { state_rx };

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .nest_service("/", ServeDir::new("frontend/dist"))
        .with_state(app_state);

    log::info!("Visualizer web server listening on http://{}", bind_addr);
    
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    
    // --- IMPROVEMENT: Robust Shutdown ---
    // We configure the server to listen for the shutdown signal.
    // `with_graceful_shutdown` takes a future that resolves when shutdown should begin.
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown_token.cancelled().await;
            log::info!("Visualizer server received shutdown signal.");
        })
        .await?;
    
    Ok(())
}

/// The handler for WebSocket upgrade requests.
/// This is called when a client tries to open a WebSocket connection at `/ws`.
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// The core logic for a single WebSocket connection.
///
/// Once a connection is established, this function is responsible for:
///   1. Sending an initial "snapshot" of the entire network state.
///   2. Watching for any subsequent state changes and broadcasting them to the client.
///   3. Handling messages from the client (though we don't expect any in this app).
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    log::info!("New WebSocket client connected.");

    // The `watch::Receiver` allows us to get the latest state and then
    // be notified of any changes. We clone it for this specific client.
    let mut state_rx = state.state_rx.clone();

    // --- Task 1: Send the initial state snapshot ---
    // We get the most recent state value from the channel.
    let initial_state = state_rx.borrow().clone();
    let initial_json = serde_json::to_string(&initial_state)
        .expect("Failed to serialize initial state");

    if socket.send(Message::Text(initial_json)).await.is_err() {
        log::warn!("Failed to send initial state to WebSocket client. Closing.");
        return;
    }

    // --- Task 2: Watch for changes and broadcast updates ---
    // The main loop for this client.
    loop {
        // The `changed()` method on the receiver will wait until the value
        // in the channel has been updated.
        // We also need to handle the case where the client disconnects.
        tokio::select! {
            // Await a state change from the GossipEngine.
            Ok(_) = state_rx.changed() => {
                let new_state = state_rx.borrow().clone();
                let new_json = match serde_json::to_string(&new_state) {
                    Ok(json) => json,
                    Err(e) => {
                        log::error!("Failed to serialize new state: {}", e);
                        continue;
                    }
                };

                // If sending fails, the client has likely disconnected. We break the loop.
                if socket.send(Message::Text(new_json)).await.is_err() {
                    log::info!("WebSocket client disconnected.");
                    break;
                }
            }
            
            // Await a message from the client (e.g., a ping or close message).
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Close(_) = msg {
                    log::info!("WebSocket client sent close message.");
                    break;
                }
                // We can ignore other message types for this application.
            }

            // If the state channel closes or the client disconnects, we exit.
            else => {
                log::info!("WebSocket connection closed or state channel dropped.");
                break;
            }
        }
    }
}