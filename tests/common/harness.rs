//! tests/common/harness.rs
//!
//! A comprehensive test harness for creating and managing test clusters.
//! This module provides the building blocks for all integration and E2E tests,
//! abstracting away the boilerplate of:
//! - Generating unique TLS certificates for different trust domains (CAs).
//! - Creating temporary directories and configuration for each node.
//! - Spawning nodes in the background.
//! - Managing graceful shutdown.
//! - Connecting WebSocket clients to monitor node state.

use anyhow::{Context, Result};
use futures::stream::StreamExt;
use gossip_network::{api::protocol::WebSocketMessage, domain::NetworkState, App, Config};
use quinn::{ClientConfig, Endpoint};
use rcgen::{Certificate, CertificateParams, DistinguishedName};
use std::{
    fs,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tempfile::{tempdir, TempDir};
use tokio_util::sync::CancellationToken;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    WebSocketStream,
};
use tracing::info;

/// Represents a set of TLS certificates (CA and a node cert signed by it).
#[derive(Clone)]
pub struct CertSet {
    pub ca_cert_der: Vec<u8>,
    pub node_cert_der: Vec<u8>,
    pub node_key_der: Vec<u8>,
}

impl CertSet {
    /// Creates a file-system representation of the certificates in a directory.
    pub fn write_to_disk(&self, dir: &PathBuf) -> Result<()> {
        fs::create_dir_all(dir)?;
        fs::write(dir.join("ca.cert"), &self.ca_cert_der)?;
        fs::write(dir.join("node.cert"), &self.node_cert_der)?;
        fs::write(dir.join("node.key"), &self.node_key_der)?;
        Ok(())
    }
}

/// Generates a new, unique `CertSet`.
pub fn generate_certs(domain: &str) -> CertSet {
    let ca_params = CertificateParams::new(vec![domain.to_string()]);
    let ca_cert = Certificate::from_params(ca_params).unwrap();
    let ca_cert_der = ca_cert.serialize_der().unwrap();
    let ca_key_pem = ca_cert.serialize_private_key_pem();

    let mut node_params = CertificateParams::new(vec![domain.to_string()]);
    node_params.distinguished_name = DistinguishedName::new();
    let node_cert = Certificate::from_params(node_params).unwrap();
    let node_cert_der = node_cert
        .serialize_der_with_signer(&ca_cert)
        .unwrap();
    let node_key_der = node_cert.serialize_private_key_der();

    let _ = ca_key_pem;

    CertSet {
        ca_cert_der,
        node_cert_der,
        node_key_der,
    }
}

/// A handle to a running gossip node instance in a test environment.
pub struct TestNode {
    pub config: Config,
    pub p2p_addr: SocketAddr,
    pub api_addr: SocketAddr,
    pub shutdown_token: CancellationToken,
    _temp_dir: TempDir,
}

impl TestNode {
    /// Configures and spawns a new node in a background task.
    pub async fn spawn(
        bootstrap_peers: Vec<SocketAddr>,
        certs: &CertSet,
    ) -> Result<Self> {
        let temp_dir = tempdir().context("Failed to create temp dir")?;
        let certs_dir = temp_dir.path().join("certs");
        certs
            .write_to_disk(&certs_dir)
            .context("Failed to write certs to disk")?;

        let p2p_addr = get_ephemeral_addr()?;
        let api_addr = get_ephemeral_addr()?;

        let config = Config {
            identity_path: temp_dir.path().join("identity.key"),
            p2p_addr,
            bootstrap_peers,
            gossip_interval_ms: 250,
            gossip_factor: 2,
            node_ttl_ms: 5000,
            // MODIFICATION: Add the new cleanup_interval_ms field.
            cleanup_interval_ms: 1000,
            community_id: 0,
            visualizer: Some(gossip_network::config::VisualizerConfig { bind_addr: api_addr }),
        };

        let shutdown_token = CancellationToken::new();
        let app_dir = temp_dir.path().to_path_buf();
        let app_config = config.clone();
        let app_token = shutdown_token.clone();

        tokio::spawn(async move {
            std::env::set_current_dir(&app_dir)
                .expect("Failed to set current dir for spawned app");
            if let Err(e) = App::new(app_config)
                .expect("Failed to create app")
                .run()
                .await
            {
                if !app_token.is_cancelled() {
                    tracing::error!(error = ?e, "Test node app failed");
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        info!(p2p = %p2p_addr, api = %api_addr, "Spawned test node");
        
        std::env::set_current_dir(std::env::var("CARGO_MANIFEST_DIR").unwrap()).unwrap();

        Ok(Self {
            config,
            p2p_addr,
            api_addr,
            shutdown_token,
            _temp_dir: temp_dir,
        })
    }

    /// Creates a WebSocket client connected to this node's API.
    pub async fn ws_client(&self) -> Result<WebSocketStream<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>> {
        let url = format!("ws://{}", self.api_addr);
        let mut request = url.into_client_request()?;
        request.headers_mut().append("Host", self.api_addr.to_string().parse()?);
        
        let (socket, _) = connect_async(request).await.context("WebSocket connect failed")?;
        Ok(socket)
    }

    /// Shuts down the node gracefully.
    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }
}

fn get_ephemeral_addr() -> Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?)
}

pub async fn wait_for_state<F>(
    ws_client: &mut WebSocketStream<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>,
    predicate: F,
    timeout_duration: Duration,
) -> Result<NetworkState>
where
    F: Fn(&NetworkState) -> bool,
{
    let wait = tokio::time::timeout(timeout_duration, async {
        loop {
            let msg = ws_client.next().await
                .context("WebSocket stream ended prematurely")?
                .context("WebSocket message error")?;

            if let Message::Text(text) = msg {
                if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                    if let WebSocketMessage::Snapshot(payload) = ws_msg {
                        let state = NetworkState {
                            self_id: Some(payload.self_id),
                            nodes: payload.nodes,
                            active_connections: payload.active_connections,
                        };

                        if predicate(&state) {
                            return Ok(state);
                        }
                    }
                }
            }
        }
    });

    wait.await.context("Timeout while waiting for state condition")?
}

pub fn create_quic_client(certs: &CertSet) -> Result<Endpoint> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(&rustls::Certificate(certs.ca_cert_der.clone()))?;

    let client_crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    
    let client_config = ClientConfig::new(Arc::new(client_crypto));

    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}