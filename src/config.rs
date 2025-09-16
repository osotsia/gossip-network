//! src/config.rs
//!
//! Defines the strongly-typed `Config` struct for all runtime parameters,
//! loaded from files and environment variables via `figment`.

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Top-level struct holding all application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub identity_path: PathBuf,
    pub p2p_addr: SocketAddr,
    pub bootstrap_peers: Vec<SocketAddr>,
    pub gossip_interval_ms: u64,
    pub gossip_factor: usize,
    pub node_ttl_ms: u64,
    pub community_id: u32,
    pub visualizer: Option<VisualizerConfig>,
}

/// Configuration for the optional visualizer web server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualizerConfig {
    pub bind_addr: SocketAddr,
}

impl Config {
    /// Loads configuration from `config.toml` and environment variables.
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("GOSSIP_"))
            .extract()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            identity_path: PathBuf::from("identity.key"),
            p2p_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000),
            bootstrap_peers: Vec::new(),
            gossip_interval_ms: 5000,
            gossip_factor: 2,
            node_ttl_ms: 300000, // 5 minutes
            community_id: 0,
            visualizer: None,
        }
    }
}