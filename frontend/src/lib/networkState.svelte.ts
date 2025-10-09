// src/lib/networkState.svelte.ts
import type { NodeId, NodeInfo, WebSocketMessage, UpdatePayload } from './types';

export interface LogEntry {
    id: number;
    timestamp: Date;
    message: string;
    type: 'info' | 'warn' | 'success' | 'error';
}

export const networkState = $state({
    isConnected: false,
    selfId: null as NodeId | null,
    nodes: {} as Record<NodeId, NodeInfo>,
    activeConnections: new Set<NodeId>(),
    currentPulsePeers: new Set<NodeId>(),
    log: [] as LogEntry[],
});

let logCounter = 0;
let pendingPulsePeers = new Set<NodeId>();
let pulseTimerId: number | null = null;

export const truncateNodeId = (id: NodeId) => `${id.substring(0, 8)}...`;

function addLogEntry(message: string, type: LogEntry['type']) {
    // MODIFICATION: Use `push` to add new entries to the end of the array (chronological order).
    networkState.log.push({ id: logCounter++, timestamp: new Date(), message, type });
    // MODIFICATION: If the log is too long, remove the OLDEST entry from the beginning using `shift`.
    if (networkState.log.length > 200) {
        networkState.log.shift();
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
            return `[Animation] Edge from ${truncateNodeId(data.from_peer)} pulsed.`;
    }
}

export function connect() {
    const wsUrl = `ws://${window.location.host}/ws`;
    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        networkState.isConnected = true;
        addLogEntry('Connected to WebSocket server.', 'success');
    };

    ws.onclose = () => {
        networkState.isConnected = false;
        networkState.selfId = null;
        networkState.nodes = {};
        networkState.activeConnections.clear();
        networkState.currentPulsePeers.clear();
        addLogEntry('Disconnected from WebSocket server. Retrying in 3s...', 'error');
        setTimeout(connect, 3000);
    };

    ws.onmessage = (event) => {
        try {
            const data: WebSocketMessage = JSON.parse(event.data);

            if (data.type === 'snapshot') {
                const payload = data.payload;
                networkState.selfId = payload.self_id;
                networkState.nodes = payload.nodes;
                networkState.activeConnections = new Set(payload.active_connections);
                addLogEntry(`Received initial state snapshot with ${Object.keys(networkState.nodes).length} nodes.`, 'info');
            } else if (data.type === 'update') {
                const payload = data.payload;
                const { event, data: eventData } = payload;

                if (event !== 'animate_edge') {
                    addLogEntry(formatUpdateMessage(payload), 'info');
                }

                switch (event) {
                    case 'node_added':
                        networkState.nodes[eventData.id] = eventData.info;
                        break;
                    case 'node_updated':
                        networkState.nodes = { ...networkState.nodes, [eventData.id]: eventData.info };
                        break;
                    case 'node_removed':
                        delete networkState.nodes[eventData.id];
                        networkState.nodes = { ...networkState.nodes };
                        break;
                    case 'connection_status':
                        if (eventData.is_connected) {
                            networkState.activeConnections.add(eventData.peer_id);
                        } else {
                            networkState.activeConnections.delete(eventData.peer_id);
                        }
                        networkState.activeConnections = new Set(networkState.activeConnections);
                        break;
                    case 'animate_edge':
                        pendingPulsePeers.add(eventData.from_peer);
                        if (!pulseTimerId) {
                            pulseTimerId = window.setTimeout(() => {
                                networkState.currentPulsePeers = new Set(pendingPulsePeers);
                                pendingPulsePeers.clear();
                                pulseTimerId = null;
                                window.setTimeout(() => {
                                    networkState.currentPulsePeers = new Set();
                                }, 750);
                            }, 50);
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