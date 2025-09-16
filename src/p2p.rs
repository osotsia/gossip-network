//! src/p2p.rs
//!
//! The Switchboard Operator of our system.
//!
//! This module is responsible for all low-level network I/O. It creates and manages
//! a QUIC endpoint, listens for incoming connections from peers, and establishes
//! outgoing connections. Its primary job is to abstract the complexities of the
//! network protocol from the application logic.
//!
//! The `P2PManager` actor provides a simple, channel-based interface:
//!   - An input channel (`command_rx`) to receive commands like "send this data to this peer".
//!   - An output channel (`inbound_tx`) to forward successfully received and deserialized
//!     messages from peers up to the application layer.
//!
//! It operates on raw `SignedMessage` objects, focusing only on transport, not on the
//! content or validity of the messages themselves.

use crate::model::SignedMessage;
use anyhow::{anyhow, Context, Result};
use quinn::{ClientConfig, ConnectError, Connection, Endpoint, ServerConfig};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

/// Commands that can be sent to the `P2PManager` task.
#[derive(Debug)]
pub enum Command {
    /// Command to send a message to a specific peer.
    SendMessage(SocketAddr, SignedMessage),
}

/// A message received from a peer, bundled with its network address.
#[derive(Debug)]
pub struct InboundMessage {
    pub peer_addr: SocketAddr,
    pub message: SignedMessage,
}

/// The P2PManager actor. It owns the QUIC endpoint and manages all connections.
pub struct P2PManager {
    endpoint: Endpoint,
    command_rx: mpsc::Receiver<Command>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    bootstrap_peers: Vec<SocketAddr>,
    /// --- IMPROVEMENT 1: Connection Caching ---
    /// A map to store and reuse active QUIC connections.
    /// This avoids the massive performance cost of creating a new connection for every message.
    /// It is wrapped in Arc<Mutex> to be safely shared across async tasks.
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
}

