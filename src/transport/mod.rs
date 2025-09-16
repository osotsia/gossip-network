//! src/transport/mod.rs
//!
//! Defines the `Transport` service, responsible for all low-level network I/O
//! using the QUIC protocol.

use crate::{
    domain::SignedMessage,
    error::{Error, Result},
    transport::{connection::handle_connection, tls::configure_tls},
};
use quinn::{Connection, Endpoint};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub mod connection;
pub mod tls;

/// The maximum allowed size for a single incoming message on a QUIC stream.
const MAX_MESSAGE_SIZE: usize = 1_024 * 1_024; // 1 MiB

/// Commands that can be sent to the `Transport` service.
#[derive(Debug)]
pub enum TransportCommand {
    SendMessage(SocketAddr, SignedMessage),
}

/// A message received from a peer, bundled with its network address.
#[derive(Debug)]
pub struct InboundMessage {
    pub peer_addr: SocketAddr,
    pub message: SignedMessage,
}

/// The P2P network transport actor.
pub struct Transport {
    endpoint: Endpoint,
    command_rx: mpsc::Receiver<TransportCommand>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    bootstrap_peers: Vec<SocketAddr>,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
}

impl Transport {
    pub fn new(
        bind_addr: SocketAddr,
        bootstrap_peers: Vec<SocketAddr>,
        command_rx: mpsc::Receiver<TransportCommand>,
        inbound_tx: mpsc::Sender<InboundMessage>,
    ) -> Result<Self> {
        let (server_config, client_config) = configure_tls()?;
        let mut endpoint = Endpoint::server(server_config, bind_addr)?;
        endpoint.set_default_client_config(client_config);

        Ok(Self {
            endpoint,
            command_rx,
            inbound_tx,
            bootstrap_peers,
            connections: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// The main run loop for the `Transport` service.
    pub async fn run(mut self, shutdown_token: CancellationToken) {
        let local_addr = self.endpoint.local_addr().unwrap();
        tracing::info!(listen_addr = %local_addr, "Transport service started");

        // Initial bootstrapping connections.
        for &peer_addr in &self.bootstrap_peers {
            let endpoint = self.endpoint.clone();
            let connections = self.connections.clone();
            tokio::spawn(async move {
                tracing::info!(peer = %peer_addr, "Attempting to connect to bootstrap peer");
                if let Err(e) = connection::connect_to_peer(endpoint, connections, peer_addr).await {
                    tracing::error!(peer = %peer_addr, error = %e, "Failed to connect to bootstrap peer");
                }
            });
        }

        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Transport service received shutdown signal.");
                    break;
                },
                Some(conn) = self.endpoint.accept() => {
                    let connections = self.connections.clone();
                    let inbound_tx = self.inbound_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(conn, connections, inbound_tx).await {
                            tracing::error!(error = %e, "Connection handling failed");
                        }
                    });
                },
                Some(command) = self.command_rx.recv() => {
                    self.handle_command(command).await;
                }
                else => {
                    tracing::info!("Command channel closed. Transport service shutting down.");
                    break;
                }
            }
        }
        self.endpoint.wait_idle().await;
    }

    async fn handle_command(&self, command: TransportCommand) {
        match command {
            TransportCommand::SendMessage(addr, msg) => {
                let endpoint = self.endpoint.clone();
                let connections = self.connections.clone();
                tokio::spawn(async move {
                    if let Err(e) = connection::send_message_to_peer(endpoint, connections, addr, msg).await {
                        tracing::error!(peer = %addr, error = %e, "Failed to send message");
                    }
                });
            }
        }
    }
}