//! src/model.rs
//!
//! The Lingua Franca of our system.
//!
//! This module defines the core, shared data structures (or "models") that are
//! passed between the different components of the application. By centralizing them
//! here, we avoid circular dependencies and create a single source of truth for our
//! application's data.

use serde::{Deserialize, Serialize};
use std::fmt;

// A type alias for a Node's unique identifier, which is its public key.
// Using a type alias makes the code more readable and self-documenting.
// We use a wrapper struct to enforce type safety and allow for custom implementations.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash, // Essential for use as a key in HashMaps
    Serialize,
    Deserialize,
)]
pub struct NodeId([u8; 32]);

impl NodeId {
    /// Creates a new NodeId from a 32-byte public key.
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the inner byte array.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

// A custom display implementation to show a shortened, human-readable version of the NodeId.
// This is invaluable for logging and visualization.
impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node::{}", &hex::encode(&self.0[..4]))
    }
}

/// Represents the actual data being gossiped across the network.
/// This is the canonical data payload that will be cryptographically signed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelemetryData {
    pub timestamp_ms: u64,
    pub value: f64,
}

/// A wrapper that bundles any message with its originator's identity and a signature.
/// This is the fundamental, verifiable unit of information that gets passed around
/// the gossip network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignedMessage {
    /// The core data being transmitted.
    pub message: TelemetryData,

    /// The public key of the node that created and signed this message.
    pub originator: NodeId,

    /// The cryptographic signature of the serialized `message` field.
    pub signature: [u8; 64],
}

// --- Unit Tests ---
// Tests are placed in the same file to keep them close to the code they're testing.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_display() {
        let bytes = [10u8; 32]; // 0x0a0a0a0a...
        let node_id = NodeId::new(bytes);
        assert_eq!(node_id.to_string(), "node::0a0a0a0a");
    }

    #[test]
    fn test_telemetry_serialization_is_stable() {
        // This test ensures that the byte representation we sign is consistent.
        // We now use bincode for a canonical, compact binary format.
        let msg1 = TelemetryData {
            timestamp_ms: 100,
            value: 123.45,
        };
        let msg2 = TelemetryData {
            timestamp_ms: 100,
            value: 123.45,
        };

        let bytes1 = bincode::serialize(&msg1).unwrap();
        let bytes2 = bincode::serialize(&msg2).unwrap();

        assert_eq!(bytes1, bytes2);
    }
}