impl P2PManager {
    /// Creates a new `P2PManager` and configures the QUIC endpoint.
    pub fn new(
        bind_addr: SocketAddr,
        bootstrap_peers: Vec<SocketAddr>,
        command_rx: mpsc::Receiver<Command>,
        inbound_tx: mpsc::Sender<InboundMessage>,
    ) -> Result<Self> {
        // --- IMPROVEMENT 2: Secure PKI Configuration ---
        // Load our private CA and the node's certificate. This is now mandatory.
        let (server_config, client_config) =
            configure_tls().context("Failed to configure QUIC TLS with private CA")?;

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

    /// The main run loop for the `P2PManager` actor.
    pub async fn run(mut self, shutdown_token: CancellationToken) { // <--- ADDED TOKEN
        log::info!(
            "P2P Manager started. Listening on {}",
            self.endpoint.local_addr().unwrap()
        );

        // Initial bootstrapping connections
        for &peer_addr in &self.bootstrap_peers {
            let endpoint = self.endpoint.clone();
            let connections = self.connections.clone();
            tokio::spawn(async move {
                log::info!("Attempting to connect to bootstrap peer {}", peer_addr);
                if let Err(e) = connect_to_peer(endpoint, connections, peer_addr).await {
                    log::error!("Failed to connect to bootstrap peer {}: {}", peer_addr, e);
                }
            });
        }

        loop {
            // --- IMPROVEMENT: Robust Shutdown ---
            tokio::select! {
                // Add a branch to listen for the shutdown signal.
                _ = shutdown_token.cancelled() => {
                    log::info!("P2P Manager received shutdown signal.");
                    break;
                },

                Some(conn) = self.endpoint.accept() => {
                    // ... (existing connection handling logic)
                },
                Some(command) = self.command_rx.recv() => {
                    self.handle_command(command).await;
                }
                else => {
                    log::info!("Command channel closed. P2P Manager shutting down.");
                    break;
                }
            }
        }
        // When the loop breaks, we perform a clean shutdown of the QUIC endpoint.
        self.endpoint.wait_idle().await;
    }

    /// Handles a single command received from the application.
    async fn handle_command(&self, command: Command) {
        match command {
            Command::SendMessage(addr, msg) => {
                log::debug!("Attempting to send message to {}", addr);
                // --- IMPROVEMENT 1: Connection Caching ---
                // We now spawn a task that gets a reusable connection and sends the message.
                let endpoint = self.endpoint.clone();
                let connections = self.connections.clone();
                tokio::spawn(async move {
                    if let Err(e) = send_message_to_peer(endpoint, connections, addr, msg).await {
                        log::error!("Failed to send message to peer {}: {}", addr, e);
                    }
                });
            }
        }
    }
}

/// Helper function to establish a connection to a single peer and cache it.
async fn connect_to_peer(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    peer_addr: SocketAddr,
) -> Result<Connection> {
    log::debug!("Connecting to peer {}", peer_addr);
    // The "localhost" server name must match the certificate's domain name.
    let conn = endpoint
        .connect(peer_addr, "localhost")?
        .await
        .context(format!("Failed to connect to peer at {}", peer_addr))?;

    log::info!("Successfully connected to peer {}", peer_addr);

    // Cache the new connection for future use.
    connections.lock().await.insert(peer_addr, conn.clone());
    Ok(conn)
}

/// Gets a cached connection or creates a new one. This is the core of the caching logic.
async fn get_or_create_connection(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    addr: SocketAddr,
) -> Result<Connection> {
    // Check for an existing, non-closed connection inside the lock.
    let mut conns_guard = connections.lock().await;
    if let Some(conn) = conns_guard.get(&addr) {
        // `close_reason` is None if the connection is still alive.
        if conn.close_reason().is_none() {
            log::trace!("Reusing existing connection to {}", addr);
            return Ok(conn.clone());
        }
    }

    // No valid connection found, so we must drop the lock before attempting to connect.
    // This avoids holding the lock during a long-running, blocking network operation.
    drop(conns_guard);

    log::debug!("No existing connection to {}. Creating a new one.", addr);
    let conn = connect_to_peer(endpoint, connections.clone(), addr).await?;
    Ok(conn)
}

/// Helper function to send a single message to a peer, using the connection cache.
async fn send_message_to_peer(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    addr: SocketAddr,
    msg: SignedMessage,
) -> Result<()> {
    // --- IMPROVEMENT 1: Connection Caching ---
    let conn = get_or_create_connection(endpoint, connections, addr).await?;

    let mut send_stream = conn.open_uni().await?;
    let bytes = bincode::serialize(&msg).context("Failed to serialize message for sending")?;
    send_stream.write_all(&bytes).await?;
    send_stream.finish().await?;
    log::trace!("Successfully sent message to {}", addr);
    Ok(())
}

/// Handles a single established QUIC connection.
async fn handle_connection(
    conn: quinn::Connecting,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    inbound_tx: mpsc::Sender<InboundMessage>,
) -> Result<()> {
    let connection = conn.await?;
    let peer_addr = connection.remote_address();
    log::info!("Accepted connection from peer {}", peer_addr);

    // --- IMPROVEMENT 1: Connection Caching ---
    // Add the newly accepted connection to our map so we can use it for sending.
    connections
        .lock()
        .await
        .insert(peer_addr, connection.clone());

    // Main loop for handling incoming streams on this connection.
    loop {
        tokio::select! {
            // A unidirectional stream was opened by the peer.
            Ok(mut recv) = connection.accept_uni() => {
                let inbound_tx = inbound_tx.clone();
                // Spawn a task to read the full message from the stream.
                tokio::spawn(async move {
                    match recv.read_to_end(usize::MAX).await {
                        Ok(bytes) => {
                            match bincode::deserialize::<SignedMessage>(&bytes) {
                                Ok(message) => {
                                    let inbound_message = InboundMessage { peer_addr, message };
                                    if let Err(e) = inbound_tx.send(inbound_message).await {
                                        log::error!("Failed to send inbound message to channel: {}", e);
                                    }
                                }
                                Err(e) => log::error!("Failed to deserialize message from {}: {}", peer_addr, e),
                            }
                        }
                        Err(e) => log::error!("Failed to read message from stream from {}: {}", peer_addr, e),
                    }
                });
            }
            // The connection was closed by the peer.
            Err(e) = connection.closed() => {
                 log::info!("Connection with {} closed: {}", peer_addr, e);
                 // --- IMPROVEMENT 1: Connection Caching ---
                 // Remove the closed connection from our map to prevent reuse.
                 connections.lock().await.remove(&peer_addr);
                 return Ok(());
            }
        }
    }
}

/// --- IMPROVEMENT 2: Secure PKI Configuration ---
/// Configures TLS for the client and server using a shared private CA.
/// This ensures that nodes only trust peers that are part of our private network.
///
/// NOTE: This function expects `ca.cert`, `node.cert`, and `node.key` files to exist.
/// You would generate these once for your entire network.
fn configure_tls() -> Result<(ServerConfig, ClientConfig)> {
    // Load the certificate authority. All nodes must trust this CA.
    let ca_cert_der = fs::read("certs/ca.cert").context("Failed to read CA certificate")?;
    let ca_cert = rustls::p_k_i_types::CertificateDer::from(ca_cert_der);

    // Configure the client to trust the CA.
    let mut root_store = rustls::RootCertStore::empty();
    root_store
        .add(ca_cert.clone())
        .context("Failed to add CA certificate to root store")?;
    let mut client_config = ClientConfig::with_root_certificates(root_store)?;
    // Quinn requires ALPN protocols to be set.
    client_config.alpn_protocols = vec![b"gossip/1.0".to_vec()];


    // Configure the server with its own certificate and private key, signed by the CA.
    let cert_chain_der = fs::read("certs/node.cert").context("Failed to read node certificate")?;
    let key_der = fs::read("certs/node.key").context("Failed to read node private key")?;

    let cert_chain = vec![rustls::p_k_i_types::CertificateDer::from(cert_chain_der)];
    let key = rustls::p_k_i_types::PrivatePkcs8KeyDer::from(key_der).into();

    let mut server_config =
        ServerConfig::with_single_cert(cert_chain, key).context("Failed to create server config")?;
    
    // Quinn requires ALPN protocols to be set.
    server_config.alpn_protocols = vec![b"gossip/1.0".to_vec()];

    // Set keep-alives to prevent connections from timing out during idle periods.
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(10)));

    Ok((server_config, client_config))
}

