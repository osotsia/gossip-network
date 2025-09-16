#!/bin/bash
#
# orchestrator.sh - Deploys a local gossip network cluster.
#
# This script automates the setup and launch of N gossip-network nodes,
# creating a partially or fully connected mesh network for testing and
# demonstration purposes.
#
# -----------------------------------------------------------------------------
# HOW TO USE
# -----------------------------------------------------------------------------
# 1. MAKE EXECUTABLE (first time only):
#    chmod +x orchestrator.sh
#
# 2. RUN THE SCRIPT:
#    Provide the number of nodes and a connection ratio (0.0 to 1.0).
#
#    Usage:
#      ./orchestrator.sh <NUM_NODES> <CONNECTION_RATIO>
#
#    Examples:
#      # Launch a dense 5-node network (each connects to ~80% of peers)
#      ./orchestrator.sh 5 0.8
#
#      # Launch a sparse 20-node network (each connects to ~10% of peers)
#      ./orchestrator.sh 20 0.1
#
# 3. VIEW THE VISUALIZER:
#    Once the cluster is running, open your browser to:
#      http://127.0.0.1:8080
#
# 4. STOP THE CLUSTER:
#    Press Ctrl+C in the terminal where the script is running.
# -----------------------------------------------------------------------------

# --- Script Configuration ---
set -e  # Exit immediately if a command exits with a non-zero status.
set -u  # Treat unset variables as an error.
set -o pipefail # Causes a pipeline to return the exit status of the last command that returned a non-zero status.

# --- Parameters ---
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <NUM_NODES> <CONNECTION_RATIO>"
    exit 1
fi

NUM_NODES=$1
CONNECTION_RATIO=$2
CLUSTER_DIR="cluster"
BASE_P2P_PORT=5000
VISUALIZER_PORT=8080

# Array to hold process IDs of launched nodes for cleanup
PIDS=()

