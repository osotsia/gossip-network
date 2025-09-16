<script>
  import * as d3 from 'd3';

  // --- Props & State ---

  /**
   * The processed graph data from the network store.
   * @type {import('./networkStore.js').networkStore.graph}
   */
  let { data } = $props();
  let svgEl = $state(); // Bound to the <svg> element
  let width = $state(0);
  let height = $state(0);

  // --- D3 Simulation Setup ---

  // The force simulation is the physics engine that positions our nodes.
  // It is created once and its data is updated reactively.
  const simulation = d3
    .forceSimulation()
    .force('charge', d3.forceManyBody().strength(-100)) // Nodes repel each other
    .force('link', d3.forceLink().id((d) => d.id).strength(0.1)) // Links pull nodes together
    .force('center', d3.forceCenter()); // A global force pulling everything to the center

  // --- Reactive Effects (Svelte 5) ---

  /**
   * This effect synchronizes the Svelte component's `data` prop with the D3 simulation.
   * It runs whenever `data`, `width`, or `height` changes.
   */
  $effect(() => {
    if (!svgEl) return;

    // Preserve existing node positions for smooth transitions.
    // When new data arrives, we find the old node with the same ID and copy its
    // position (`x`, `y`) and velocity (`vx`, `vy`) to the new node object.
    const oldNodeMap = new Map(simulation.nodes().map((d) => [d.id, d]));
    const nodes = data.nodes.map((d) =>
      Object.assign(oldNodeMap.get(d.id) || { x: width / 2, y: height / 2 }, d)
    );

    // Update the simulation with the new data.
    simulation.nodes(nodes);
    simulation.force('link').links(data.links);
    simulation.force('center').x(width / 2).y(height / 2);

    // A map of community ID to its center point.
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

    // Apply clustering forces. These gently pull nodes towards their community's focus point.
    simulation.force('x', d3.forceX((d) => communityFoci.get(d.community_id)?.x || width / 2).strength(0.05));
    simulation.force('y', d3.forceY((d) => communityFoci.get(d.community_id)?.y || height / 2).strength(0.05));

    // Give the simulation a "kick" to re-heat it when data changes.
    simulation.alpha(0.4).restart();
  });

  /**
   * This effect handles the D3 zoom and pan behavior.
   * It's set up once and attaches event listeners to the SVG.
   */
  $effect(() => {
    if (!svgEl) return;
    const g = d3.select(svgEl).select('g');
    const zoom = d3.zoom().on('zoom', (event) => {
      g.attr('transform', event.transform);
    });
    d3.select(svgEl).call(zoom);

    // Cleanup function to remove listeners when the component is destroyed.
    return () => d3.select(svgEl).on('.zoom', null);
  });
</script>

<!--
  The SVG is bound to the component's dimensions.
  A svelte:window binding is used to reactively update width/height on resize.
-->
<svg bind:this={svgEl} {width} {height}>
  <!-- This group is for zoom/pan transformations -->
  <g>
    <!-- Render links -->
    <g class="links" stroke="#555" stroke-width="1.5" stroke-opacity="0.6">
      {#each data.links as link}
        {@const sourceNode = simulation.nodes().find(n => n.id === link.source)}
        {@const targetNode = simulation.nodes().find(n => n.id === link.target)}
        {#if sourceNode && targetNode}
          <line
            x1={sourceNode.x}
            y1={sourceNode.y}
            x2={targetNode.x}
            y2={targetNode.y}
          />
        {/if}
      {/each}
    </g>

    <!-- Render nodes -->
    <g class="nodes" stroke="#fff" stroke-width="1.5">
      <!-- D3's simulation mutates the node objects with x/y positions on each "tick".
           Svelte's #each block reactively updates the DOM. -->
      {#each simulation.nodes() as node (node.id)}
        <circle
          cx={node.x}
          cy={node.y}
          r={node.radius}
          fill={node.color}
          class:self={node.id === data.selfId}
        >
          <!-- Tooltip shows a shortened Node ID -->
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

  /* Highlight the node that is serving the visualizer */
  .self {
    stroke: #fde047; /* yellow-400 */
    stroke-width: 3px;
  }
</style>