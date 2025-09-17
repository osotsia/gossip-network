import { defineConfig } from 'vite'
// 1. Import vitePreprocess from the plugin
import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    // 2. Pass the configuration object directly to svelte()
    svelte({
      // This ensures TypeScript and other languages are preprocessed
      preprocess: vitePreprocess(),

      // This explicitly enables Svelte 5 runes mode for the compiler
      compilerOptions: {
        runes: true,
      },
    }),
  ],
  server: {
    // This proxy is essential for the dev server
    proxy: {
      '/ws': {
        target: 'ws://127.0.0.1:8080',
        ws: true,
      },
    },
  },
})