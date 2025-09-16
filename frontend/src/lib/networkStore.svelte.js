import * as d3 from 'd3';

// --- Local State Signals (not exported) ---
const rawNetworkState = $state(/** @type {import('./types').NetworkState | null} */ (null));
const status = $state(/** @type {import('./types').ConnectionStatus} */ ('connecting'));

// --- Local Derived Signal (not exported) ---
const graph = $derived(() => {
  const state = rawNetworkState.value;
  if (!state || !state.nodes) {
    return { nodes: [], links: [], selfId: null, communities: new Map() };
  }

  const nodes = Object.entries(state.nodes).map(([id, info]) => ({ id, ...info }));
  const links = state.self_id && state.edges
      ? state.edges.map(targetId => ({ source: state.self_id, target: targetId }))
          .filter(link => state.nodes[link.target])
      : [];

  const degrees = new Map(nodes.map(n => [n.id, 0]));
  links.forEach(link => {
    degrees.set(link.source, degrees.get(link.source) + 1);
    degrees.set(link.target, degrees.get(link.target) + 1);
  });

  const communities = new Map(nodes.map(n => [n.community_id, 0]));
  const colorScale = d3.scaleOrdinal(d3.schemeTableau10).domain([...communities.keys()]);

  return {
    nodes: nodes.map(node => ({
      ...node,
      radius: 4 + degrees.get(node.id) * 4,
      color: colorScale(node.community_id),
    })),
    links,
    selfId: state.self_id,
    communities,
  };
});

// --- Connection Logic ---
function connect() {
  const wsUrl = `ws://${window.location.host}/ws`;
  const ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    console.log('WebSocket connected');
    status.value = 'connected';
  };

  ws.onmessage = (event) => {
    try {
      rawNetworkState.value = JSON.parse(event.data);
    } catch (e) {
      console.error('Failed to parse WebSocket message:', e);
    }
  };

  ws.onclose = () => {
    console.warn(`WebSocket disconnected. Reconnecting in 3s...`);
    status.value = 'disconnected';
    setTimeout(connect, 3000);
  };

  ws.onerror = (error) => {
    console.error('WebSocket error:', error);
    ws.close();
  };
}

// --- Public API (The Store) ---
// This is the single export that conforms to the compiler rule.
export const networkStore = {
  /** A getter that reactively returns the current connection status. */
  get status() {
    return status.value;
  },
  /** A getter that reactively returns the current derived graph data. */
  get graph() {
    return graph.value;
  },
  /** The function to initiate the connection. */
  connect,
};