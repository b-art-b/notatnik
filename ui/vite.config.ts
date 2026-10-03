import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [svelte()],
  // Tauri serves the built files from disk, so asset URLs must be relative.
  base: './',
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: '127.0.0.1' },
  preview: { port: 4173, strictPort: true, host: '127.0.0.1' },
  build: { target: 'es2023', sourcemap: true },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
  },
})
