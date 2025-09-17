//! src/transport/mod.rs
//!
//! Defines the `Transport` service, responsible for all low-level network I/O
//! using the QUIC protocol.

use crate::{
    domain::SignedMessage,
    error::Result,
    transport::{connection::handle_connection, tls::configure_tls},
};
use quinn::{Connection, Endpoint, TokioRuntime};
use socket2::{Domain, Protocol, Socket, Type};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
// MODIFICATION: Add Semaphore for concurrency limiting.
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

pub mod connection;
pub mod tls;

/// The maximum allowed size for a single incoming message on a QUIC stream.
const MAX_MESSAGE_SIZE: usize = 1_024 * 1_024; // 1 MiB
// MODIFICATION: Define a limit for concurrent inbound streams.
const MAX_CONCURRENT_STREAMS: usize = 256;

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

// NEW: Events sent from Transport to Engine to report connection status.
#[derive(Debug)]
pub enum ConnectionEvent {
    PeerConnected { peer_addr: SocketAddr },
    PeerDisconnected { peer_addr: SocketAddr },
}

/// The P2P network transport actor.
pub struct Transport {
    endpoint: Endpoint,
    command_rx: mpsc::Receiver<TransportCommand>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    // NEW: Channel for sending connection events to the Engine.
    conn_event_tx: mpsc::Sender<ConnectionEvent>,
    bootstrap_peers: Vec<SocketAddr>,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    // NEW: Semaphore to limit concurrent stream handling.
    stream_semaphore: Arc<Semaphore>,
}

impl Transport {
    pub fn new(
        bind_addr: SocketAddr,
        bootstrap_peers: Vec<SocketAddr>,
        command_rx: mpsc::Receiver<TransportCommand>,
        inbound_tx: mpsc::Sender<InboundMessage>,
        // NEW: Add the connection event channel to the constructor.
        conn_event_tx: mpsc::Sender<ConnectionEvent>,
    ) -> Result<Self> {
        let (server_config, client_config) = configure_tls()?;

        let socket = Socket::new(
            Domain::for_address(bind_addr),
            Type::DGRAM,
            Some(Protocol::UDP),
        )?;
        socket.set_reuse_address(true)?;
        socket.bind(&bind_addr.into())?;
        let std_socket: std::net::UdpSocket = socket.into();
        std_socket.set_nonblocking(true)?;

        let mut endpoint = Endpoint::new(
            Default::default(),
            Some(server_config),
            std_socket,
            Arc::new(TokioRuntime),
        )?;
        endpoint.set_default_client_config(client_config);

        Ok(Self {
            endpoint,
            command_rx,
            inbound_tx,
            conn_event_tx,
            bootstrap_peers,
            connections: Arc::new(Mutex::new(HashMap::new())),
            // NEW: Initialize the semaphore.
            stream_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_STREAMS)),
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
            // NEW: Clone the event sender for the bootstrap task.
            let conn_event_tx = self.conn_event_tx.clone();
            tokio::spawn(async move {
                tracing::info!(peer = %peer_addr, "Attempting to connect to bootstrap peer");
                if let Err(e) = connection::connect_to_peer(endpoint, connections, peer_addr, conn_event_tx).await {
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
                    // NEW: Clone the event sender and semaphore for the connection handler task.
                    let conn_event_tx = self.conn_event_tx.clone();
                    let stream_semaphore = self.stream_semaphore.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(conn, connections, inbound_tx, conn_event_tx, stream_semaphore).await {
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
                // NEW: Clone the event sender for message sending tasks.
                let conn_event_tx = self.conn_event_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = connection::send_message_to_peer(endpoint, connections, addr, msg, conn_event_tx).await {
                        tracing::warn!(peer = %addr, error = %e, "Failed to send message");
                    }
                });
            }
        }
    }
}