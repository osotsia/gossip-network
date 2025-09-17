// --- File: src/api/mod.rs ---
//! src/api/mod.rs
//!
//! Defines the `ApiServer` service, which provides the web frontend and
//! a WebSocket endpoint for real-time visualization.

// FIX: Remove the incorrect use statement for websocket_handler.
use crate::domain::NetworkState;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tower_http::services::ServeDir;

pub mod protocol;
pub mod ws;

/// The shared state accessible by all Axum handlers.
#[derive(Clone)]
pub struct ApiState {
    pub state_rx: watch::Receiver<NetworkState>,
}

pub struct ApiServer {
    bind_addr: SocketAddr,
    state_rx: watch::Receiver<NetworkState>,
}

impl ApiServer {
    pub fn new(bind_addr: SocketAddr, state_rx: watch::Receiver<NetworkState>) -> Self {
        Self {
            bind_addr,
            state_rx,
        }
    }

    pub async fn run(self, shutdown_token: CancellationToken) -> crate::error::Result<()> {
        let app_state = ApiState {
            state_rx: self.state_rx,
        };

        // FIX: Use the correct path to the handler: `ws::websocket_handler`.
        let app = Router::new()
            .route("/ws", get(ws::websocket_handler))
            .nest_service("/", ServeDir::new("dist"))
            .with_state(app_state);

        tracing::info!(listen_addr = %self.bind_addr, "API server listening");

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_token.cancelled().await;
                tracing::info!("API server received shutdown signal.");
            })
            .await?;

        Ok(())
    }
}