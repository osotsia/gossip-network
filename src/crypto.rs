//! src/crypto.rs
//!
//! The Notary Public of our system.
//!
//! This module abstracts all cryptographic operations, providing a clean and safe
//! interface for handling digital identities. It is responsible for:
//!   - Generating cryptographic keypairs.
//!   - Loading and saving identities to disk.
//!   - Signing messages to prove authenticity.
//!   - Verifying signatures to validate incoming messages.
//!
//! By isolating this logic, we can easily swap out the underlying cryptographic
//! library or algorithms in the future without changing the rest of the application.

use crate::model::{NodeId, SignedMessage, TelemetryData};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature, Verifier};
use rand::rngs::OsRng;
use std::fs;
use std::io;
use std::path::Path;

/// Represents the cryptographic identity of a single node.
/// It holds the keypair needed to sign outgoing messages and a `NodeId`
/// (public key) to identify itself to others.
#[derive(Debug)]
pub struct Identity {
    keypair: SigningKey,
    pub node_id: NodeId,
}

impl Identity {
    /// Generates a new, random identity.
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let keypair = SigningKey::generate(&mut csprng);
        let node_id = NodeId::new(keypair.verifying_key().to_bytes());
        Self { keypair, node_id }
    }

    /// Loads an identity from a file, or creates a new one if the file doesn't exist.
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        // ... (this function's implementation remains unchanged) ...
        match fs::read(path.as_ref()) {
            Ok(bytes) => {
                let keypair_bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Invalid key file length")
                })?;

                let keypair = SigningKey::from_bytes(&keypair_bytes);
                let node_id = NodeId::new(keypair.verifying_key().to_bytes());
                Ok(Self { keypair, node_id })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let identity = Self::new();
                fs::write(path.as_ref(), identity.keypair.to_bytes())?;
                Ok(identity)
            }
            Err(e) => Err(e),
        }
    }

    /// --- IMPROVEMENT 3: Correct Cryptographic Signing ---
    /// Signs a data payload, producing a complete and valid `SignedMessage`.
    ///
    /// This function now takes the actual data to be signed (`TelemetryData`) as
    /// input, rather than a mutable `SignedMessage`. This is a safer pattern that
    /// prevents accidental or malicious mutation. It constructs and returns a
    /// new, fully-formed `SignedMessage`.
    pub fn sign(&self, message_data: TelemetryData) -> SignedMessage {
        // The signature is calculated over the canonical byte representation of the data.
        // We use bincode for a compact and stable binary format.
        let message_bytes =
            bincode::serialize(&message_data).expect("TelemetryData should be serializable");
        let signature = self.keypair.sign(&message_bytes);

        SignedMessage {
            message: message_data,
            originator: self.node_id,
            signature: signature.to_bytes(),
        }
    }
}

/// --- IMPROVEMENT 3: Correct Cryptographic Verification ---
/// Verifies that a `SignedMessage` is authentic.
///
/// This function now correctly checks that the signature on the `message` field
/// was produced by the key corresponding to the `originator` field. An attacker
/// cannot change the originator without invalidating the signature.
pub fn verify(message: &SignedMessage) -> Result<(), &'static str> {
    let public_key_bytes = message.originator.as_bytes();
    let verifying_key =
        VerifyingKey::from_bytes(public_key_bytes).map_err(|_| "Invalid public key bytes")?;

    let signature =
        Signature::from_bytes(&message.signature).map_err(|_| "Invalid signature format")?;

    // We must serialize the inner message here to get the exact byte string that was signed.
    let message_bytes =
        bincode::serialize(&message.message).map_err(|_| "Failed to serialize message for verification")?;

    verifying_key
        .verify(&message_bytes, &signature)
        .map_err(|_| "Signature verification failed")
}


// --- Unit Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TelemetryData;
    use tempfile::tempdir;

    fn create_test_telemetry() -> TelemetryData {
        TelemetryData {
            timestamp_ms: 12345,
            value: 99.9,
        }
    }

    #[test]
    fn test_sign_and_verify_ok() {
        let identity = Identity::new();
        let telemetry = create_test_telemetry();
        
        // Sign the data payload.
        let signed_message = identity.sign(telemetry);

        // The originator should be set correctly.
        assert_eq!(signed_message.originator, identity.node_id);
        
        // Verification should succeed.
        assert!(verify(&signed_message).is_ok());
    }

    #[test]
    fn test_verify_fails_on_tampered_message() {
        let identity = Identity::new();
        let telemetry = create_test_telemetry();
        
        let mut signed_message = identity.sign(telemetry);

        // Tamper with the data after it has been signed.
        signed_message.message.value = 0.0;
        
        // Verification must fail.
        assert!(verify(&signed_message).is_err());
    }

    #[test]
    fn test_verify_fails_with_wrong_identity() {
        let identity1 = Identity::new();
        let identity2 = Identity::new(); // A different node
        let telemetry = create_test_telemetry();
        
        let mut signed_message = identity1.sign(telemetry);

        // Falsify the originator. The signature was made by identity1, but we claim
        // it was made by identity2.
        signed_message.originator = identity2.node_id;
        
        // Verification must fail because the signature does not match the claimed originator.
        assert!(verify(&signed_message).is_err());
    }

    #[test]
    fn test_identity_from_file_creates_new() {
        // ... (this test remains unchanged) ...
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("test_key.bin");
        assert!(!key_path.exists());
        let identity = Identity::from_file(&key_path).unwrap();
        assert!(key_path.exists());
        let key_bytes = fs::read(&key_path).unwrap();
        assert_eq!(key_bytes.len(), 32);
        let reloaded_identity = Identity::from_file(&key_path).unwrap();
        assert_eq!(identity.node_id, reloaded_identity.node_id);
    }
}