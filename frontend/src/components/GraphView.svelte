<!-- --- File: frontend/src/components/GraphView.svelte --- -->
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
        // MODIFICATION: Keep a stable reference to the merged link selection.
        linkMerged: d3.Selection<d3.BaseType | SVGLineElement, SimulationLink, SVGGElement, unknown>;
	} | null = null;
    
    let graphNodes: SimulationNode[] = [];

	const MIN_RADIUS = 8;
	const MAX_RADIUS = 24;

	// The `$effect` rune handles component initialization and reactive updates.
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

            // Initialize linkMerged to an empty selection.
            const linkMerged = linkGroup.selectAll('line').data<SimulationLink>([]);

			d3State = { simulation, linkGroup, nodeGroup, linkMerged };
		}

		const { simulation, linkGroup, nodeGroup } = d3State;

		// --- Reactive Data Merge ---
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
		d3State.linkMerged = linkEnter.merge(linkSelection); // Update the stored selection

		const nodeSelection = nodeGroup
			.selectAll('g.node')
			.data(graphNodes, (d: any) => d.id);

		nodeSelection.exit().remove();
		const nodeEnter = nodeSelection.enter().append('g').attr('class', 'node');

		const colorScale = d3.scaleOrdinal(d3.schemeCategory10);
		const radiusScale = d3.scaleLinear().domain([100, 150]).range([MIN_RADIUS, MAX_RADIUS]).clamp(true);

		nodeEnter.append('circle');
		nodeEnter.append('text');
		nodeEnter.append('title');
		nodeEnter.call(drag(simulation));

		const nodeMerged = nodeEnter.merge(nodeSelection as any);

		nodeMerged.select('circle')
            .transition().duration(200)
			.attr('r', d => radiusScale(d.info.telemetry.value))
			.attr('fill', d => colorScale(d.info.community_id.toString()))
			.attr('stroke', d => (d.id === selfId ? '#facc15' : '#777'));

		nodeMerged.select('text')
			.text(d => networkStore.truncateNodeId(d.id));

		nodeMerged.select('title')
			.text(d => `ID: ${d.id}\nCommunity: ${d.info.community_id}\nValue: ${d.info.telemetry.value.toFixed(2)}`);

		simulation.nodes(graphNodes);
		simulation.force<d3.ForceLink<SimulationNode, SimulationLink>>('link')?.links(links);
		simulation.alpha(0.3).restart();

		function ticked() {
			d3State?.linkMerged
				.attr('x1', (d: any) => d.source.x)
				.attr('y1', (d: any) => d.source.y)
				.attr('x2', (d: any) => d.target.x)
				.attr('y2', (d: any) => d.target.y);

			nodeMerged.attr('transform', (d) => `translate(${d.x},${d.y})`);
		}
	});

    // NEW: Add a separate effect specifically for handling the highlight.
    // This effect is lean and runs only when the highlight state changes.
    $effect(() => {
        if (!d3State) return;

        const highlightedNodeId = networkStore.lastMessageSource;
        const selfId = networkStore.selfId;

        // Use D3's `classed` method to efficiently toggle the highlight class.
        d3State.linkMerged.classed('highlighted', d =>
            highlightedNodeId !== null &&
            selfId !== null &&
            // Check if the highlighted node is either the source or target of the link
            ((d.source as SimulationNode).id === selfId && (d.target as SimulationNode).id === highlightedNodeId) ||
            ((d.target as SimulationNode).id === selfId && (d.source as SimulationNode).id === highlightedNodeId)
        );
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
	.graph-container { display: flex; flex-direction: column; height: 100%; padding: 1.5rem; gap: 1rem; box-sizing: border-box; }
	.stats-bar { color: #ccc; display: flex; gap: 2rem; background: #2a2a2e; padding: 0.5rem 1rem; border-radius: 6px; border: 1px solid #444; font-family: monospace; flex-shrink: 0; }
	.svg-wrapper { flex-grow: 1; border: 1px solid #444; border-radius: 8px; overflow: hidden; background-color: #1e1e1e; }

	/* Default link style */
	:global(svg .links line) {
		stroke: #555;
		stroke-opacity: 0.7;
		stroke-width: 1.5px;
        /* NEW: Add transition for smooth style changes */
        transition: stroke 0.2s ease-out, stroke-width 0.2s ease-out;
	}
    
    /* NEW: Style for the highlighted link */
    :global(svg .links line.highlighted) {
        stroke: #facc15; /* A bright yellow */
        stroke-width: 4px;
    }

	:global(svg .nodes circle) { stroke-width: 2px; transition: transform 0.1s ease-in-out; }
	:global(svg .nodes g.node:hover circle) { transform: scale(1.1); }
	:global(svg .nodes text) { fill: #ccc; font-size: 10px; font-family: monospace; paint-order: stroke; stroke: #1e1e1e; stroke-width: 2px; stroke-linejoin: round; pointer-events: none; transform: translate(0, -16px); text-anchor: middle; }
</style>