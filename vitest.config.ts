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
    // Browser-mode specs run in `just test-visual`, not in the jsdom lane; the
    // rest of src/test/visual (mock registry guards) belongs here.
    exclude: ['src/test/visual/specs/**'],
    globals: true,
    setupFiles: [],
  },
})
