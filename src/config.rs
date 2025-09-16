//! src/config.rs
//!
//! Defines the strongly-typed `Config` struct for all runtime parameters,
//! loaded from files and environment variables via `figment`.

use figment::{
    providers::{Env, Format, Serialized, Toml},
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
    pub visualizer: Option<VisualizerConfig>,
}

/// Configuration for the optional visualizer web server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualizerConfig {
    pub bind_addr: SocketAddr,
}

impl Config {
    /// Loads configuration from `config.toml` and environment variables.
    /// It uses the `Default` implementation as a base layer.
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Serialized::defaults(Config::default()))
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
            visualizer: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::Jail;

    fn test_config() -> Config {
        Config {
            identity_path: PathBuf::from("test.key"),
            p2p_addr: "127.0.0.1:1234".parse().unwrap(),
            bootstrap_peers: vec!["127.0.0.1:5678".parse().unwrap()],
            gossip_interval_ms: 100,
            gossip_factor: 3,
            node_ttl_ms: 60000,
            visualizer: Some(VisualizerConfig {
                bind_addr: "127.0.0.1:8080".parse().unwrap(),
            }),
        }
    }

    #[test]
    fn test_loading_from_file() {
        Jail::expect_with(|jail| {
            let config_content = r#"
                identity_path = "test.key"
                p2p_addr = "127.0.0.1:1234"
                bootstrap_peers = ["127.0.0.1:5678"]
                gossip_interval_ms = 100
                gossip_factor = 3
                node_ttl_ms = 60000
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
    fn test_env_overrides_file() {
        Jail::expect_with(|jail| {
            let config_content = r#"p2p_addr = "1.1.1.1:1111""#;
            jail.create_file("config.toml", config_content)?;
            jail.set_env("GOSSIP_P2P_ADDR", "127.0.0.1:9999");
            let config = Config::load()?;
            assert_eq!(config.p2p_addr, "127.0.0.1:9999".parse().unwrap());
            Ok(())
        });
    }
}