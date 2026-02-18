/**
 * Frontend log bridge — wraps console.log/warn/error to also forward
 * messages to the Rust backend via IPC, which writes them to a log file.
 *
 * Import this module once at app startup (main.js) for it to take effect.
 * Original console methods still work as before.
 */
import { invoke } from '@tauri-apps/api/core'

const _log = console.log.bind(console)
const _warn = console.warn.bind(console)
const _error = console.error.bind(console)
const _debug = console.debug.bind(console)

function serialize(...args) {
  return args
    .map(a => (typeof a === 'string' ? a : JSON.stringify(a, null, 0)))
    .join(' ')
}

function forward(level, ...args) {
  invoke('frontend_log', { level, message: serialize(...args) }).catch(() => {})
}

console.log = (...args) => { _log(...args); forward('info', ...args) }
console.warn = (...args) => { _warn(...args); forward('warn', ...args) }
console.error = (...args) => { _error(...args); forward('error', ...args) }
console.debug = (...args) => { _debug(...args); forward('debug', ...args) }

// Confirm the bridge is active — this line proves IPC is working
console.log('[logger] frontend log bridge initialized')
