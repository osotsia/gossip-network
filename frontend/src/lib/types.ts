import type { SimulationLinkDatum, SimulationNodeDatum } from 'd3';

// --- Backend Data Structures ---

export type NodeId = string;

export interface TelemetryData {
  timestamp_ms: number;
  value: number;
}

export interface NodeInfo {
  telemetry: TelemetryData;
  community_id: number;
}

export interface NetworkState {
  self_id: NodeId | null;
  nodes: Record<NodeId, NodeInfo>;
  edges: NodeId[];
}

export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected';


// --- Frontend Simulation-Specific Types ---

/**
 * The object representing a node within the D3 force simulation.
 * It combines backend data with D3's required properties and our custom vis properties.
 */
export interface SimulationNode extends NodeInfo, SimulationNodeDatum {
  id: NodeId;
  radius: number;
  color: string;
}

/**
 * The object representing a link within the D3 force simulation.
 * D3 will replace the `source` and `target` string IDs with references
 * to the actual SimulationNode objects.
 */
export interface SimulationLink extends SimulationLinkDatum<SimulationNode> {
  // D3 requires source and target to be of this type,
  // but we initialize them as strings.
  source: SimulationNode | NodeId;
  target: SimulationNode | NodeId;
}

/**
 * The shape of the processed graph data object, ready for rendering.
 */
export interface GraphData {
  nodes: SimulationNode[];
  links: SimulationLink[];
  selfId: NodeId | null;
  communities: Map<number, number>;
}