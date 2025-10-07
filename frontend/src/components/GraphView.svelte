<script lang="ts">
	import { networkStore } from '../lib/networkStore.svelte.ts';
	import * as d3 from 'd3';
	import type { NodeId, NodeInfo } from '../lib/types';

	// D3 requires mutable objects for simulation, so we define an extended type.
	interface SimulationNode extends d3.SimulationNodeDatum {
		id: NodeId;
		info: NodeInfo;
	}

	interface SimulationLink extends d3.SimulationLinkDatum<SimulationNode> {
		source: NodeId;
		target: NodeId;
	}

	let svgElement: SVGSVGElement;
	let width = 800;
	let height = 600;

	// Encapsulate D3 state to manage it across effect re-runs.
	let d3State: {
		simulation: d3.Simulation<SimulationNode, SimulationLink>;
		linkGroup: d3.Selection<SVGGElement, unknown, null, undefined>;
		nodeGroup: d3.Selection<SVGGElement, unknown, null, undefined>;
        linkMerged: d3.Selection<d3.BaseType | SVGLineElement, SimulationLink, SVGGElement, unknown>;
	} | null = null;
    
    let graphNodes: SimulationNode[] = [];

	// FIX: Replace dynamic radius constants with a single fixed value for clarity.
	const NODE_RADIUS = 12;

	// FIX: Combine the graph update and animation logic into a single effect.
	// This guarantees that link elements exist before animation is attempted, resolving the race condition.
	$effect(() => {
		if (!svgElement) return; // Wait for the SVG element to be bound.

		// --- One-time D3 setup ---
		if (!d3State) {
			const svg = d3.select(svgElement);
			width = svg.node()?.clientWidth ?? 800;
			height = svg.node()?.clientHeight ?? 600;

			const simulation = d3
				.forceSimulation<SimulationNode, SimulationLink>()
				.force('link', d3.forceLink<SimulationNode, SimulationLink>([]).id((d) => d.id).distance(150))
				.force('charge', d3.forceManyBody().strength(-200))
                .force('x', d3.forceX(width / 2).strength(0.05))
                .force('y', d3.forceY(height / 2).strength(0.05))
				.on('tick', ticked);

			const linkGroup = svg.append('g').attr('class', 'links');
			const nodeGroup = svg.append('g').attr('class', 'nodes');

            const linkMerged = linkGroup.selectAll('line').data<SimulationLink>([]);

			d3State = { simulation, linkGroup, nodeGroup, linkMerged };
		}

		const { simulation, linkGroup, nodeGroup } = d3State;

		// --- Reactive Data Merge (updates graph structure) ---
        const storeNodes = networkStore.nodes;
        const nodeMap = new Map(graphNodes.map(n => [n.id, n]));
        graphNodes = graphNodes.filter(n => storeNodes[n.id]);

        for (const id in storeNodes) {
            const info = storeNodes[id];
            const existingNode = nodeMap.get(id);
            if (existingNode) {
                existingNode.info = info;
            } else {
                graphNodes.push({ id, info });
            }
        }

		const selfId = networkStore.selfId;
		let links: SimulationLink[] = [];
		if (selfId) {
			links = [...networkStore.activeConnections]
                .filter(peerId => storeNodes[peerId]) 
                .map((peerId) => ({
				    source: selfId,
				    target: peerId,
			    }));
		}

		// --- D3 Data Join & Update ---
		const linkSelection = linkGroup
			.selectAll('line')
			.data(links, (d: any) => `${d.source}-${d.target}`);

		linkSelection.exit().remove();
		const linkEnter = linkSelection.enter().append('line');
		d3State.linkMerged = linkEnter.merge(linkSelection); 

		const nodeSelection = nodeGroup
			.selectAll('g.node')
			.data(graphNodes, (d: any) => d.id);

		nodeSelection.exit().remove();
		const nodeEnter = nodeSelection.enter().append('g').attr('class', 'node');

		const colorScale = d3.scaleOrdinal(d3.schemeCategory10);
		// FIX: The radius scale is no longer needed.
		// const radiusScale = d3.scaleLinear().domain([100, 150]).range([MIN_RADIUS, MAX_RADIUS]).clamp(true);

		nodeEnter.append('circle');
		nodeEnter.append('text');
		nodeEnter.append('title');
		nodeEnter.call(drag(simulation));

		const nodeMerged = nodeEnter.merge(nodeSelection as any);

		nodeMerged.select('circle')
            // FIX: Use the fixed NODE_RADIUS. The transition is removed as it's now static.
			.attr('r', NODE_RADIUS)
			.attr('fill', d => colorScale(d.info.community_id.toString()))
			.attr('stroke', d => (d.id === selfId ? '#facc15' : '#777'));

		nodeMerged.select('text')
			.text(d => networkStore.truncateNodeId(d.id));

		nodeMerged.select('title')
			// FIX: Remove the telemetry value from the tooltip for consistency.
			.text(d => `ID: ${d.id}\nCommunity: ${d.info.community_id}`);

		simulation.nodes(graphNodes);
		simulation.force<d3.ForceLink<SimulationNode, SimulationLink>>('link')?.links(links);
		simulation.alpha(0.3).restart();

		// --- Animation Logic ---
		const peersToAnimate = networkStore.currentPulsePeers;
		if (peersToAnimate.size > 0) {
			const linksToAnimate = d3State.linkMerged.filter(d => {
				const sourceNodeId = (d.source as SimulationNode).id;
				const targetNodeId = (d.target as SimulationNode).id;
				
				return (sourceNodeId === selfId && peersToAnimate.has(targetNodeId)) ||
					   (targetNodeId === selfId && peersToAnimate.has(sourceNodeId));
			});

			if (!linksToAnimate.empty()) {
				linksToAnimate.classed('highlight-pulse', false);
				requestAnimationFrame(() => {
					linksToAnimate.classed('highlight-pulse', true);
				});
			}
		}

		// --- Ticked Function ---
		function ticked() {
			d3State?.linkMerged
				.attr('x1', (d: any) => d.source.x)
				.attr('y1', (d: any) => d.source.y)
				.attr('x2', (d: any) => d.target.x)
				.attr('y2', (d: any) => d.target.y);

			nodeMerged.attr('transform', (d) => `translate(${d.x},${d.y})`);
		}
	});

	// --- D3 Drag Handler ---
	function drag(simulation: d3.Simulation<SimulationNode, any>) {
		function dragstarted(event: d3.D3DragEvent<any, any, any>, d: SimulationNode) {
			if (!event.active) simulation.alphaTarget(0.3).restart();
			d.fx = d.x;
			d.fy = d.y;
		}
		function dragged(event: d3.D3DragEvent<any, any, any>, d: SimulationNode) {
			d.fx = event.x;
			d.fy = event.y;
		}
		function dragended(event: d3.D3DragEvent<any, any, any>, d: SimulationNode) {
			if (!event.active) simulation.alphaTarget(0);
			d.fx = null;
			d.fy = null;
		}
		return d3.drag<any, SimulationNode>().on('start', dragstarted).on('drag', dragged).on('end', dragended);
	}
