<script>
  import * as d3 from 'd3';

  // The 'data' prop is now a plain object, but Svelte knows to
  // update this component when it changes because of the reactive
  // getter in the parent (`networkStore.graph`).
  let { data } = $props();

  let svgEl = $state();
  let width = $state(0);
  let height = $state(0);

  const simulation = d3
    .forceSimulation()
    .force('charge', d3.forceManyBody().strength(-100))
    .force('link', d3.forceLink().id((d) => d.id).strength(0.1))
    .force('center', d3.forceCenter());

  $effect(() => {
    if (!svgEl) return;

    const oldNodeMap = new Map(simulation.nodes().map((d) => [d.id, d]));
    const nodes = data.nodes.map((d) =>
      Object.assign(oldNodeMap.get(d.id) || { x: width / 2, y: height / 2 }, d)
    );

    simulation.nodes(nodes);
    simulation.force('link').links(data.links);
    simulation.force('center').x(width / 2).y(height / 2);

    const communityFoci = new Map();
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

    simulation.force('x', d3.forceX((d) => communityFoci.get(d.community_id)?.x || width / 2).strength(0.05));
    simulation.force('y', d3.forceY((d) => communityFoci.get(d.community_id)?.y || height / 2).strength(0.05));

    simulation.alpha(0.4).restart();
  });

  $effect(() => {
    if (!svgEl) return;
    const g = d3.select(svgEl).select('g');
    const zoom = d3.zoom().on('zoom', (event) => {
      g.attr('transform', event.transform);
    });
    d3.select(svgEl).call(zoom);
    return () => d3.select(svgEl).on('.zoom', null);
  });
</script>

<svg bind:this={svgEl} {width} {height}>
  <g>
    <g class="links" stroke="#555" stroke-width="1.5" stroke-opacity="0.6">
      <!-- Optimization: Iterate over the simulation's links. The `source` and `target`
           properties are replaced by D3 with direct references to the node objects,
           which contain the calculated x/y coordinates. This is much more efficient
           than searching for nodes in an array on every render. -->
      {#each simulation.force('link').links() as link}
        <line
          x1={link.source.x}
          y1={link.source.y}
          x2={link.target.x}
          y2={link.target.y}
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