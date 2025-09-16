//! src/lib.rs
//!
//! This is the main library crate for our gossip network application.
//! It contains the `run` function, which serves as the primary entrypoint
//! for initializing and orchestrating all the concurrent components of the system.

// Declare all the modules that make up our library.
pub mod config;
pub mod crypto;
pub mod gossip;
pub mod model;
pub mod p2p;
pub mod visualizer;

use crate::{
    config::Config,
    crypto::Identity,
    gossip::{GossipEngine, NetworkState},
    p2p::P2PManager,
    visualizer::run_visualizer,
};
use anyhow::Result;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

/// The main entrypoint for the gossip network application.
///
/// This function performs the following steps:
///   1. Loads the configuration.
///   2. Initializes the node's cryptographic identity.
///   3. Creates the communication channels that will link the different actors.
///   4. Spawns each actor (`P2PManager`, `GossipEngine`, `Visualizer`) in its own
///      asynchronous Tokio task.
///   5. Waits for a shutdown signal (like Ctrl+C) and gracefully terminates all tasks.
pub async fn run() -> Result<()> {
    // --- 1. Initialization ---
    let config = Config::load().expect("Failed to load configuration.");
    let identity = Identity::from_file(&config.identity_path)?;
    // --- IMPROVEMENT: Robust Shutdown ---
    // This CancellationToken is the master signal for graceful shutdown.
    let shutdown_token = CancellationToken::new();

    log::info!(
        "🚀 Starting node {}...",
        identity.node_id
    );

    // --- 2. Create Communication Channels ---
    let (p2p_command_tx, p2p_command_rx) = mpsc::channel(100);
    let (inbound_message_tx, inbound_message_rx) = mpsc::channel(100);
    let (network_state_tx, network_state_rx) = watch::channel(NetworkState::default());

    // --- 3. Instantiate and Spawn Actors ---

    // P2P Manager: The Switchboard Operator
    let p2p_manager = P2PManager::new(
        config.p2p_addr,
        config.bootstrap_peers.clone(),
        p2p_command_rx,
        inbound_message_tx,
    )?;
    // --- IMPROVEMENT: Robust Shutdown ---
    // We clone the token and pass it to the actor's run method.
    let p2p_task = tokio::spawn(p2p_manager.run(shutdown_token.clone()));
    log::info!("P2P Manager task spawned.");

    // Gossip Engine: The Town Crier
    let gossip_engine = GossipEngine::new(
        identity,
        config.gossip_interval_ms,
        inbound_message_rx,
        p2p_command_tx.clone(),
        network_state_tx,
    );
    // --- IMPROVEMENT: Robust Shutdown ---
    let gossip_task = tokio::spawn(gossip_engine.run(shutdown_token.clone()));
    log::info!("Gossip Engine task spawned.");

    // Visualizer: The Broadcast Studio (optional)
    let visualizer_task = if let Some(viz_config) = config.visualizer {
        log::info!("Visualizer is enabled.");
        // --- IMPROVEMENT: Robust Shutdown ---
        let viz_task = tokio::spawn(run_visualizer(
            viz_config.bind_addr,
            network_state_rx,
            shutdown_token.clone(),
        ));
        Some(viz_task)
    } else {
        None
    };

    // --- 4. Wait for Shutdown Signal ---
    // This section ensures a clean shutdown. When Ctrl+C is pressed, we notify all
    // tasks that need to be aware of shutdown, and then we wait for them to finish.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("Ctrl+C received. Initiating graceful shutdown...");
            // --- IMPROVEMENT: Robust Shutdown ---
            // Trigger the shutdown signal. All tasks listening to the token will be notified.
            shutdown_token.cancel();
        }
    }

    // --- 5. Graceful Shutdown ---
    // We can now await the completion of our main tasks.
    // The `.await` will not return until each task's `run` function has completed.
    if let Err(e) = p2p_task.await {
        log::error!("P2P Manager task failed: {:?}", e);
    }
    if let Err(e) = gossip_task.await {
        log::error!("Gossip Engine task failed: {:?}", e);
    }
    if let Some(task) = visualizer_task {
        if let Err(e) = task.await {
            log::error!("Visualizer task failed: {:?}", e);
        }
    }
    
    log::info!("👋 Node has shut down gracefully.");
    Ok(())
}