import './lib/logger.js'
import './app.css'
import App from './App.svelte'
import { mount } from 'svelte'

// Disable the default WebView context menu. Custom context menus are added
// per-component where useful (sidebar, file tree, git commits).
document.addEventListener('contextmenu', (e) => {
  // Allow default context menu on text inputs and textareas for cut/copy/paste
  const tag = e.target?.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA') return
  e.preventDefault()
})

const app = mount(App, { target: document.getElementById('app') })

export default app
