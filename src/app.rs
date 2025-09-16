//! src/app.rs
//!
//! Defines the main `App` struct, which encapsulates the application's state
//! and manages the lifecycle of all its concurrent services.

use crate::{
    api::ApiServer,
    config::Config,
    domain::{Identity, NetworkState},
    engine::Engine,
    error::Result,
    transport::{InboundMessage, Transport, TransportCommand},
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

/// Encapsulates the entire application, including its configuration and the
/// lifecycle management of its concurrent services.
pub struct App {
    config: Config,
    shutdown_token: CancellationToken,
}

impl App {
    /// Creates a new `App` from the given configuration.
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            config,
            shutdown_token: CancellationToken::new(),
        })
    }

    /// The main run loop for the application.
    ///
    /// This function performs the following steps:
    ///   1. Initializes the node's cryptographic identity.
    ///   2. Creates the communication channels that link the services.
    ///   3. Spawns each service (`Transport`, `Engine`, `ApiServer`) in its own
    ///      asynchronous Tokio task.
    ///   4. Waits for a shutdown signal (like Ctrl+C) and gracefully
    ///      terminates all tasks.
    pub async fn run(self) -> Result<()> {
        let identity = Identity::from_file(&self.config.identity_path)?;

        tracing::info!(
            node_id = %identity.node_id,
            p2p_addr = %self.config.p2p_addr,
            "🚀 Starting node..."
        );

        // --- Create Communication Channels ---
        let (transport_command_tx, transport_command_rx) = mpsc::channel::<TransportCommand>(100);
        let (inbound_message_tx, inbound_message_rx) = mpsc::channel::<InboundMessage>(100);
        let (network_state_tx, network_state_rx) = watch::channel(NetworkState::default());

        // --- Instantiate and Spawn Services ---

        // Transport: The network I/O layer.
        let transport = Transport::new(
            self.config.p2p_addr,
            self.config.bootstrap_peers.clone(),
            transport_command_rx,
            inbound_message_tx,
        )?;
        let transport_task = tokio::spawn(transport.run(self.shutdown_token.clone()));
        tracing::debug!("Transport service spawned.");

        // Engine: The core application logic.
        let engine = Engine::new(
            identity,
            self.config.clone(),
            inbound_message_rx,
            transport_command_tx,
            network_state_tx,
        );
        let engine_task = tokio::spawn(engine.run(self.shutdown_token.clone()));
        tracing::debug!("Engine service spawned.");

        // API Server (optional).
        let api_task = if let Some(viz_config) = self.config.visualizer {
            tracing::info!("Visualizer is enabled. Starting API server.");
            let api_server = ApiServer::new(viz_config.bind_addr, network_state_rx);
            let api_server_task = tokio::spawn(api_server.run(self.shutdown_token.clone()));
            Some(api_server_task)
        } else {
            None
        };

        // --- Wait for Shutdown Signal ---
        let shutdown_token = self.shutdown_token.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            tracing::info!("Ctrl+C received. Initiating graceful shutdown...");
            shutdown_token.cancel();
        });

        // --- Await Service Termination ---
        self.shutdown_token.cancelled().await;

        // The tasks will complete once the shutdown token is cancelled.
        // We await them to ensure they finish cleanly.
        if let Err(e) = transport_task.await {
            tracing::error!(error = ?e, "Transport service task failed");
        }
        if let Err(e) = engine_task.await {
            tracing::error!(error = ?e, "Engine service task failed");
        }
        if let Some(task) = api_task {
            if let Err(e) = task.await {
                tracing::error!(error = ?e, "API server task failed");
            }
        }
        tracing::info!("👋 Node has shut down gracefully.");

        Ok(())
    }
}