//! src/domain.rs
//!
//! Consolidates core data structures and cryptographic operations. This module
//! is the single source of truth for the application's domain model, merging
//! the concepts of data representation (model) and identity (crypto).

use crate::error::{Error, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::{rngs::OsRng, RngCore};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::{HashMap},
    fmt, fs, io,
    path::Path,
};
// --- Cryptographic Identity ---
#[derive(Debug, Clone)] // MODIFICATION: Added Clone
pub struct Identity {
    keypair: SigningKey,
    pub node_id: NodeId,
}

impl Identity {
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let mut secret_key_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_key_bytes);
        let keypair = SigningKey::from_bytes(&secret_key_bytes);
        let node_id = NodeId(keypair.verifying_key().to_bytes());
        Self { keypair, node_id }
    }

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
                fs::write(path.as_ref(), identity.keypair.to_bytes())?;
                Ok(identity)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn sign(&self, message_data: GossipPayload) -> SignedMessage {
        let message_bytes =
            bincode::serialize(&message_data).expect("GossipPayload is serializable");
        let signature = self.keypair.sign(&message_bytes);

        SignedMessage {
            message: message_data,
            originator: self.node_id,
            signature,
        }
    }
}

// --- Domain Models ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct NodeId(pub [u8; 32]);

impl Serialize for NodeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes_vec = hex::decode(s).map_err(D::Error::custom)?;
        let bytes: [u8; 32] = bytes_vec
            .try_into()
            .map_err(|_| D::Error::custom("Invalid hex string length for NodeId"))?;
        Ok(NodeId(bytes))
    }
}

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelemetryData {
    pub timestamp_ms: u64,
    pub value: f64,
}

/// The data payload that is signed and gossiped across the network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GossipPayload {
    pub telemetry: TelemetryData,
    pub community_id: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignedMessage {
    pub message: GossipPayload,
    pub originator: NodeId,
    pub signature: Signature,
}

impl SignedMessage {
    pub fn verify(&self) -> Result<()> {
        let public_key = VerifyingKey::from_bytes(self.originator.as_bytes())?;
        let message_bytes = bincode::serialize(&self.message)?;
        public_key.verify(&message_bytes, &self.signature)?;
        Ok(())
    }
}

/// Information about a node, as held by the Engine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeInfo {
    pub telemetry: TelemetryData,
    pub community_id: u32,
}

/// A snapshot of the network state, for use by the visualizer.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NetworkState {
    pub self_id: Option<NodeId>,
    pub nodes: HashMap<NodeId, NodeInfo>,
    pub active_connections: Vec<NodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A helper struct to bundle an identity with a convenient way to sign messages.
    struct TestPeer {
        identity: Identity,
    }

    impl TestPeer {
        fn new() -> Self {
            Self {
                identity: Identity::new(),
            }
        }

        fn sign(&self, timestamp_ms: u64) -> SignedMessage {
            let payload = GossipPayload {
                telemetry: TelemetryData { timestamp_ms, value: 42.0 },
                community_id: 1,
            };
            self.identity.sign(payload)
        }
    }

    #[test]
    fn signature_verification_succeeds_for_valid_message() {
        let peer = TestPeer::new();
        let message = peer.sign(1000);
        assert!(message.verify().is_ok());
    }

    #[test]
    fn signature_verification_fails_for_tampered_payload() {
        let peer = TestPeer::new();
        let mut message = peer.sign(1000);

        // Mutate the payload after signing.
        message.message.telemetry.value = 999.0;

        assert!(message.verify().is_err());
    }

    #[test]
    fn signature_verification_fails_for_wrong_originator() {
        let peer_a = TestPeer::new();
        let peer_b = TestPeer::new(); // A different identity.
        let mut message = peer_a.sign(1000);

        // Attribute the message to a different peer without re-signing.
        message.originator = peer_b.identity.node_id;

        assert!(message.verify().is_err());
    }

    #[test]
    fn signature_verification_fails_for_corrupted_signature() {
        let peer = TestPeer::new();
        let mut message = peer.sign(1000);

        // Flip a bit in the signature.
        let mut sig_bytes = message.signature.to_bytes();
        sig_bytes[0] ^= 0xff;
        message.signature = Signature::from_bytes(&sig_bytes);

        assert!(message.verify().is_err());
    }
}