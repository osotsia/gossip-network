# System Architecture

## 1. Architectural Philosophy

This system is designed around three core principles: a Service-Oriented Architecture, Domain-Centric Design, and the Actor Model for concurrency.

*   **Service-Oriented Architecture (SOA):** The application is decomposed into independent, long-running services, each with a single, well-defined responsibility. The primary services are the `Engine` (core logic), `Transport` (networking), and `ApiServer` (web interface). These services do not share memory and communicate exclusively through asynchronous message passing via channels. This design promotes loose coupling, making services easier to test, maintain, and reason about in isolation.

*   **Domain-Centric Design:** The core concepts, data structures, and business rules of the application are isolated in a `domain` module. This module is self-contained and has no dependencies on infrastructure concerns like networking or web frameworks. The cryptographic identity of a node is treated as a fundamental part of the domain, not as an external utility. This separation ensures the core logic is pure, portable, and highly testable.

*   **Actor Model:** Each service is implemented as an actor running in its own lightweight Tokio task. An actor owns its state, processes messages from an inbox (an `mpsc` channel), and sends messages to other actors. This model provides a robust framework for managing concurrency and state, eliminating the need for complex locks or other manual synchronization primitives at the service level.

## 2. Folder and File Structure

The project is organized into modules that reflect the service-oriented architecture.

```
gossip-network/
├── Cargo.toml
├── config.toml
├── certs/
│   ├── ca.cert
│   ├── node.cert
│   └── node.key
├── frontend/
│   └── (Svelte project files)
├── src/
│   ├── main.rs         # Binary entry point. Instantiates and runs `App`.
│   ├── lib.rs          # Public library facade. Exports `App`, `config`, `error`.
│   │
│   ├── app.rs          # Defines the main `App` struct. Manages service lifecycle and orchestration.
│   ├── config.rs       # Configuration loading and data structures.
│   ├── domain.rs       # Core data types, cryptographic identity, and operations.
│   ├── error.rs        # Custom, typed error enum for the library using `thiserror`.
│   │
│   ├── engine/         # Core application logic and state management.
│   │   ├── mod.rs      # Defines and runs the `Engine` service/actor. Owns state.
│   │   └── protocol.rs # Implements the gossip propagation algorithm.
│   │
│   ├── transport/      # P2P network transport layer (QUIC).
│   │   ├── mod.rs      # Defines and runs the `Transport` service/actor. Manages the QUIC endpoint.
│   │   ├── connection.rs # Connection caching, establishment, and stream handling logic.
│   │   └── tls.rs      # TLS configuration.
│   │
│   └── api/            # External API for the web visualizer.
│       ├── mod.rs      # Defines and runs the `ApiServer` service. Sets up Axum routes.
│       └── ws.rs       # WebSocket connection and state broadcasting logic.
│
└── tests/
    ├── common/
    │   └── mod.rs      # Shared test utilities (e.g., fixture generation, mock services).
    └── integration/
        └── network.rs  # Full end-to-end tests with multiple live nodes.

```

## 3. Core Components (Services)

The application's runtime is composed of three primary services orchestrated by the `App` struct.

### `engine` Service
The `Engine` is the brain of a node. It encapsulates the application's core logic and state.

*   **Responsibilities:**
    *   Maintaining the node's view of the network state (a map of all known nodes and their latest telemetry).
    *   Periodically generating this node's own signed telemetry data.
    *   Processing validated inbound messages from the `Transport` service.
    *   Applying the gossip protocol to decide which peers to forward new information to.
    *   Publishing state changes for consumption by the `ApiServer`.
*   **Inputs:** Receives `InboundMessage` objects from the `Transport` service via an `mpsc` channel.
*   **Outputs:** Sends `TransportCommand` objects to the `Transport` service via an `mpsc` channel. Broadcasts `NetworkState` updates via a `watch` channel.

### `transport` Service
The `Transport` service is the node's interface to the network. It handles all low-level peer-to-peer communication over QUIC.

*   **Responsibilities:**
    *   Binding a QUIC endpoint to a network socket.
    *   Establishing and accepting secure peer connections using a private TLS certificate authority.
    *   Managing a connection cache to reuse existing connections for performance.
    *   Serializing outbound messages into bytes and deserializing inbound bytes into messages.
    *   Abstracting all network I/O complexity from the `Engine`.
*   **Inputs:** Receives `TransportCommand` objects (e.g., `SendMessage`) from the `Engine` via an `mpsc` channel.
*   **Outputs:** Sends validated and deserialized `InboundMessage` objects to the `Engine` via an `mpsc` channel.

### `api` Service
The `ApiServer` provides the HTTP and WebSocket interface for the web-based visualizer. It is a read-only component.

*   **Responsibilities:**
    *   Serving the static frontend application files (HTML, CSS, JS).
    *   Accepting WebSocket connections from clients.
    *   Sending a full snapshot of the current network state to newly connected clients.
    *   Broadcasting incremental state updates to all connected WebSocket clients.
