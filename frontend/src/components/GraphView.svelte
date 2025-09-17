<script lang="ts">
	// FIX: Corrected import path
	import { networkStore } from '../lib/networkStore.svelte.ts';

	let svgElement: SVGSVGElement;
    
    // The `$effect` rune is the Svelte 5 replacement for onMount/afterUpdate.
    $effect(() => {
        if (svgElement) {
            // D3 initialization logic would go here.
            
            // For now, we just log to show it's reacting to data changes.
            // FIX: Access reactive state directly to ensure the effect re-runs on change.
            console.log(
                `D3 placeholder effect: ${Object.keys(networkStore.nodes).length} nodes, ${networkStore.activeConnections.size} connections.`
            );
        }
    });
</script>

<div class="graph-container">
    <div class="stats-bar">
		<!-- FIX: Access reactive state properties directly from the store object.
		     Destructuring them into constants would break reactivity. -->
        <span>Nodes: {Object.keys(networkStore.nodes).length}</span>
        <span>Active Connections: {networkStore.activeConnections.size}</span>
    </div>
    <div class="svg-wrapper">
        <svg bind:this={svgElement} width="100%" height="100%" viewbox="0 0 800 600" preserveAspectRatio="xMidYMid meet">
            <defs>
                <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
                    <feGaussianBlur stdDeviation="3.5" result="coloredBlur"/>
                    <feMerge>
                        <feMergeNode in="coloredBlur"/>
                        <feMergeNode in="SourceGraphic"/>
                    </feMerge>
                </filter>
            </defs>
            
            <rect width="100%" height="100%" fill="#1e1e1e" />
            <text x="400" y="280" font-size="24" text-anchor="middle" fill="#888">
                D3 Graph Placeholder
            </text>
            <text x="400" y="320" font-size="16" text-anchor="middle" fill="#666">
                (Animation logic to be implemented here)
            </text>

            <g opacity="0.5">
                <line x1="250" y1="300" x2="400" y2="200" stroke="#007acc" stroke-width="2" />
                <line x1="400" y1="200" x2="550" y2="300" stroke="#007acc" stroke-width="2" />
                <line x1="550" y1="300" x2="250" y2="300" stroke="#444" stroke-width="1.5" stroke-dasharray="4" />

                <circle cx="250" cy="300" r="15" fill="#2a2a2e" stroke="#0099ff" stroke-width="3" filter="url(#glow)"/>
                <circle cx="400" cy="200" r="15" fill="#2a2a2e" stroke="#4ade80" stroke-width="3" filter="url(#glow)"/>
                <circle cx="550" cy="300" r="15" fill="#2a2a2e" stroke="#0099ff" stroke-width="3" filter="url(#glow)"/>
            </g>
        </svg>
    </div>
</div>

<style>
    .graph-container { display: flex; flex-direction: column; height: 100%; padding: 1.5rem; gap: 1rem; }
    .stats-bar { color: #ccc; display: flex; gap: 2rem; background: #2a2a2e; padding: 0.5rem 1rem; border-radius: 6px; border: 1px solid #444; font-family: monospace; }
    .svg-wrapper { flex-grow: 1; border: 1px solid #444; border-radius: 8px; overflow: hidden; }
</style>