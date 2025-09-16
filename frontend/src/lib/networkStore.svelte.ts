import * as d3 from 'd3';
import type { ConnectionStatus, GraphData, NetworkState, SimulationLink, SimulationNode } from '$lib/types';

// Create a single reactive `$state` object. This guarantees that `store.graph`
// has a valid, non-null initial value from the moment the module is loaded.
const store = $state({
    status: 'connecting' as ConnectionStatus,
    graph: {
        nodes: [],
        links: [],
        selfId: null,
        communities: new Map(),
    } as GraphData,
    rawNetworkState: null as NetworkState | null,
    initialized: false,
});

// This effect listens for changes to the raw state and recalculates the graph.
// It is still an "orphaned" effect, but it corrects the fatal TypeError.
$effect(() => {
	const state = store.rawNetworkState;

	if (!state || !state.nodes) {
        // Explicitly reset the graph to its empty state if raw data is cleared.
        store.graph = {
			nodes: [],
			links: [],
			selfId: null,
			communities: new Map(),
		};
		return;
	}

	const nodes = Object.entries(state.nodes).map(([id, info]) => ({ id, ...info }));
	const links: SimulationLink[] =
		state.self_id && state.edges
			? state.edges
					.map((targetId) => ({ source: state.self_id!, target: targetId }))
					.filter((link) => state.nodes[link.target as string])
			: [];
	
	const degrees = new Map<string, number>(nodes.map((n) => [n.id, 0]));
	links.forEach((link) => {
		degrees.set(link.source as string, (degrees.get(link.source as string) ?? 0) + 1);
		degrees.set(link.target as string, (degrees.get(link.target as string) ?? 0) + 1);
	});

	const communities = new Map<number, number>(nodes.map((n) => [n.community_id, 0]));
	const colorScale = d3.scaleOrdinal(d3.schemeTableau10).domain([...communities.keys()].map(String));

	const simulationNodes: SimulationNode[] = nodes.map((node) => ({
		...node,
		radius: 4 + (degrees.get(node.id) ?? 0) * 4,
		color: colorScale(String(node.community_id)),
	}));
	
    // Atomically assign the newly computed graph object.
	store.graph = {
		nodes: simulationNodes,
		links: links,
		selfId: state.self_id,
		communities: communities,
	};
});

function init() {
	if (store.initialized) return;
	store.initialized = true;
	const wsUrl = `ws://${window.location.host}/ws`;
	const ws = new WebSocket(wsUrl);
	ws.onopen = () => {
		store.status = 'connected';
	};
	ws.onmessage = (event) => {
		store.rawNetworkState = JSON.parse(event.data);
	};
	ws.onclose = () => {
		console.warn('[Store] WebSocket CLOSED.');
		store.status = 'disconnected';
		store.initialized = false;
		setTimeout(init, 3000);
	};
	ws.onerror = (error) => console.error('[Store] WebSocket ERROR:', error);
}

// --- PUBLIC API ---
// The public API surface remains identical to consumers.
export const networkStore = {
	get status() { return store.status; },
	get graph() { return store.graph; },
	init,
};