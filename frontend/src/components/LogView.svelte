<script lang="ts">
    import { networkState } from '../lib/networkState.svelte.ts';
    import type { LogEntry } from '../lib/networkState.svelte.ts';
    // MODIFICATION: Import the `fade` transition for new log entries.
    import { fade } from 'svelte/transition';

    let logContainer: HTMLElement;

    // MODIFICATION: Auto-scroll to the BOTTOM when new messages arrive.
    $effect(() => {
        // This effect runs after the DOM has been updated.
        if (networkState.log.length > 0 && logContainer) {
            // A potential improvement here would be to only auto-scroll if the
            // user is already near the bottom of the log, to avoid interrupting
            // them if they have scrolled up to view older messages.
            logContainer.scrollTop = logContainer.scrollHeight;
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
        {#if networkState.log.length === 0}
            <div class="log-entry log-placeholder">
                Waiting for WebSocket connection and messages...
            </div>
        {:else}
            {#each networkState.log as entry (entry.id)}
                <!-- MODIFICATION: Add a fade-in transition to new log entries. -->
                <div class="log-entry {typeToClass[entry.type]}" in:fade={{ duration: 300 }}>
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
    .log-entry:first-child { border-top: none; }
    .timestamp { color: #888; white-space: nowrap; }
    .message { color: #ccc; }
    .log-placeholder { color: #888; padding: 1rem; }
    .log-info .message { color: #90caf9; }
    .log-success .message { color: #a5d6a7; }
    .log-warn .message { color: #ffe082; }
    .log-error .message { color: #ef9a9a; }
</style>