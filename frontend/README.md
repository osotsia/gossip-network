# Frontend Architecture (Svelte 5)

This document provides an analysis of the frontend visualizer, built with Svelte 5. It details the architectural principles, state management patterns, and component responsibilities.

## 1. Architectural Philosophy

The frontend is designed around modern, compiler-first web principles, leveraging the full capabilities of Svelte 5.

*   **Compiler-First Framework:** The application is built with Svelte 5, which shifts the bulk of the work from a client-side runtime (like a Virtual DOM) to a compile step. This results in highly optimized, minimal JavaScript bundles that directly manipulate the DOM, leading to superior performance.

*   **Runes-Based Reactivity:** The core of the application's reactivity is managed by Svelte 5's runes (`$state`, `$effect`). This provides a granular and predictable system for tracking state changes and updating the UI, eliminating the complexities of older lifecycle models and store subscriptions.

*   **Component-Centric Design:** The UI is decomposed into logical, reusable components (`Header`, `LogView`, `GraphView`). Each component encapsulates its own markup, styling, and logic, promoting modularity and maintainability.

*   **Centralized, Reactive State:** Application state is managed in dedicated TypeScript modules (`networkState.svelte.ts`, `viewState.svelte.ts`). These modules export deeply reactive `$state` objects, providing a single source of truth that components can reactively consume without the boilerplate of traditional store patterns.

## 2. Folder and File Structure

The `frontend/` directory contains all assets and source code for the visualizer application.

```
frontend/
├── dist/                     # (Auto-generated) The compiled, static output served by the backend.
├── node_modules/             # Project dependencies.
├── public/                   # Static assets copied to `dist` (e.g., favicons).
├── src/
│   ├── components/
│   │   ├── GraphView.svelte  # D3.js powered network graph visualization.
│   │   ├── Header.svelte     # Top navigation bar and status display.
│   │   └── LogView.svelte    # Real-time log of network events.
│   ├── lib/
│   │   ├── networkState.svelte.ts # Central state management for all network data.
│   │   ├── types.ts          # TypeScript types for the WebSocket protocol.
│   │   └── viewState.svelte.ts    # State management for UI view selection.
│   ├── App.svelte            # The root component of the application.
│   └── main.ts               # The application's entry point.
├── .gitignore
├── index.html                # The HTML shell for the single-page application.
├── package.json              # Project metadata and dependencies.
├── svelte.config.js          # Svelte compiler configuration (enables runes).
├── tsconfig.json             # TypeScript configuration.
└── vite.config.ts            # Vite build tool configuration.
```

## 3. Core Concepts: Reactivity & State Management

The frontend abandons the traditional Svelte store pattern in favor of a more direct, rune-based state management architecture.

### 3.1. Runes-Based Reactivity
State is declared using `$state`. When a value managed by `$state` is mutated, Svelte's compiler ensures that any part of the application that depends on this value is automatically and efficiently updated. Effects, which run code in response to state changes (such as redrawing a D3 graph), are managed with the `$effect` rune.

### 3.2. Centralized State Modules
All application state is consolidated into two key modules:
*   **`src/lib/networkState.svelte.ts`:** This module exports a single, deeply reactive `$state` object called `networkState`. It holds all data related to the gossip network, such as the list of nodes, active connections, and event logs. It also contains the `connect` function, which establishes the WebSocket connection and mutates the `networkState` object in response to server messages.
*   **`src/lib/viewState.svelte.ts`:** A simpler module that manages the UI state, specifically which view (`Log` or `Graph`) is currently active.

This pattern provides a clean, centralized API for state. Components import `networkState` and can directly reference its properties (e.g., `networkState.nodes`). When the WebSocket handler updates `networkState.nodes`, all components using that property will reactively update.

**Example: `networkState.svelte.ts`**
```typescript
// All reactive state is consolidated into a single exported `$state` object.
export const networkState = $state({
    isConnected: false,
    selfId: null as NodeId | null,
    nodes: {} as Record<NodeId, NodeInfo>,
    // ...
});

// The connect function mutates this object directly.
export function connect() {
    ws.onmessage = (event) => {
        // ... logic to parse message
        // Direct mutation of the state object triggers UI updates globally.
        networkState.nodes = newNodes;
    };
}
```

