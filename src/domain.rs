//! src/domain.rs
//!
//! Consolidates core data structures and cryptographic operations. This module
//! is the single source of truth for the application's domain model, merging
//! the concepts of data representation (model) and identity (crypto).

use crate::error::{Error, Result};
use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, fs, io, path::Path};

// --- Cryptographic Identity ---

/// Represents the cryptographic identity of a single node.
#[derive(Debug)]
pub struct Identity {
    keypair: SigningKey,
    pub node_id: NodeId,
}

impl Identity {
    /// Generates a new, random identity.
    pub fn new() -> Self {
        let mut csprng = OsRng;
        // In ed25519-dalek v2, we generate a secret key and derive the signing key from it.
        let mut secret_key_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_key_bytes);
        let keypair = SigningKey::from_bytes(&secret_key_bytes);
        let node_id = NodeId(keypair.verifying_key().to_bytes());
        Self { keypair, node_id }
    }

    /// Loads an identity from a file, or creates a new one if the file doesn't exist.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        match fs::read(path.as_ref()) {
            Ok(bytes) => {
                let keypair_bytes: [u8; 32] =
                    bytes.try_into().map_err(|_| Error::InvalidKeyFile)?;
                let keypair = SigningKey::from_bytes(&keypair_bytes);
                let node_id = NodeId(keypair.verifying_key().to_bytes());
                Ok(Self { keypair, node_id })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let identity = Self::new();
                // Store only the secret part of the keypair.
                fs::write(path.as_ref(), identity.keypair.to_bytes())?;
                Ok(identity)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Signs a data payload, producing a complete `SignedMessage`.
    pub fn sign(&self, message_data: TelemetryData) -> SignedMessage {
        let message_bytes =
            bincode::serialize(&message_data).expect("TelemetryData is serializable");
        let signature = self.keypair.sign(&message_bytes);

        SignedMessage {
            message: message_data,
            originator: self.node_id,
            signature,
        }
    }
}

// --- Domain Models ---

/// A unique identifier for a node, derived from its public key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node::{}", &hex::encode(&self.0[..4]))
    }
}

/// The core data payload gossiped across the network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelemetryData {
    pub timestamp_ms: u64,
    pub value: f64,
}

/// A verifiable unit of information, bundling the data with the originator's
/// identity and a cryptographic signature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignedMessage {
    pub message: TelemetryData,
    pub originator: NodeId,
    pub signature: Signature,
}

impl SignedMessage {
    /// Verifies that the message was authentically signed by the originator.
    pub fn verify(&self) -> Result<()> {
        let public_key = VerifyingKey::from_bytes(self.originator.as_bytes())?;
        let message_bytes = bincode::serialize(&self.message)?;
        public_key.verify(&message_bytes, &self.signature)?;
        Ok(())
    }
}

/// A snapshot of the network state, for use by the visualizer.
#[derive(Clone, Debug, Default, Serialize)]
pub struct NetworkState {
    pub nodes: HashMap<NodeId, TelemetryData>,
}

// --- Unit Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sign_and_verify_ok() {
        let identity = Identity::new();
        let telemetry = TelemetryData { timestamp_ms: 1, value: 1.0 };
        let signed_message = identity.sign(telemetry);
        assert_eq!(signed_message.originator, identity.node_id);
        assert!(signed_message.verify().is_ok());
    }

    #[test]
    fn test_verify_fails_on_tampered_message() {
        let identity = Identity::new();
        let telemetry = TelemetryData { timestamp_ms: 1, value: 1.0 };
        let mut signed_message = identity.sign(telemetry);
        signed_message.message.value = 2.0; // Tamper
        assert!(signed_message.verify().is_err());
    }

    #[test]
    fn test_verify_fails_with_wrong_identity() {
        let identity1 = Identity::new();
        let identity2 = Identity::new();
        let telemetry = TelemetryData { timestamp_ms: 1, value: 1.0 };
        let mut signed_message = identity1.sign(telemetry);
        signed_message.originator = identity2.node_id; // Falsify originator
        assert!(signed_message.verify().is_err());
    }

    #[test]
    fn test_identity_from_file_creates_new() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("test.key");
        assert!(!key_path.exists());
        let identity = Identity::from_file(&key_path).unwrap();
        assert!(key_path.exists());
        let reloaded_identity = Identity::from_file(&key_path).unwrap();
        assert_eq!(identity.node_id, reloaded_identity.node_id);
    }
}