import * as d3 from 'd3';
import {
  /** @type {import('./types').NetworkState} */
  NetworkState,
} from './types.js';

// --- State Management (Svelte 5 Runes) ---

// This is the raw state received from the WebSocket.
const rawNetworkState = $state(/** @type {NetworkState | null} */ (null));
const connectionStatus = $state(/** @type {import('./types').ConnectionStatus} */ ('connecting'));

// --- Constants & Configuration ---
const RECONNECT_DELAY_MS = 3000;
const COLOR_SCHEME = d3.schemeTableau10; // A colorblind-safe and pleasant color scheme.

/**
 * A reactive, derived store that transforms raw backend data into a format
 * suitable for D3's force simulation. This is the "brain" of the data layer.
 *
 * It computes nodes with radii and colors, and links between them, memoizing
 * the result with `$derived`.
 */
const graphData = $derived(() => {
  if (!rawNetworkState || !rawNetworkState.nodes) {
    return { nodes: [], links: [], selfId: null, communities: new Map() };
  }

  // 1. Transform the node map from the backend into an array of objects.
  const nodes = Object.entries(rawNetworkState.nodes).map(([id, info]) => ({
    id,
    ...info,
  }));

  // 2. Create links from the `self_id` to its bootstrap peers (`edges`).
  const links =
    rawNetworkState.self_id && rawNetworkState.edges
      ? rawNetworkState.edges
          .map((targetId) => ({
            source: rawNetworkState.self_id,
            target: targetId,
          }))
          .filter((link) => rawNetworkState.nodes[link.target]) // Ensure target node exists
      : [];

  // 3. Calculate the degree of each node (number of connections).
  const degrees = new Map();
  nodes.forEach((n) => degrees.set(n.id, 0));
  links.forEach((link) => {
    degrees.set(link.source, (degrees.get(link.source) || 0) + 1);
    degrees.set(link.target, (degrees.get(link.target) || 0) + 1);
  });

  // 4. Create a color scale for communities.
  const communities = new Map(nodes.map((n) => [n.community_id, 0]));
  const colorScale = d3.scaleOrdinal(COLOR_SCHEME).domain(communities.keys());

  // 5. Enhance node objects with calculated properties for visualization.
  const enhancedNodes = nodes.map((node) => ({
    ...node,
    radius: 4 + (degrees.get(node.id) || 0) * 4, // Radius based on degree
    color: colorScale(node.community_id),
  }));

  return {
    nodes: enhancedNodes,
    links,
    selfId: rawNetworkState.self_id,
    communities,
  };
});

/**
 * Establishes and manages the WebSocket connection.
 * This function is the "engine" that fetches data. It's designed
 * to be called once when the application starts.
 */
function connect() {
  const wsUrl = `ws://${window.location.host}/ws`;
  const ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    console.log('WebSocket connected');
    connectionStatus.value = 'connected';
  };

  ws.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data);
      // The hex-encoded NodeId from Rust is a 32-byte array, resulting in a
      // 64-char string. It's too long for display. We will use it as the
      // internal ID and display a shortened version.
      rawNetworkState.value = data;
    } catch (e) {
      console.error('Failed to parse WebSocket message:', e);
    }
  };

  ws.onclose = () => {
    console.warn(`WebSocket disconnected. Reconnecting in ${RECONNECT_DELAY_MS / 1000}s...`);
    connectionStatus.value = 'disconnected';
    setTimeout(connect, RECONNECT_DELAY_MS);
  };

  ws.onerror = (error) => {
    console.error('WebSocket error:', error);
    ws.close();
  };
}

// --- Public API ---

/**
 * A singleton store that exposes reactive state to Svelte components.
 */
export const networkStore = {
  /** The processed, D3-ready graph data. */
  get graph() {
    return graphData;
  },
  /** The current status of the WebSocket connection. */
  get status() {
    return connectionStatus.value;
  },
  /** Initializes the connection. Should be called once. */
  connect,
};