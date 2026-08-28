import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: {
    conditions: ['browser'],
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.{js,ts}', 'e2e/**/*.test.{js,ts}', 'scripts/**/*.test.mjs'],
    exclude: ['src/test/visual/**'],
    globals: true,
    setupFiles: [],
  },
})
