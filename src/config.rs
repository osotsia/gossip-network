//! src/config.rs
//!
//! The Mission Briefing for our application.
//!
//! This module defines a single, strongly-typed `Config` struct that holds all
//! runtime parameters. It uses the `figment` crate to orchestrate loading these
//! parameters from multiple sources (a `config.toml` file, environment variables),
//! providing a clean and flexible configuration system.
//!
//! By centralizing configuration, we ensure that the rest of the application
//! doesn't need to know *where* settings come from, only *what* they are.

use figment::{
    providers::{Format, Toml, Env},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::path::PathBuf;

/// The top-level struct holding all configuration for the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// The path to the file where this node's cryptographic identity is stored.
    pub identity_path: PathBuf,
    /// The IP address and port to bind the P2P networking listener to.
    pub p2p_addr: SocketAddr,
    /// An optional list of bootstrap peers to connect to on startup.
    /// If empty, this node will start in isolation.
    pub bootstrap_peers: Vec<SocketAddr>,
    /// The interval, in milliseconds, at which this node will generate and
    /// gossip its own telemetry data.
    pub gossip_interval_ms: u64,
    /// Configuration for the optional visualizer web server.
    pub visualizer: Option<VisualizerConfig>,
}

/// Configuration specific to the visualizer web server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualizerConfig {
    /// The IP address and port to bind the web server to.
    pub bind_addr: SocketAddr,
}

impl Config {
    /// Loads the application configuration from various sources.
    ///
    // The loading strategy is layered (a "cascade"):
    //   1. Start with `Config::default()` for sane defaults.
    //   2. Merge settings from `config.toml` (if it exists).
    //   3. Merge settings from environment variables (e.g., `APP_P2P_ADDR=...`).
    ///
    /// This cascade allows for easy overrides in different environments (dev vs. prod).
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("GOSSIP_"))
            .extract()
    }
}

/// Provides a default configuration, which is a best practice.
/// This ensures the application can run out-of-the-box without a config file.
impl Default for Config {
    fn default() -> Self {
        Self {
            identity_path: PathBuf::from("identity.key"),
            p2p_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000),
            bootstrap_peers: Vec::new(),
            gossip_interval_ms: 5000,
            visualizer: None, // The visualizer is disabled by default.
        }
    }
}

// --- Unit Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use figment::Jail;
    use std::fs;
    
    // A helper to create a default test config.
    fn test_config() -> Config {
        Config {
            identity_path: PathBuf::from("test.key"),
            p2p_addr: "127.0.0.1:1234".parse().unwrap(),
            bootstrap_peers: vec!["127.0.0.1:5678".parse().unwrap()],
            gossip_interval_ms: 100,
            visualizer: Some(VisualizerConfig {
                bind_addr: "127.0.0.1:8080".parse().unwrap(),
            }),
        }
    }

    #[test]
    fn test_loading_from_file() {
        // Use `figment::Jail` to create a temporary, isolated directory for the test.
        // This is a robust way to test file-based configuration.
        Jail::expect_with(|jail| {
            let config_content = r#"
                identity_path = "test.key"
                p2p_addr = "127.0.0.1:1234"
                bootstrap_peers = ["127.0.0.1:5678"]
                gossip_interval_ms = 100
                [visualizer]
                bind_addr = "127.0.0.1:8080"
            "#;
            jail.create_file("config.toml", config_content)?;

            let config = Config::load()?;
            assert_eq!(config, test_config());

            Ok(())
        });
    }

    #[test]
    fn test_loading_from_env_vars() {
        Jail::expect_with(|jail| {
            // Set environment variables for the duration of this test.
            jail.set_env("GOSSIP_IDENTITY_PATH", "test.key");
            jail.set_env("GOSSIP_P2P_ADDR", "127.0.0.1:1234");
            // Note: Figment can parse complex types from strings for env vars.
            jail.set_env("GOSSIP_BOOTSTRAP_PEERS", r#"["127.0.0.1:5678"]"#);
            jail.set_env("GOSSIP_GOSSIP_INTERVAL_MS", "100");
            jail.set_env("GOSSIP_VISUALIZER.BIND_ADDR", "127.0.0.1:8080");

            let config = Config::load()?;
            assert_eq!(config, test_config());

            Ok(())
        });
    }
    
    #[test]
    fn test_env_overrides_file() {
        Jail::expect_with(|jail| {
            // Create a file with one value.
            let config_content = r#"
                p2p_addr = "1.1.1.1:1111"
            "#;
            jail.create_file("config.toml", config_content)?;

            // Override it with an environment variable.
            jail.set_env("GOSSIP_P2P_ADDR", "127.0.0.1:9999");
            
            let config = Config::load()?;

            // The env var should win.
            assert_eq!(config.p2p_addr, "127.0.0.1:9999".parse().unwrap());

            Ok(())
        });
    }
}