*   **Inputs:** Subscribes to `NetworkState` updates from the `Engine` via a `watch` channel.
*   **Outputs:** Sends serialized JSON data over WebSocket connections.

## 4. Key Supporting Modules

*   **`domain.rs`**: The lingua franca of the system. It contains the core data structures (`NodeId`, `TelemetryData`, `SignedMessage`) and consolidates cryptographic identity management (`Identity`, signing, verification). This module has zero dependencies on infrastructure, making it the most stable and central part of the application.
*   **`app.rs`**: The application orchestrator. The `App` struct is responsible for initializing all services, wiring their communication channels together, and managing graceful shutdown.
*   **`error.rs`**: Defines a comprehensive, typed `Error` enum for the entire library. Using `thiserror`, it provides clear, structured error handling, which is crucial for building a robust and maintainable system.

## 5. Data Flow: Lifecycle of a Gossiped Message

The interaction between services is best understood by tracing a message through the system.

1.  **Generation (Node A):** A periodic timer in Node A's `Engine` fires. The `Engine` creates a `TelemetryData` payload, signs it using its `Identity` to produce a `SignedMessage`, and updates its own local state.
2.  **Command (Node A):** The `Engine` wraps the `SignedMessage` and the target peer's address (Node B) in a `TransportCommand::SendMessage` and sends it to its `Transport` service.
3.  **Transmission (Node A):** The `Transport` service receives the command. It retrieves a cached QUIC connection to Node B (or establishes a new one), serializes the `SignedMessage` using `bincode`, and writes the bytes to a network stream.
4.  **Reception (Node B):** Node B's `Transport` service receives the bytes from the QUIC stream. It deserializes them back into a `SignedMessage`.
5.  **Forwarding (Node B):** The `Transport` service wraps the message in an `InboundMessage` (which includes Node A's network address) and sends it to its `Engine`.
6.  **Processing (Node B):** Node B's `Engine` receives the `InboundMessage`. It first performs cryptographic verification on the `SignedMessage`. It then checks its internal state to see if the message contains newer information than what it already knows about Node A.
7.  **Propagation (Node B):** If the information is new, the `Engine` updates its state, publishes the new `NetworkState` to its `ApiServer`, and then invokes the `protocol::select_peers` logic to choose a subset of its *other* peers (e.g., Node C) to forward the original `SignedMessage` to, repeating the cycle from Step 2.

## 6. Testing Strategy

The architecture supports a multi-layered testing strategy:

*   **Unit Tests:** Placed directly within modules (`#[cfg(test)]`), these test pure, stateless logic. Examples include cryptographic operations in `domain.rs` and the peer selection algorithm in `engine/protocol.rs`. They are fast and require no I/O.
*   **Integration Tests:** Located in the `tests/integration/` directory, these tests validate the behavior of a complete service or the interaction between multiple services. The `network.rs` test is an end-to-end test that spins up multiple full application instances in separate threads, configures them to connect to each other, and verifies correct state propagation over a real (local) network. This provides the highest level of confidence in the system's correctness.

## 7. Multi-Node Deployment and Orchestration

The architecture described above details the components of a single node. A network is formed by running multiple instances of this application, orchestrated as independent processes. The system does not possess an intrinsic "cluster mode"; network topology is an emergent property of configuration.

### External Orchestration
The recommended method for local development and testing of a multi-node network is through an external orchestration script (e.g., Bash, Python).

*   **Responsibilities of the Script:**
    *   **Configuration Generation:** For a cluster of `N` nodes, the script generates `N` unique configuration files (e.g., `config-0.toml`, `config-1.toml`, ...). Each file is assigned a distinct `p2p_addr`.
    *   **Topology Definition:** The script calculates a `bootstrap_peers` list for each node based on a specified connection ratio `R`. This allows for the creation of partially-connected or fully-connected mesh networks.
    *   **Process Management:** The script launches `N` instances of the `gossip-network` binary, each with its corresponding configuration.

### Visualizer Integration
To integrate the web visualizer in a multi-node environment, a **Designated Visualizer Node** pattern is employed.

*   **Mechanism:** The orchestration script enables the `api` service on only one specific node (e.g., `node-0`) by including the `[visualizer]` section in its configuration file. This node's visualizer is bound to a static, predictable address (e.g., `127.0.0.1:8080`).
*   **Frontend Connection:** The Svelte application is configured to connect its WebSocket to this static address. It therefore receives and displays the network state as seen from the perspective of the designated visualizer node.
*   **Analysis:** This approach requires no modification to the frontend application, ensuring simplicity. The trade-off is that visualization becomes dependent on the liveness of this single designated node. The failure of the visualizer node does not impact the health or operation of the underlying gossip network itself.