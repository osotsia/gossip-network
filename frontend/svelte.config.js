import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

export default {
  // Add this compilerOptions block
  compilerOptions: {
    runes: true,
  },

  preprocess: vitePreprocess(),
}