/*
--------------------------------------------------------------------------------
-- HOW TO GENERATE CERTIFICATES FOR THIS PKI
--------------------------------------------------------------------------------
This setup requires a private Public Key Infrastructure (PKI).
You can use a simple tool like `minica` (https://github.com/jsha/minica) for this.

1. Install `minica` (requires Go):
   go install github.com/jsha/minica@latest

2. Create a directory for certificates at the project root:
   mkdir certs
   cd certs

3. Generate the CA and a certificate for "localhost".
   All our nodes will connect using the "localhost" server name.
   minica --domains localhost

This will create:
  - `minica.pem` and `minica.key` (The Certificate Authority)
  - `localhost/cert.pem` and `localhost/key.pem` (The node's certificate)

4. Convert the PEM files to the DER format Rustls expects:
   openssl x509 -outform der -in minica.pem -out ca.cert
   openssl x509 -outform der -in localhost/cert.pem -out node.cert
   openssl pkcs8 -topk8 -nocrypt -outform der -in localhost/key.pem -out node.key

5. Your `certs/` directory should now contain `ca.cert`, `node.cert`, and `node.key`.
   All nodes in your network will share these three files for this simple setup.
   In a real-world system, each node would have its own unique `node.cert` and
   `node.key` files, signed by the same `ca.cert`.
--------------------------------------------------------------------------------
*/