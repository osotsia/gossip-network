// src/lib/viewState.svelte.ts

type View = 'Log' | 'Graph';

// REFACTOR: The state is now a single, exported `$state` object.
// This allows for deep reactivity and avoids the need for getter/setter boilerplate.
export const viewState = $state({
    active: 'Log' as View,
});

// REFACTOR: Related constants are exported directly from the module.
export const viewOrder: readonly View[] = ['Log', 'Graph'];

// REFACTOR: State mutations are handled by simple exported functions.
export function setView(view: View) {
    viewState.active = view;
}