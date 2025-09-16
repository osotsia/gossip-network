//! tests/common/mod.rs
//!
//! Shared utilities for integration tests.

use gossip_network::{config::VisualizerConfig, Config};
use std::{net::SocketAddr, path::PathBuf};
use tempfile::{tempdir, TempDir};

/// Sets up a node's configuration for testing.
///
/// Creates a temporary directory for the identity key and uses ephemeral ports
/// to avoid conflicts during parallel test runs.
pub fn setup_test_config(dir: &TempDir, bootstrap_peers: Vec<SocketAddr>) -> Config {
    Config {
        identity_path: dir.path().join("identity.key"),
        // Use port 0 to let the OS assign an ephemeral port.
        p2p_addr: "127.0.0.1:0".parse().unwrap(),
        bootstrap_peers,
        gossip_interval_ms: 250, // Use a short interval for faster test execution.
        gossip_factor: 2,
        node_ttl_ms: 5000, // Short TTL for testing pruning.
        visualizer: Some(VisualizerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
        }),
    }
}