//! src/transport/connection.rs
//!
//! Handles the logic for establishing, caching, and using QUIC connections.

use crate::{
    domain::SignedMessage,
    error::{Error, Result},
    // MODIFICATION: Import new types.
    transport::{ConnectionEvent, InboundMessage, MAX_MESSAGE_SIZE},
};
use quinn::{Connection, Endpoint};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
// MODIFICATION: Add Semaphore.
use tokio::sync::{mpsc, Mutex, Semaphore};

/// Establishes a connection to a peer and caches it.
pub async fn connect_to_peer(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    peer_addr: SocketAddr,
    // NEW: Accept event sender.
    conn_event_tx: mpsc::Sender<ConnectionEvent>,
) -> Result<Connection> {
    let connecting = endpoint
        .connect(peer_addr, "localhost")
        .map_err(|e| Error::ConnectFailed(peer_addr, e))?;

    let conn = connecting
        .await
        .map_err(|e| Error::ConnectionEstablishFailed(peer_addr, e))?;

    tracing::info!(peer = %peer_addr, "Successfully connected to peer");

    // NEW: Send connection event.
    let _ = conn_event_tx
        .send(ConnectionEvent::PeerConnected { peer_addr })
        .await;

    connections.lock().await.insert(peer_addr, conn.clone());
    Ok(conn)
}

/// Gets a cached connection or creates a new one.
async fn get_or_create_connection(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    addr: SocketAddr,
    // NEW: Pass through event sender.
    conn_event_tx: mpsc::Sender<ConnectionEvent>,
) -> Result<Connection> {
    let mut conns_guard = connections.lock().await;
    if let Some(conn) = conns_guard.get(&addr) {
        if conn.close_reason().is_none() {
            return Ok(conn.clone());
        }
        // Connection is closed, remove it.
        conns_guard.remove(&addr);
    }
    drop(conns_guard);
    connect_to_peer(endpoint, connections, addr, conn_event_tx).await
}

/// Sends a single message to a peer, using the connection cache.
pub async fn send_message_to_peer(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    addr: SocketAddr,
    msg: SignedMessage,
    // NEW: Accept event sender.
    conn_event_tx: mpsc::Sender<ConnectionEvent>,
) -> Result<()> {
    let conn = get_or_create_connection(endpoint, connections, addr, conn_event_tx).await?;
    let mut send_stream = conn.open_uni().await?;
    let bytes = bincode::serialize(&msg)?;
    send_stream.write_all(&bytes).await?;
    send_stream.finish().await?;
    tracing::trace!(peer = %addr, "Successfully sent message");
    Ok(())
}

/// Handles a single established QUIC connection, processing all incoming streams.
pub async fn handle_connection(
    conn: quinn::Connecting,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    // NEW: Accept event sender and semaphore.
    conn_event_tx: mpsc::Sender<ConnectionEvent>,
    stream_semaphore: Arc<Semaphore>,
) -> Result<()> {
    let connection = conn.await?;
    let peer_addr = connection.remote_address();
    tracing::info!(peer = %peer_addr, "Accepted connection from peer");

    // NEW: Send connection event.
    let _ = conn_event_tx
        .send(ConnectionEvent::PeerConnected { peer_addr })
        .await;

    connections.lock().await.insert(peer_addr, connection.clone());

    loop {
        tokio::select! {
            stream = connection.accept_uni() => {
                match stream {
                    Ok(mut recv) => {
                        let inbound_tx = inbound_tx.clone();
                        // FIX: Acquire a permit from the semaphore before spawning a task.
                        // `acquire_owned` ties the permit lifetime to the spawned task.
                        let permit = match stream_semaphore.clone().acquire_owned().await {
                            Ok(p) => p,
                            Err(_) => {
                                tracing::warn!("Semaphore closed, cannot accept new streams.");
                                break Ok(());
                            }
                        };
                        tokio::spawn(async move {
                            match recv.read_to_end(MAX_MESSAGE_SIZE).await {
                                Ok(bytes) => {
                                    match bincode::deserialize::<SignedMessage>(&bytes) {
                                        Ok(message) => {
                                            let inbound = InboundMessage { peer_addr, message };
                                            if inbound_tx.send(inbound).await.is_err() {
                                                tracing::warn!("Inbound message channel is closed.");
                                            }
                                        }
                                        Err(e) => tracing::error!(from = %peer_addr, error = %e, "Failed to deserialize message"),
                                    }
                                }
                                Err(e) => tracing::error!(from = %peer_addr, error = %e, "Failed to read from stream (potential DoS: exceeded size limit)"),
                            }
                            // Permit is automatically dropped here when the task finishes.
                            drop(permit);
                        });
                    }
                    Err(e) => {
                        tracing::warn!(peer = %peer_addr, error = %e, "Stream acceptance failed");
                        break Ok(());
                    }
                }
            }
            reason = connection.closed() => {
                 tracing::info!(peer = %peer_addr, reason = %reason, "Connection closed");
                 // NEW: Send disconnect event.
                 let _ = conn_event_tx.send(ConnectionEvent::PeerDisconnected { peer_addr }).await;
                 connections.lock().await.remove(&peer_addr);
                 return Ok(());
            }
        }
    }
}