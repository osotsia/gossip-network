// src/lib/types.ts

export type NodeId = string; // A hex-encoded string

export interface TelemetryData {
    timestamp_ms: number;
    value: number;
}

export interface NodeInfo {
    telemetry: TelemetryData;
    community_id: number;
}

// --- WebSocket Message Protocol ---

export interface SnapshotPayload {
    self_id: NodeId;
    nodes: Record<NodeId, NodeInfo>;
    active_connections: NodeId[];
}

export type UpdatePayload =
    | { event: 'node_added'; data: { id: NodeId; info: NodeInfo } }
    | { event: 'node_updated'; data: { id: NodeId; info: NodeInfo } }
    | { event: 'node_removed'; data: { id: NodeId } }
    | { event: 'connection_status'; data: { peer_id: NodeId; is_connected: boolean } };


export type WebSocketMessage =
    | { type: 'snapshot'; payload: SnapshotPayload }
    | { type: 'update'; payload: UpdatePayload };