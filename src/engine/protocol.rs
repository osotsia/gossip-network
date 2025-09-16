//! src/engine/protocol.rs
//!
//! Implements the specific gossip propagation algorithm. By isolating this
//! logic, the protocol can be easily analyzed, tested, and replaced.

use crate::domain::NodeId;
use rand::{seq::SliceRandom, thread_rng};
use std::{collections::HashMap, net::SocketAddr};

/// Selects a random subset of known peers to forward a message to.
///
/// # Arguments
/// * `known_peers` - A map of all peers the node is aware of.
/// * `exclude_originator` - The `NodeId` of the message originator, to prevent sending it back.
/// * `gossip_factor` - The number of peers to select.
pub fn select_peers<'a>(
    known_peers: &'a HashMap<NodeId, SocketAddr>,
    exclude_originator: NodeId,
    gossip_factor: usize,
) -> Vec<(&'a NodeId, &'a SocketAddr)> {
    let mut rng = thread_rng();
    known_peers
        .iter()
        .filter(|(id, _)| **id != exclude_originator)
        .collect::<Vec<_>>()
        .choose_multiple(&mut rng, gossip_factor)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Helper to create a dummy NodeId for testing.
    fn create_node_id(id: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        NodeId(bytes)
    }

    #[test]
    fn test_select_peers_excludes_originator() {
        let originator = create_node_id(1);
        let peer_b = create_node_id(2);
        let peer_c = create_node_id(3);

        let mut peers = HashMap::new();
        peers.insert(originator, SocketAddr::from_str("127.0.0.1:1001").unwrap());
        peers.insert(peer_b, SocketAddr::from_str("127.0.0.1:1002").unwrap());
        peers.insert(peer_c, SocketAddr::from_str("127.0.0.1:1003").unwrap());

        let selected = select_peers(&peers, originator, 5);

        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|(id, _)| **id != originator));
    }

    #[test]
    fn test_select_peers_respects_gossip_factor() {
        let originator = create_node_id(1);
        let mut peers = HashMap::new();
        for i in 2..=10 {
            peers.insert(create_node_id(i), SocketAddr::from_str("127.0.0.1:1000").unwrap());
        }

        let selected = select_peers(&peers, originator, 3);
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_select_peers_with_no_valid_peers() {
        let originator = create_node_id(1);
        let mut peers = HashMap::new();
        peers.insert(originator, SocketAddr::from_str("127.0.0.1:1001").unwrap());

        let selected = select_peers(&peers, originator, 2);
        assert!(selected.is_empty());
    }
}