// --- File: frontend/src/lib/networkStore.svelte.ts ---
// src/lib/networkStore.ts
import type { NodeId, NodeInfo, WebSocketMessage, UpdatePayload } from './types';

// --- State Definition (using Svelte 5 runes) ---
let isConnected = $state(false);
let selfId: NodeId | null = $state(null);
let nodes = $state<Record<NodeId, NodeInfo>>({});
let activeConnections = $state<Set<NodeId>>(new Set());

// NEW: Add state to track the source of the last message for highlighting.
let lastMessageSource: NodeId | null = $state(null);
let highlightTimeoutId: number | null = null;


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
            return `Received telemetry update from node: ${truncateNodeId(data.id)}`;
        case 'node_removed':
            return `Node considered stale and removed: ${truncateNodeId(data.id)}`;
        case 'connection_status':
            return `Peer connection ${data.is_connected ? 'established with' : 'lost from'} ${truncateNodeId(data.peer_id)}`;
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
        setTimeout(connect, 3000); // Simple retry mechanism
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

                switch (event) {
                    case 'node_added':
                        nodes[eventData.id] = eventData.info;
                        break;
                    case 'node_updated':
                        nodes[eventData.id] = eventData.info;
                        // NEW: Trigger the highlight effect for the link.
                        // This identifies the originator of the update.
                        if (highlightTimeoutId) clearTimeout(highlightTimeoutId);
                        lastMessageSource = eventData.id;
                        highlightTimeoutId = window.setTimeout(() => {
                            lastMessageSource = null;
                        }, 750); // Highlight duration
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
                }
                // Add a formatted log message for the update
                addLogEntry(formatUpdateMessage(payload), 'info');
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
    // NEW: Expose the last message source for the visualizer to react to.
    get lastMessageSource() { return lastMessageSource; },
    get log() { return log; },
    connect,
    truncateNodeId
};