#!/bin/bash
#
# orchestrator.sh - Deploys a local gossip network cluster with communities.
#
# This script automates the setup and launch of N gossip-network nodes,
# creating a structured network with dense intra-community connections and
# sparse inter-community connections.
#
# -----------------------------------------------------------------------------
# HOW TO USE
# -----------------------------------------------------------------------------
# 1. MAKE EXECUTABLE (first time only):
#    chmod +x orchestrator.sh
#
# 2. RUN THE SCRIPT:
#    Provide the number of nodes, number of communities, and two connection
#    ratios (intra-community and inter-community).
#
#    Usage:
#      ./orchestrator.sh <NUM_NODES> <NUM_COMMUNITIES> <INTRA_RATIO> <INTER_RATIO>
#
#    Examples:
#      # Launch a 30-node network with 3 communities.
#      # Each node connects to 80% of its own community and 5% of others.
#      ./orchestrator.sh 30 3 0.8 0.05
#
# 3. VIEW THE VISUALIZER:
#    Once the cluster is running, open your browser to:
#      http://127.0.0.1:8080
#
# 4. STOP THE CLUSTER:
#    Press Ctrl+C in the terminal where the script is running.
# -----------------------------------------------------------------------------

# --- Script Configuration ---
set -e
set -u
set -o pipefail

# --- Parameters ---
if [ "$#" -ne 4 ]; then
    echo "Usage: $0 <NUM_NODES> <NUM_COMMUNITIES> <INTRA_RATIO> <INTER_RATIO>"
    exit 1
fi

NUM_NODES=$1
NUM_COMMUNITIES=$2
INTRA_RATIO=$3
INTER_RATIO=$4
CLUSTER_DIR="cluster"
BASE_P2P_PORT=5000
VISUALIZER_PORT=8080

PIDS=()

# --- Cleanup Function ---
cleanup() {
    echo ""
    echo "--- Shutting down cluster ---"
    if [ -n "${BLOCKER_PID-}" ]; then
        kill "$BLOCKER_PID" 2>/dev/null || true
    fi
    if [ ${#PIDS[@]} -ne 0 ]; then
        kill "${PIDS[@]}" 2>/dev/null || true
        echo "All node processes have been terminated."
    else
        echo "No processes to terminate."
    fi
}
trap cleanup EXIT

# --- Pre-flight Checks ---
echo "--- Preparing environment ---"
if [ ! -d "certs" ] || [ ! -f "certs/ca.cert" ]; then
    echo "Error: 'certs' directory not found." && exit 1
fi
if [ ! -d "frontend/dist" ]; then
    echo "Error: 'frontend/dist' directory not found. Please build the frontend." && exit 1
fi
echo "Building gossip-network binary..."
cargo build --release
echo "Creating cluster directory: $CLUSTER_DIR"
rm -rf "$CLUSTER_DIR"
mkdir -p "$CLUSTER_DIR"

# --- Phase 1: Generate Node Configurations ---
echo "--- Generating node configurations (Phase 1) ---"
P2P_ADDRESSES=()
COMMUNITY_IDS=()
for i in $(seq 0 $((NUM_NODES - 1))); do
    NODE_DIR="$CLUSTER_DIR/node-$i"
    CONFIG_PATH="$NODE_DIR/config.toml"
    P2P_PORT=$((BASE_P2P_PORT + i))
    COMMUNITY_ID=$((i % NUM_COMMUNITIES))

    P2P_ADDRESSES+=("127.0.0.1:$P2P_PORT")
    COMMUNITY_IDS+=($COMMUNITY_ID)

    mkdir -p "$NODE_DIR"

    cat << EOF > "$CONFIG_PATH"
# Auto-generated configuration for node-$i
identity_path = "identity.key"
p2p_addr = "127.0.0.1:$P2P_PORT"
gossip_interval_ms = 1500
gossip_factor = 3
node_ttl_ms = 300000
community_id = $COMMUNITY_ID
bootstrap_peers = []
EOF

    if [ "$i" -eq 0 ]; then
        cat << EOF >> "$CONFIG_PATH"
[visualizer]
bind_addr = "127.0.0.1:$VISUALIZER_PORT"
EOF
        cp -r frontend/dist "$NODE_DIR/"
    fi
    cp -r certs "$NODE_DIR/"
    echo "Generated config for node-$i (Community $COMMUNITY_ID)"
done

# --- Phase 2: Calculate and Assign Bootstrap Peers ---
echo "--- Assigning bootstrap peers (Phase 2) ---"
for i in $(seq 0 $((NUM_NODES - 1))); do
    CONFIG_PATH="$CLUSTER_DIR/node-$i/config.toml"
    CURRENT_COMMUNITY_ID=${COMMUNITY_IDS[i]}

    intra_community_peers=()
    inter_community_peers=()

    for j in $(seq 0 $((NUM_NODES - 1))); do
        if [ "$i" -ne "$j" ]; then
            if [ "$CURRENT_COMMUNITY_ID" -eq "${COMMUNITY_IDS[j]}" ]; then
                intra_community_peers+=("${P2P_ADDRESSES[j]}")
            else
                inter_community_peers+=("${P2P_ADDRESSES[j]}")
            fi
        fi
    done

    num_intra=$(awk "BEGIN {print int(${#intra_community_peers[@]} * $INTRA_RATIO)}")
    num_inter=$(awk "BEGIN {print int(${#inter_community_peers[@]} * $INTER_RATIO)}")

    selected_peers=""
    if [ "$num_intra" -gt 0 ]; then
        selected_peers+=$(printf "%s\n" "${intra_community_peers[@]}" | awk 'BEGIN{srand()}{print rand() "\t" $0}' | sort -n | cut -f2- | head -n "$num_intra")
    fi
    if [ "$num_inter" -gt 0 ]; then
        # Add a newline if intra peers were already selected
        [ -n "$selected_peers" ] && selected_peers+=$'\n'
        selected_peers+=$(printf "%s\n" "${inter_community_peers[@]}" | awk 'BEGIN{srand()}{print rand() "\t" $0}' | sort -n | cut -f2- | head -n "$num_inter")
    fi

    if [ -n "$selected_peers" ]; then
        peers_toml_array=$(echo "$selected_peers" | awk '{print "\""$0"\""}' | paste -sd, -)
        sed "s/bootstrap_peers = \[\]/bootstrap_peers = [$peers_toml_array]/" "$CONFIG_PATH" > "$CONFIG_PATH.tmp" && mv "$CONFIG_PATH.tmp" "$CONFIG_PATH"
        echo "node-$i will connect to $num_intra intra-community and $num_inter inter-community peer(s)."
    else
        echo "node-$i will start in isolation."
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
tail -f /dev/null &
BLOCKER_PID=$!
wait "$BLOCKER_PID"