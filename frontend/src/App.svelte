<script>
  import { networkStore } from '$lib/networkStore.js';
  import NetworkGraph from '$lib/NetworkGraph.svelte';

  // Initiate the WebSocket connection when the app starts.
  networkStore.connect();

  // Make the store's reactive properties available to the template.
  const graph = $derived(networkStore.graph);
  const status = $derived(networkStore.status);
</script>

<header>
  Gossip Network Status:
  <span class="status {status}">
    {status.toUpperCase()}
  </span>
  {#if status === 'connected'}
    | Nodes: {graph.nodes.length} | Communities: {graph.communities.size}
  {/if}
</header>

<main>
  {#if status === 'connected' && graph.nodes.length > 0}
    <NetworkGraph data={graph} />
  {:else}
    <div class="placeholder">
      <h1>{status.toUpperCase()}...</h1>
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