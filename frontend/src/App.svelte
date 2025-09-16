<script>
  import { networkStore } from '$lib/networkStore.svelte.js';
  import NetworkGraph from '$lib/NetworkGraph.svelte';

  // Initiate the WebSocket connection once.
  networkStore.connect();
</script>

<header>
  Gossip Network Status:
  <span class="status {networkStore.status}">
    {networkStore.status.toUpperCase()}
  </span>
  {#if networkStore.status === 'connected'}
    | Nodes: {networkStore.graph.nodes.length} | Communities: {networkStore.graph.communities.size}
  {/if}
</header>

<main>
  {#if networkStore.status === 'connected' && networkStore.graph.nodes.length > 0}
    <!-- Pass the reactive graph data as a prop -->
    <NetworkGraph data={networkStore.graph} />
  {:else}
    <div class="placeholder">
      <h1>{networkStore.status.toUpperCase()}...</h1>
      <p>Waiting for connection to the gossip network visualizer node.</p>
    </div>
  {/if}
</main>

<style>
  main {
    flex-grow: 1;
    display: flex;
    justify-content: center;
    align-items: center;
    width: 100%;
    height: 100%;
  }

  .placeholder {
    color: #666;
  }
</style>