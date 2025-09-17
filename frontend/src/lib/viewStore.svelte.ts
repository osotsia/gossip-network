// src/lib/viewStore.svelte.ts

type View = 'Log' | 'Graph';

// 1. The reactive state is now a private, un-exported variable within the module.
let activeView = $state<View>('Log');

// 2. Export a single store object that exposes a controlled API.
export const viewStore = {
    /** A getter provides reactive, read-only access to the state. */
    get activeView() {
        return activeView;
    },

    /** A method provides a safe way to mutate the internal state. */
    setView(view: View) {
        activeView = view;
    },

    /** Related constants are part of the store's API for consistency. */
    viewOrder: ['Log', 'Graph'] as const,
};