//! src/main.rs
//!
//! The entrypoint for the gossip network binary.
//!
//! This file is intentionally minimal. Its sole responsibilities are:
//!   - Setting up the asynchronous `tokio` runtime.
//!   - Initializing the logger.
//!   - Calling the main `run` function from our library crate.

use gossip_network::run;

#[tokio::main]
async fn main() {
    // Initialize the logger. `RUST_LOG=info` will print all info-level logs
    // and below (warn, error). Use `RUST_LOG=debug` or `RUST_LOG=trace` for more detail.
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    if let Err(e) = run().await {
        log::error!("💥 Application failed to run: {}", e);
        std::process::exit(1);
    }
}