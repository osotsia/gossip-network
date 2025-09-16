//! src/transport/connection.rs
//!
//! Handles the logic for establishing, caching, and using QUIC connections.

use crate::{
    domain::SignedMessage,
    error::{Error, Result},
    transport::{InboundMessage, MAX_MESSAGE_SIZE},
};
use quinn::{Connection, Endpoint};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, Mutex};

/// Establishes a connection to a peer and caches it.
pub async fn connect_to_peer(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    peer_addr: SocketAddr,
) -> Result<Connection> {
    let connecting = endpoint
        .connect(peer_addr, "localhost")
        .map_err(|e| Error::ConnectFailed(peer_addr, e))?;

    let conn = connecting
        .await
        .map_err(|e| Error::ConnectionEstablishFailed(peer_addr, e))?;

    tracing::info!(peer = %peer_addr, "Successfully connected to peer");
    connections.lock().await.insert(peer_addr, conn.clone());
    Ok(conn)
}

/// Gets a cached connection or creates a new one.
async fn get_or_create_connection(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    addr: SocketAddr,
) -> Result<Connection> {
    let conns_guard = connections.lock().await;
    if let Some(conn) = conns_guard.get(&addr) {
        if conn.close_reason().is_none() {
            return Ok(conn.clone());
        }
    }
    drop(conns_guard);
    connect_to_peer(endpoint, connections, addr).await
}

/// Sends a single message to a peer, using the connection cache.
pub async fn send_message_to_peer(
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    addr: SocketAddr,
    msg: SignedMessage,
) -> Result<()> {
    let conn = get_or_create_connection(endpoint, connections, addr).await?;
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
) -> Result<()> {
    let connection = conn.await?;
    let peer_addr = connection.remote_address();
    tracing::info!(peer = %peer_addr, "Accepted connection from peer");
    connections.lock().await.insert(peer_addr, connection.clone());

    loop {
        tokio::select! {
            stream = connection.accept_uni() => {
                match stream {
                    Ok(mut recv) => {
                        let inbound_tx = inbound_tx.clone();
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
                 connections.lock().await.remove(&peer_addr);
                 return Ok(());
            }
        }
    }
}