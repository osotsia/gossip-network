# Decentralized Telemetry Gossip Network

A peer-to-peer network built in Rust where nodes exchange signed telemetry data using a gossip protocol over QUIC. This project demonstrates a robust, decentralized, and secure system for asynchronous data propagation, built on modern concurrency patterns with Tokio.

## Features

*   **Persistent Cryptographic Identity:** Nodes use long-term ED25519 keypairs for identity and message signing.
*   **Secure Transport Layer:** All peer-to-peer communication is encrypted using QUIC (TLS 1.3) within a private Public Key Infrastructure (PKI).
*   **Resilient Gossip Protocol:** New information is propagated efficiently and robustly through the network to a random subset of peers, providing high resilience to node failure.
*   **Service-Oriented Architecture:** The application is decomposed into independent, concurrent services (Engine, Transport, API) that communicate via message passing, promoting modularity and testability.
*   **Live Network Visualization:** A Svelte frontend connects to a node's WebSocket to provide a real-time graph visualization of the network state and data flow.

## Quick Start

This project uses an orchestration script to simulate a local cluster.

1.  **Generate Certificates:**
    Follow the instructions in `src/transport/tls.rs` to generate the required `certs/` directory and files.

2.  **Build & Run:**
    The orchestrator builds the project and launches a cluster.

    ```sh
    # Make the script executable (first time only)
    chmod +x orchestrator.sh

    # Run a 10-node cluster with a 30% connection ratio
    ./orchestrator.sh 10 0.3
    ```

3.  **View Visualizer:**
    Navigate to `http://127.0.0.1:8080` in your browser.

## Architecture

The system is designed as a set of isolated, concurrent services communicating via asynchronous channels. For a detailed breakdown of the design, components, and data flow, see [architecture.md](architecture.md).

## FAQ

**Q1: Why use a gossip protocol instead of just broadcasting messages to all known peers?**

A: Broadcasting creates network storms in dense networks (O(N²) message complexity) and is brittle; if a central node fails, the network can be partitioned. Gossip is more scalable and resilient. By sending messages to a small, random subset of peers, it reduces redundant traffic and ensures information can bypass failed nodes, propagating through the network like a rumor.

**Q2: How does this system establish trust between nodes?**

A: Trust is established at two layers. First, at the **transport layer**, QUIC connections are only permitted between nodes that present a TLS certificate signed by a shared, private Certificate Authority (CA). This prevents unauthorized machines from even joining the network. Second, at the **application layer**, every piece of telemetry data is individually signed by the originator's ED25519 private key. This proves data authenticity and integrity, ensuring that a compromised node cannot forge messages on behalf of other nodes.

**Q3: The visualizer shows the state of the whole network. Does this mean one node has perfect, real-time information?**

A: No, and this demonstrates a key principle of distributed systems. The visualizer shows the network state *from the perspective of a single designated node*. This view is subject to **eventual consistency**. Due to network latency, this node's state will always lag slightly behind the true state of the network. Watching the graph allows you to observe this propagation delay in real-time as new nodes appear and values update.

**Q4: If an attacker compromises one node, can they see and attack the entire network?**

A: No. An attacker cannot map the entire network topology. They only discover the IP addresses of the small, random subset of peers they are directly connected to. This partial visibility makes it difficult to launch targeted network-level attacks (e.g., DDoS) against specific, non-adjacent nodes. While the attacker will eventually learn the cryptographic *identities* of all nodes as state propagates, they cannot forge messages (prevented by ED25519 signatures) or decrypt traffic between other honest peers (prevented by QUIC's TLS 1.3 encryption).
