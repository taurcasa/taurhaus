import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

function projectPath(relativePath) {
  return fileURLToPath(new URL(relativePath, import.meta.url))
}

const visualHostRootPlugin = {
  name: 'visual-host-root-rewrite',
  configureServer(server) {
    server.middlewares.use((req, _res, next) => {
      if (req.url === '/' || req.url === '/index.html') {
        req.url = '/visual-host.html'
      }
      next()
    })
  },
}

export default defineConfig({
  plugins: [svelte(), tailwindcss(), visualHostRootPlugin],
  clearScreen: false,
  appType: 'mpa',
  resolve: {
    alias: [
      {
        find: projectPath('./src/lib/ipc.js'),
        replacement: projectPath('./src/visual-host/mocks/ipc.js'),
      },
      {
        find: projectPath('./src/lib/sessionStore.svelte.js'),
        replacement: projectPath('./src/visual-host/mocks/sessionStore.svelte.js'),
      },
    ],
  },
  build: {
    rollupOptions: {
      input: {
        visual: projectPath('./visual-host.html'),
      },
    },
  },
  server: {
    host: '127.0.0.1',
    port: 1425,
    strictPort: true,
  },
})