</script>

<div class="graph-container">
	<div class="stats-bar">
		<span>Nodes: {Object.keys(networkStore.nodes).length}</span>
		<span>Active Connections: {networkStore.activeConnections.size}</span>
	</div>
	<div class="svg-wrapper">
		<svg bind:this={svgElement} width="100%" height="100%">
			<!-- D3 will manage the content of this SVG -->
		</svg>
	</div>
</div>

<style>
	.graph-container {
		display: flex;
		flex-direction: column;
		height: 100%;
		padding: 1.5rem;
		gap: 1rem;
		box-sizing: border-box;
	}
	.stats-bar {
		color: #ccc;
		display: flex;
		gap: 2rem;
		background: #2a2a2e;
		padding: 0.5rem 1rem;
		border-radius: 6px;
		border: 1px solid #444;
		font-family: monospace;
		flex-shrink: 0;
	}
	.svg-wrapper {
		flex-grow: 1;
		border: 1px solid #444;
		border-radius: 8px;
		overflow: hidden;
		background-color: #1e1e1e;
	}

	:global(svg .links line) {
		stroke: #555;
		stroke-opacity: 0.7;
		stroke-width: 1.5px;
	}

    @keyframes pulse-animation {
        0% {
            stroke: #fde047; /* Bright yellow */
            stroke-width: 4px;
        }
        100% {
            stroke: #555;
            stroke-width: 1.5px;
        }
    }

    :global(svg .links line.highlight-pulse) {
        animation-name: pulse-animation;
        animation-duration: 750ms;
        animation-timing-function: ease-out;
        animation-fill-mode: forwards;
    }

	:global(svg .nodes circle) {
		stroke-width: 2px;
		transition: transform 0.1s ease-in-out;
	}
	:global(svg .nodes g.node:hover circle) {
		transform: scale(1.1);
	}

	:global(svg .nodes text) {
		fill: #ccc;
		font-size: 10px;
		font-family: monospace;
		paint-order: stroke;
		stroke: #1e1e1e;
		stroke-width: 2px;
		stroke-linejoin: round;
		pointer-events: none;
		transform: translate(0, -16px);
		text-anchor: middle;
	}
</style>