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

// Prevent high-volume UI debug logs from saturating IPC + backend file writes.
const DROPPED_PREFIXES = ['[filewatch]', '[file] open:', '[code] highlighted']
const RATE_WINDOW_MS = 1000
const MAX_INFO_DEBUG_PER_WINDOW = 25
let rateWindowStart = Date.now()
let rateWindowCount = 0

function serialize(...args) {
  return args
    .map((value) => {
      if (typeof value === 'string') return value
      if (typeof value === 'bigint') return value.toString()
      try {
        const serialized = JSON.stringify(value, null, 0)
        return serialized === undefined ? String(value) : serialized
      } catch {
        return '[unserializable]'
      }
    })
    .join(' ')
}

function shouldForward(level, message) {
  if (level === 'warn' || level === 'error') return true
  if (DROPPED_PREFIXES.some(prefix => message.startsWith(prefix))) return false

  const now = Date.now()
  if (now - rateWindowStart >= RATE_WINDOW_MS) {
    rateWindowStart = now
    rateWindowCount = 0
  }
  if (rateWindowCount >= MAX_INFO_DEBUG_PER_WINDOW) {
    return false
  }
  rateWindowCount++
  return true
}

function forward(level, ...args) {
  const message = serialize(...args)
  if (!shouldForward(level, message)) return
  invoke('frontend_log', { level, message }).catch((error) => {
    console.warn('[logger] failed to forward frontend log to backend:', error)
  })
}

console.log = (...args) => { _log(...args); forward('info', ...args) }
console.warn = (...args) => { _warn(...args); forward('warn', ...args) }
console.error = (...args) => { _error(...args); forward('error', ...args) }
console.debug = (...args) => { _debug(...args); forward('debug', ...args) }

// Confirm the bridge is active — this line proves IPC is working
console.log('[logger] frontend log bridge initialized')
