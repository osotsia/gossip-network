import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

// ADD THIS LINE FOR DIAGNOSTICS
console.log('--- VITE CONFIG LOADED ---');

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/ws': {
        target: 'ws://127.0.0.1:8080',
        ws: true,
      },
    },
  },
  // Add the resolve.alias configuration here.
  // This tells Vite how to interpret the '$lib' import path.
  resolve: {
    alias: {
      $lib: path.resolve(__dirname, './src/lib'),
    },
  },
});