## 4. Component Breakdown

*   **`App.svelte`:** The root component. It initializes the WebSocket connection by calling `connect()` and acts as a router, conditionally rendering either `LogView` or `GraphView` based on the value of `viewState.active`.

*   **`Header.svelte`:** The main navigation and status bar. It reads connection status and the local node's ID from `networkState` to display them. It uses `setView()` from `viewState.svelte.ts` to switch between the Log and Graph views.

*   **`LogView.svelte`:** A simple component that displays a chronological list of network events. It iterates over the `networkState.log` array. New entries are added to the bottom, and the view automatically scrolls to show the latest event. A subtle `fade` transition highlights new entries.

*   **`GraphView.svelte`:** The most complex component, responsible for rendering the interactive network graph using D3.js. It is a prime example of integrating a third-party library with Svelte 5's reactivity model.

## 5. Data Flow: WebSocket to UI

The frontend is kept in sync with the backend via a simple but effective WebSocket protocol.

1.  **Connection:** `App.svelte` calls the `connect()` function in `networkState.svelte.ts`.
2.  **Initial State:** Upon connection, the server sends a `snapshot` message containing the entire known network state. The `onmessage` handler in `networkState` populates `networkState.nodes`, `networkState.selfId`, etc.
3.  **Incremental Updates:** Subsequently, the server sends small `update` messages for specific events (`node_added`, `connection_status`, etc.). The `onmessage` handler applies these deltas to the `networkState` object.
4.  **Reactivity:** Because `networkState` is a `$state` proxy, any mutation (e.g., adding a node, changing a connection status) is detected by Svelte.
5.  **UI Update:** All components and effects that depend on the mutated properties are automatically re-rendered or re-run. For example, when `networkState.nodes` changes, the `GraphView` component's main `$effect` re-runs to update the D3 visualization.

### Edge Pulse Animation Flow
A dedicated event, `animate_edge`, triggers the pulsing animation on the graph.
1.  The backend sends an `animate_edge` update when it receives gossip from a peer.
2.  The `onmessage` handler in `networkState.svelte.ts` adds the peer ID to a temporary `pendingPulsePeers` set.
3.  To prevent visual "storms" from many rapid updates, these events are batched. A `setTimeout` of 50ms collects all events within that window.
4.  After 50ms, the batched peer IDs are moved to the reactive `networkState.currentPulsePeers` set.
5.  This change triggers the nested animation `$effect` inside `GraphView.svelte`, which applies a CSS class to the corresponding edge to trigger the animation.
6.  After the animation duration (750ms), `currentPulsePeers` is cleared, resetting the state.

## 6. Visualization with D3.js

The integration between D3.js and Svelte 5 in `GraphView.svelte` is designed for performance and clarity.

*   **Main `$effect` for Structure:** A top-level `$effect` is responsible for all structural aspects of the graph. It runs once for initial setup and then re-runs only when `networkState.nodes` or `networkState.activeConnections` changes. Its responsibilities include:
    *   Initializing the D3 force simulation.
    *   Performing D3's data join pattern (`.data()`, `.enter()`, `.exit()`) to add, update, or remove node and link elements.
    *   Restarting the simulation when the graph topology changes.

*   **Pinned Central Node:** The visualizer's own node is fixed to the center of the SVG. This is achieved by setting its `fx` and `fy` properties in the D3 simulation and preventing it from being dragged via the `.filter()` method on the D3 drag handler. This provides a stable anchor point for the rest of the dynamic graph.

*   **Nested `$effect` for Animation:** A nested `$effect` is used exclusively for the edge pulse animation. It *only* subscribes to `networkState.currentPulsePeers`. This is a critical performance optimization: it ensures that the expensive graph restructuring logic in the parent effect does not re-run for simple, frequent animation events.

## 7. Build Process

The frontend is a standard Vite project.
*   `npm install` installs dependencies like Svelte and D3.
*   `npm run dev` starts the Vite development server with Hot Module Replacement (HMR) for a fast development loop. A proxy is configured in `vite.config.ts` to forward WebSocket requests to the Rust backend.
*   `npm run build` uses Vite to compile and bundle the application into a set of static HTML, CSS, and JavaScript files located in the `frontend/dist/` directory. These static assets are then served directly by the Rust backend, making the application self-contained.