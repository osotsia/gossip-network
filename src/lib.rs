//! src/lib.rs
//!
//! Main library crate for the gossip network application.
//! This file declares the module hierarchy and exports the primary public
//! interface for the library, allowing it to be used by other crates or for
//! integration testing.

// Declare the module hierarchy.
pub mod api;
pub mod app;
pub mod config;
pub mod domain;
pub mod engine;
pub mod error;
pub mod transport;

// Re-export key types for the public API.
pub use app::App;
pub use config::Config;
pub use error::Error;