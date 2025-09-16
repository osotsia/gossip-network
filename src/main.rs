//! src/main.rs
//!
//! Binary entry point. Responsible for initializing tracing, loading
//! configuration, instantiating the main `App`, and running it.

use anyhow::Context;
use gossip_network::{App, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize the tracing subscriber.
    // RUST_LOG=info will be the default.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load configuration.
    let config = Config::load().context("Failed to load configuration")?;

    // Create and run the application.
    if let Err(e) = App::new(config)?.run().await {
        tracing::error!(error = %e, "💥 Application failed");
        std::process::exit(1);
    }

    Ok(())
}