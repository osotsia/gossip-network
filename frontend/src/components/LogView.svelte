<script lang="ts">
    // FIX: Corrected import paths to point to .ts files
    import { networkStore } from '../lib/networkStore.svelte.ts';
    import type { LogEntry } from '../lib/networkStore.svelte.ts';

    let logContainer: HTMLElement;

    // Auto-scroll to the top (most recent) when new messages arrive
    $effect(() => {
        // FIX: Access reactive state directly to ensure the effect re-runs on change.
        if (networkStore.log.length > 0 && logContainer) {
            logContainer.scrollTop = 0;
        }
    });

    const typeToClass: Record<LogEntry['type'], string> = {
        info: 'log-info',
        warn: 'log-warn',
        success: 'log-success',
        error: 'log-error',
    };
</script>

<div class="log-view-container">
    <h2>Live Event Log</h2>
    <div class="log-entries" bind:this={logContainer}>
        <!-- FIX: Access reactive state directly from the store object. -->
        {#if networkStore.log.length === 0}
            <div class="log-entry log-placeholder">
                Waiting for WebSocket connection and messages...
            </div>
        {:else}
            {#each networkStore.log as entry (entry.id)}
                <div class="log-entry {typeToClass[entry.type]}">
                    <span class="timestamp">{entry.timestamp.toLocaleTimeString()}</span>
                    <span class="message">{entry.message}</span>
                </div>
            {/each}
        {/if}
    </div>
</div>

<style>
    .log-view-container { display: flex; flex-direction: column; height: 100%; padding: 1.5rem; }
    h2 { margin: 0 0 1rem 0; color: #e0e0e0; font-weight: 500; }
    .log-entries { flex-grow: 1; overflow-y: auto; background-color: #1e1e1e; border-radius: 8px; border: 1px solid #444; font-family: monospace; font-size: 0.9rem; }
    .log-entry { display: flex; gap: 1rem; padding: 0.5rem 1rem; border-bottom: 1px solid #333; }
    .log-entry:last-child { border-bottom: none; }
    .timestamp { color: #888; white-space: nowrap; }
    .message { color: #ccc; }
    .log-placeholder { color: #888; padding: 1rem; }
    .log-info .message { color: #90caf9; }
    .log-success .message { color: #a5d6a7; }
    .log-warn .message { color: #ffe082; }
    .log-error .message { color: #ef9a9a; }
</style>