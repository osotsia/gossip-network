<script lang="ts">
    import Header from './components/Header.svelte';
    import LogView from './components/LogView.svelte';
    import GraphView from './components/GraphView.svelte';
    // REFACTOR: Import the new state object and the connect function.
    import { viewState } from './lib/viewState.svelte.ts';
    import { connect } from './lib/networkState.svelte.ts';

    connect();
</script>

<div class="app-layout">
    <Header />
    <main class="main-content">
        <!-- REFACTOR: Access the `active` property directly from the state object. -->
        {#if viewState.active === 'Log'}
            <LogView />
        {:else if viewState.active === 'Graph'}
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