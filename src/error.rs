//! src/error.rs
//!
//! Defines the library's custom, comprehensive `Error` enum using `thiserror`.

use std::net::SocketAddr;
use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] figment::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to serialize or deserialize: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Cryptography error: {0}")]
    Crypto(#[from] ed25519_dalek::SignatureError),

    #[error("Invalid identity key file")]
    InvalidKeyFile,

    #[error("Tokio task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("Tokio runtime error: {0}")]
    TokioRuntime(#[from] tokio::runtime::InitError),

    #[error("TLS configuration error: {0}")]
    TlsConfig(String),

    #[error("Network connection to {0} failed: {1}")]
    ConnectionFailed(SocketAddr, #[source] quinn::ConnectError),

    #[error("Failed to establish connection: {0}")]
    ConnectionError(#[from] quinn::ConnectionError),

    #[error("Failed to write to network stream: {0}")]
    WriteStream(#[from] quinn::WriteError),

    #[error("API server error: {0}")]
    ApiServer(#[from] axum::Error),
}