# --- Cleanup Function ---
# This function is called on script exit to ensure all child processes are terminated.
cleanup() {
    echo ""
    echo "--- Shutting down cluster ---"
    # Kill the blocking process if it exists
    if [ -n "${BLOCKER_PID-}" ]; then
        kill "$BLOCKER_PID" 2>/dev/null || true
    fi

    if [ ${#PIDS[@]} -ne 0 ]; then
        # Kill all child processes gracefully. The `|| true` prevents errors if a process is already dead.
        kill "${PIDS[@]}" 2>/dev/null || true
        echo "All node processes have been terminated."
    else
        echo "No processes to terminate."
    fi
    # Optional: remove the generated cluster directory
    # rm -rf "$CLUSTER_DIR"
    # echo "Cluster directory '$CLUSTER_DIR' cleaned up."
}
trap cleanup EXIT

# --- Pre-flight Checks ---
echo "--- Preparing environment ---"

# 1. Check for the 'certs' directory.
if [ ! -d "certs" ] || [ ! -f "certs/ca.cert" ]; then
    echo "Error: 'certs' directory with required TLS certificates not found."
    echo "Please run the certificate generation steps outlined in the documentation."
    exit 1
fi

# 2. Check for the frontend build directory.
if [ ! -d "frontend/dist" ]; then
    echo "Error: 'frontend/dist' directory not found."
    echo "Please build the Svelte frontend first (e.g., cd frontend && npm install && npm run build)."
    exit 1
fi

# 3. Build the Rust binary in release mode for performance.
echo "Building gossip-network binary..."
cargo build --release

# 4. Clean up and create the directory for generated configurations.
echo "Creating cluster directory: $CLUSTER_DIR"
rm -rf "$CLUSTER_DIR"
mkdir -p "$CLUSTER_DIR"

# --- Phase 1: Generate Node Configurations and Discover Addresses ---
echo "--- Generating node configurations (Phase 1) ---"
P2P_ADDRESSES=()
for i in $(seq 0 $((NUM_NODES - 1))); do
    NODE_DIR="$CLUSTER_DIR/node-$i"
    CONFIG_PATH="$NODE_DIR/config.toml"
    P2P_PORT=$((BASE_P2P_PORT + i))
    P2P_ADDRESSES+=("127.0.0.1:$P2P_PORT")

    mkdir -p "$NODE_DIR"

    # Create the base configuration file.
    cat << EOF > "$CONFIG_PATH"
# Auto-generated configuration for node-$i
identity_path = "identity.key"
p2p_addr = "127.0.0.1:$P2P_PORT"
gossip_interval_ms = 1500
gossip_factor = 2
node_ttl_ms = 300000
bootstrap_peers = []
EOF

    # Designate node-0 as the single visualizer node.
    if [ "$i" -eq 0 ]; then
        cat << EOF >> "$CONFIG_PATH"

[visualizer]
bind_addr = "127.0.0.1:$VISUALIZER_PORT"
EOF
        # **FIX:** Copy the frontend assets to the visualizer node's directory.
        cp -r frontend/dist "$NODE_DIR/"
    fi

    # Copy the certs directory into each node's directory.
    cp -r certs "$NODE_DIR/"

    echo "Generated base config for node-$i at $CONFIG_PATH"
done

# --- Phase 2: Calculate and Assign Bootstrap Peers ---
echo "--- Assigning bootstrap peers (Phase 2) ---"
for i in $(seq 0 $((NUM_NODES - 1))); do
    CONFIG_PATH="$CLUSTER_DIR/node-$i/config.toml"

    # Create a list of all *other* nodes to select peers from.
    potential_peers=()
    for j in $(seq 0 $((NUM_NODES - 1))); do
        if [ "$i" -ne "$j" ]; then
            potential_peers+=("${P2P_ADDRESSES[j]}")
        fi
    done

    # Calculate the number of peers to connect to based on the ratio.
    num_peers_to_connect=$(awk "BEGIN {print int((${#potential_peers[@]}) * $CONNECTION_RATIO)}")

    if [ "$num_peers_to_connect" -gt 0 ]; then
        # Randomly select peers using a portable method.
        selected_peers=$(printf "%s\n" "${potential_peers[@]}" | awk 'BEGIN{srand()}{print rand() "\t" $0}' | sort -n | cut -f2- | head -n "$num_peers_to_connect")
        peers_toml_array=$(echo "$selected_peers" | awk '{print "\""$0"\""}' | paste -sd, -)

        # Append the bootstrap_peers list to the config file.
        sed "s/bootstrap_peers = \[\]/bootstrap_peers = [$peers_toml_array]/" "$CONFIG_PATH" > "$CONFIG_PATH.tmp" && mv "$CONFIG_PATH.tmp" "$CONFIG_PATH"

        echo "node-$i will connect to $num_peers_to_connect peer(s)."
    else
        echo "node-$i will start in isolation (0 peers to connect)."
    fi
done

# --- Phase 3: Launch All Nodes ---
echo "--- Launching $NUM_NODES nodes ---"
for i in $(seq 0 $((NUM_NODES - 1))); do
    NODE_DIR="$CLUSTER_DIR/node-$i"
    
    (
        cd "$NODE_DIR"
        ../../target/release/gossip-network > node.log 2>&1 &
        PIDS+=($!)
        echo "Launched node-$i (PID: $!)"
    )
done

# --- Wait for Interruption ---
echo ""
echo "--- Cluster is running ---"
echo "Visualizer UI available at: http://127.0.0.1:$VISUALIZER_PORT"
echo "Logs for each node are located in '$CLUSTER_DIR/node-*/node.log'"
echo "Press Ctrl+C to stop the cluster."
echo ""

# **FIX:** Keep the script alive so the trap can catch Ctrl+C.
# This starts a process in the background and waits for it.
tail -f /dev/null &
BLOCKER_PID=$!
wait "$BLOCKER_PID"