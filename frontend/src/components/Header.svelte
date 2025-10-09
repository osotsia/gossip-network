<script lang="ts">
    // REFACTOR: Import state objects, constants, and mutator functions from the new state modules.
    import { viewState, viewOrder, setView } from '../lib/viewState.svelte.ts';
    import { networkState, truncateNodeId } from '../lib/networkState.svelte.ts';
</script>

<header class="app-header">
    <div class="title-and-nav">
        <h1>Gossip Network Visualizer</h1>
        <nav aria-label="Main Views">
            <!-- REFACTOR: Use the exported constant and function for view management. -->
            {#each viewOrder as view}
                <button
                    class:active={viewState.active === view}
                    onclick={() => setView(view)}
                    aria-current={viewState.active === view ? 'page' : undefined}
                >
                    {view}
                </button>
            {/each}
        </nav>
    </div>

    <div class="status-wrapper">
        <!-- REFACTOR: Access state properties directly from the reactive state objects. -->
        {#if networkState.selfId}
            <div class="status-item">
                <span class="label">Node ID:</span>
                <span class="value self-id">{truncateNodeId(networkState.selfId)}</span>
            </div>
        {/if}
        <div class="status-item">
            <span class="label">Status:</span>
            <span class="value" class:connected={networkState.isConnected} class:disconnected={!networkState.isConnected}>
                {networkState.isConnected ? 'Connected' : 'Disconnected'}
            </span>
        </div>
    </div>
</header>

<style>
    .app-header { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 1rem; padding: 0.75rem 1.5rem; background-color: #2a2a2e; border-bottom: 1px solid #444; }
    .title-and-nav { display: flex; align-items: center; gap: 2rem; }
    h1 { margin: 0; font-size: 1.25rem; color: #e0e0e0; font-weight: 600; }
    nav { display: flex; gap: 0.5rem; }
    nav button { background: none; border: 1px solid transparent; font-size: 1rem; padding: 0.4rem 1rem; cursor: pointer; border-radius: 6px; color: #aaa; font-weight: 500; transition: all 0.2s ease; }
    nav button:hover { background-color: #38383c; color: #fff; }
    nav button.active { background-color: #007acc; color: #fff; font-weight: 600; border-color: #0099ff; }
    .status-wrapper { display: flex; gap: 1.5rem; align-items: center; font-size: 0.9rem; }
    .status-item { display: flex; align-items: center; gap: 0.5rem; }
    .label { color: #888; }
    .value { font-weight: 600; font-family: monospace; }
    .self-id { color: #9f86ff; }
    .connected { color: #4ade80; }
    .disconnected { color: #f87171; }
</style>