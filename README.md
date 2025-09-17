# Decentralized Telemetry Gossip Network

A peer-to-peer network built in Rust where nodes exchange signed telemetry data using a gossip protocol over QUIC. This project demonstrates a robust, decentralized, and secure system for asynchronous data propagation, built on modern concurrency patterns with Tokio.

## Features

*   **Persistent Cryptographic Identity:** Nodes use long-term ED25519 keypairs for identity and message signing.
*   **Secure Transport Layer:** All peer-to-peer communication is encrypted using QUIC (TLS 1.3) within a private Public Key Infrastructure (PKI).
*   **Resilient Gossip Protocol:** New information is propagated efficiently and robustly through the network to a random subset of peers, providing high resilience to node failure.
*   **Service-Oriented Architecture:** The application is decomposed into independent, concurrent services (Engine, Transport, API) that communicate via message passing, promoting modularity and testability.
*   **Live Network Visualization:** A Svelte frontend connects to a node's WebSocket to provide a real-time graph visualization of the network state and data flow.


## Quick Start

This project uses a comprehensive orchestration script to build the application, generate a private Public Key Infrastructure (PKI), and simulate a local cluster with a configurable topology.

### Prerequisites

Ensure the following command-line tools are installed and available in your `PATH`:

*   **Rust Toolchain:** (e.g., `cargo`, `rustc`)
*   **Node.js & npm:** For building the frontend
*   **Go:** For installing `minica`
*   **OpenSSL:** For certificate conversion
*   **minica:** A simple CA management tool.

### 1. Build the Frontend

The orchestrator serves the pre-built static assets for the visualizer.

```sh
# Navigate to the frontend directory
cd frontend

# Install dependencies
npm install

# Build the static site
npm run build

# Return to the project root
cd ..
```

### 2. Run the Orchestrator

The script handles the entire cluster deployment process. It will:
1.  Build the Rust binary in release mode.
2.  Automatically generate a root CA and unique TLS certificates for each node if they don't exist.
3.  Create a `cluster/` directory with unique configurations for each node.
4.  Launch all node processes in the background.

To run the script, provide the number of nodes, number of communities, and the connection ratios for intra-community and inter-community peers.

```sh
# Make the script executable (first time only)
chmod +x orchestrator.sh

# Example: Run a 15-node cluster with 3 communities.
# Each node connects to 80% of its own community members and 10% of others.
./orchestrator.sh 15 3 0.8 0.1
```

### 3. View the Visualizer

Once the cluster is running, open your browser and navigate to the address of the designated visualizer node (`node-0`).

*   URL: `http://127.0.0.1:8080`

### 4. Stop the Cluster

To shut down all node processes gracefully, press `Ctrl+C` in the terminal where `orchestrator.sh` is running.

## Architecture

The system is designed as a set of isolated, concurrent services communicating via asynchronous channels. For a detailed breakdown of the design, components, and data flow, see the documentation in docs/.

## FAQ

**Q1: Why use a gossip protocol instead of just broadcasting messages to all known peers?**

A: Broadcasting creates network storms in dense networks (O(N²) message complexity) and is brittle; if a central node fails, the network can be partitioned. Gossip is more scalable and resilient. By sending messages to a small, random subset of peers, it reduces redundant traffic and ensures information can bypass failed nodes, propagating through the network like a rumor.

**Q2: How does this system establish trust between nodes?**

A: Trust is established at two layers. First, at the **transport layer**, QUIC connections are only permitted between nodes that present a TLS certificate signed by a shared, private Certificate Authority (CA). This prevents unauthorized machines from even joining the network. Second, at the **application layer**, every piece of telemetry data is individually signed by the originator's ED25519 private key. This proves data authenticity and integrity, ensuring that a compromised node cannot forge messages on behalf of other nodes.

**Q3: The visualizer shows the state of the whole network. Does this mean one node has perfect, real-time information?**

A: No, and this demonstrates a key principle of distributed systems. The visualizer shows the network state *from the perspective of a single designated node*. This view is subject to **eventual consistency**. Due to network latency, this node's state will always lag slightly behind the true state of the network. Watching the graph allows you to observe this propagation delay in real-time as new nodes appear and values update.

**Q4: If an attacker compromises one node, can they see and attack the entire network?**

A: No. An attacker cannot map the entire network topology. They only discover the IP addresses of the small, random subset of peers they are directly connected to. This partial visibility makes it difficult to launch targeted network-level attacks (e.g., DDoS) against specific, non-adjacent nodes. While the attacker will eventually learn the cryptographic *identities* of all nodes as state propagates, they cannot forge messages (prevented by ED25519 signatures) or decrypt traffic between other honest peers (prevented by QUIC's TLS 1.3 encryption).
