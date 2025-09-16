<script lang="ts">
  import * as d3 from 'd3';
  import type { GraphData, SimulationNode, SimulationLink } from '$lib/types';

  let { data }: { data: GraphData } = $props();

  let svgEl: SVGElement | undefined = $state();
  let width = $state(0);
  let height = $state(0);

  const simulation: d3.Simulation<SimulationNode, SimulationLink> = d3
    .forceSimulation<SimulationNode>()
    .force('charge', d3.forceManyBody().strength(-100))
    .force('link', d3.forceLink<SimulationNode, SimulationLink>()
        .id((d) => d.id)
        .strength(0.1))
    .force('center', d3.forceCenter());

  $effect(() => {
    if (!svgEl) return;

    const oldNodeMap = new Map<string, SimulationNode>(
      simulation.nodes().map((d) => [d.id, d])
    );

    const nodes = data.nodes.map((d) =>
      Object.assign(oldNodeMap.get(d.id) || { x: width / 2, y: height / 2 }, d)
    );

    simulation.nodes(nodes);
    simulation.force<d3.ForceLink<SimulationNode, SimulationLink>>('link')?.links(data.links);
    simulation.force<d3.ForceCenter<SimulationNode>>('center')?.x(width / 2).y(height / 2);

    const communityFoci = new Map<number, { x: number; y: number }>();
    const communityCount = data.communities.size;
    let i = 0;
    for (const communityId of data.communities.keys()) {
      const angle = (i / communityCount) * 2 * Math.PI;
      communityFoci.set(communityId, {
        x: width / 2 + (width / 4) * Math.cos(angle),
        y: height / 2 + (height / 4) * Math.sin(angle),
      });
      i++;
    }

    simulation
      .force('x', d3.forceX<SimulationNode>((d) => communityFoci.get(d.community_id)?.x ?? width / 2).strength(0.05))
      .force('y', d3.forceY<SimulationNode>((d) => communityFoci.get(d.community_id)?.y ?? height / 2).strength(0.05));

    simulation.alpha(0.4).restart();
  });

  $effect(() => {
    if (!svgEl) return;
    const g = d3.select(svgEl).select<SVGGElement>('g');
    const zoomBehavior = d3.zoom<SVGElement, unknown>().on('zoom', (event) => {
      g.attr('transform', event.transform.toString());
    });
    d3.select(svgEl).call(zoomBehavior);
    return () => d3.select(svgEl).on('.zoom', null);
  });
</script>

<svg bind:this={svgEl} {width} {height}>
  <g>
    <g class="links" stroke="#555" stroke-width="1.5" stroke-opacity="0.6">
      {#each (simulation.force('link') as d3.ForceLink<SimulationNode, SimulationLink>).links() as link}
        {@const source = link.source as SimulationNode}
        {@const target = link.target as SimulationNode}
        <line
          x1={source.x}
          y1={source.y}
          x2={target.x}
          y2={target.y}
        />
      {/each}
    </g>
    <g class="nodes" stroke="#fff" stroke-width="1.5">
      {#each simulation.nodes() as node (node.id)}
        <circle
          cx={node.x}
          cy={node.y}
          r={node.radius}
          fill={node.color}
          class:self={node.id === data.selfId}
        >
          <title>
            ID: {node.id.substring(0, 8)}...
            Community: {node.community_id}
            Value: {node.telemetry.value.toFixed(2)}
          </title>
        </circle>
      {/each}
    </g>
  </g>
</svg>

<svelte:window bind:innerWidth={width} bind:innerHeight={height} />

<style>
  svg {
    width: 100%;
    height: 100%;
    display: block;
    cursor: grab;
  }
  svg:active {
    cursor: grabbing;
  }
  .self {
    stroke: #fde047;
    stroke-width: 3px;
  }
</style>