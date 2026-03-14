import './app.css'
import './lib/logger.js'
import { mount } from 'svelte'
import { renderStartupFailure, extractStartupErrorMessage } from './lib/startupFailure.js'

// Disable the default WebView context menu. Custom context menus are added
// per-component where useful (sidebar, file tree, git commits).
document.addEventListener('contextmenu', (e) => {
  // Allow default context menu on text inputs and textareas for cut/copy/paste
  const tag = e.target?.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA') return
  e.preventDefault()
})

window.addEventListener('error', (event) => {
  console.error('[startup] uncaught window error:', event.error ?? event.message)
  renderStartupFailure(event.error ?? event.message)
})

window.addEventListener('unhandledrejection', (event) => {
  console.error('[startup] unhandled promise rejection:', event.reason)
  renderStartupFailure(event.reason)
})

let app = null

async function boot() {
  try {
    const { default: App } = await import('./App.svelte')
    app = mount(App, { target: document.getElementById('app') })
  } catch (error) {
    console.error('[startup] app mount failed:', error)
    renderStartupFailure(extractStartupErrorMessage(error))
  }
}

void boot()

export default app
