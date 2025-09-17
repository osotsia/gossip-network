import { mount } from 'svelte';
import App from './App.svelte'

// Use the Svelte 5 `mount` API instead of the deprecated `new App({ target })`
const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app