// --- File: frontend/src/lib/networkStore.svelte.ts ---
import type { NodeId, NodeInfo, WebSocketMessage, UpdatePayload } from './types';

// --- State Definition (using Svelte 5 runes) ---
let isConnected = $state(false);
let selfId: NodeId | null = $state(null);
let nodes = $state<Record<NodeId, NodeInfo>>({});
let activeConnections = $state<Set<NodeId>>(new Set());

// NEW: State for the "pulsing" animation mechanism.
// This holds the set of peer IDs for the current animation pulse.
let currentPulsePeers = $state(new Set<NodeId>());
// Internal, non-reactive state to batch incoming animation events.
let pendingPulsePeers = new Set<NodeId>();
let pulseTimerId: number | null = null;


export interface LogEntry {
    id: number;
    timestamp: Date;
    message: string;
    type: 'info' | 'warn' | 'success' | 'error';
}
let log = $state<LogEntry[]>([]);
let logCounter = 0;

// --- Helper Functions ---
const truncateNodeId = (id: NodeId) => `${id.substring(0, 8)}...`;

function addLogEntry(message: string, type: LogEntry['type']) {
    log.unshift({ id: logCounter++, timestamp: new Date(), message, type });
    if (log.length > 200) { // Keep the log from growing indefinitely
        log.pop();
    }
}

function formatUpdateMessage(payload: UpdatePayload): string {
    const { event, data } = payload;
    switch (event) {
        case 'node_added':
            return `Discovered new node: ${truncateNodeId(data.id)} (Community ${data.info.community_id})`;
        case 'node_updated':
            return `Received telemetry update for node: ${truncateNodeId(data.id)}`;
        case 'node_removed':
            return `Node considered stale and removed: ${truncateNodeId(data.id)}`;
        case 'connection_status':
            return `Peer connection ${data.is_connected ? 'established with' : 'lost from'} ${truncateNodeId(data.peer_id)}`;
        case 'animate_edge':
            // This event is now handled in batches, so we don't log individual ones.
            return `[Animation] Edge from ${truncateNodeId(data.from_peer)} pulsed.`;
    }
}

// --- WebSocket Connection Logic ---
function connect() {
    const wsUrl = `ws://${window.location.host}/ws`;
    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        isConnected = true;
        addLogEntry('Connected to WebSocket server.', 'success');
    };

    ws.onclose = () => {
        isConnected = false;
        selfId = null;
        nodes = {};
        activeConnections.clear();
        addLogEntry('Disconnected from WebSocket server. Retrying in 3s...', 'error');
        setTimeout(connect, 3000);
    };

    ws.onmessage = (event) => {
        try {
            const data: WebSocketMessage = JSON.parse(event.data);

            if (data.type === 'snapshot') {
                const payload = data.payload;
                selfId = payload.self_id;
                nodes = payload.nodes;
                activeConnections = new Set(payload.active_connections);
                addLogEntry(`Received initial state snapshot with ${Object.keys(nodes).length} nodes.`, 'info');
            } else if (data.type === 'update') {
                const payload = data.payload;
                const { event, data: eventData } = payload;

                if (event !== 'animate_edge') {
                    addLogEntry(formatUpdateMessage(payload), 'info');
                }

                switch (event) {
                    case 'node_added':
                        nodes[eventData.id] = eventData.info;
                        break;
                    case 'node_updated':
                        nodes[eventData.id] = eventData.info;
                        break;
                    case 'node_removed':
                        delete nodes[eventData.id];
                        break;
                    case 'connection_status':
                        if (eventData.is_connected) {
                            activeConnections.add(eventData.peer_id);
                        } else {
                            activeConnections.delete(eventData.peer_id);
                        }
                        break;
                    case 'animate_edge':
                        // FIX: Batch animation events instead of handling them individually.
                        pendingPulsePeers.add(eventData.from_peer);

                        // If a pulse is not already scheduled, schedule one.
                        if (!pulseTimerId) {
                            pulseTimerId = window.setTimeout(() => {
                                // 1. Promote the pending batch to the reactive `currentPulsePeers`.
                                // This triggers the animation effect in the GraphView.
                                currentPulsePeers = new Set(pendingPulsePeers);

                                // 2. Clear the pending batch and the timer ID.
                                pendingPulsePeers.clear();
                                pulseTimerId = null;

                                // 3. Schedule the clearing of the reactive state after the animation completes.
                                window.setTimeout(() => {
                                    currentPulsePeers = new Set();
                                }, 750); // Must match CSS animation duration
                            }, 50); // Batch events that arrive within a 50ms window.
                        }
                        break;
                }
            }
        } catch (error) {
            console.error('Failed to process WebSocket message:', error);
            addLogEntry('Received an invalid WebSocket message.', 'warn');
        }
    };

    ws.onerror = (err) => {
        console.error('WebSocket error:', err);
    };
}

// --- Exported Store API ---
export const networkStore = {
    get isConnected() { return isConnected; },
    get selfId() { return selfId; },
    get nodes() { return nodes; },
    get activeConnections() { return activeConnections; },
    // MODIFICATION: Expose the set of peers for the current animation pulse.
    get currentPulsePeers() { return currentPulsePeers; },
    get log() { return log; },
    connect,
    truncateNodeId
};