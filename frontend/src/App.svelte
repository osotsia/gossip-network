<script lang="ts">
    import Header from './components/Header.svelte';
    import LogView from './components/LogView.svelte';
    import GraphView from './components/GraphView.svelte';
    // FIX: Import the single viewStore object.
    import { viewStore } from './lib/viewStore.svelte.ts';
    import { networkStore } from './lib/networkStore.svelte.ts';

    networkStore.connect();
</script>

<div class="app-layout">
    <Header />
    <main class="main-content">
        <!-- FIX: Access the activeView property from the viewStore object. -->
        {#if viewStore.activeView === 'Log'}
            <LogView />
        {:else if viewStore.activeView === 'Graph'}
            <GraphView />
        {/if}
    </main>
</div>

<style>
    :global(body) {
        margin: 0;
        background-color: #222;
        color: #eee;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif;
    }

    .app-layout {
        display: flex;
        flex-direction: column;
        height: 100vh;
        width: 100vw;
    }

    .main-content {
        flex-grow: 1;
        overflow: hidden;
    }
</style>