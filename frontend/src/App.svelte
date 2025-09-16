<script lang="ts">
	import { networkStore } from '$lib/networkStore.svelte.ts';
	import NetworkGraph from '$lib/NetworkGraph.svelte';

	// No changes to the script section are needed.
	networkStore.init();
</script>

<header>
	Gossip Network Status:
	<span class="status {networkStore.status}">
		{networkStore.status.toUpperCase()}
	</span>
	<!-- FIX: Use optional chaining (?.) to prevent a crash if networkStore.graph is null during initial render. -->
	{#if networkStore.status === 'connected' && networkStore.graph?.nodes}
		| Nodes: {networkStore.graph.nodes.length} | Communities: {networkStore.graph.communities.size}
	{/if}
</header>

<main>
	<!-- FIX: Apply the same optional chaining guard here. -->
	{#if networkStore.status === 'connected' && networkStore.graph?.nodes.length > 0}
		<NetworkGraph data={networkStore.graph} />
	{:else}
		<div class="placeholder">
			<h1>{networkStore.status.toUpperCase()}...</h1>
			<p>Waiting for data from the gossip network visualizer node.</p>
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