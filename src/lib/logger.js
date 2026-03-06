/**
 * Frontend log bridge — wraps console.log/info/warn/error/debug to also forward
 * structured events to the Rust backend via IPC, which writes JSONL.
 *
 * Import this module once at app startup (main.js) for it to take effect.
 * Original console methods still work as before.
 */
import { invoke } from '@tauri-apps/api/core'

const _log = console.log.bind(console)
const _info = console.info.bind(console)
const _warn = console.warn.bind(console)
const _error = console.error.bind(console)
const _debug = console.debug.bind(console)

// Prevent high-volume UI debug logs from saturating IPC + backend file writes.
const DROPPED_PREFIXES = ['[filewatch]', '[file] open:', '[code] highlighted']
const RATE_WINDOW_MS = 1000
const MAX_INFO_DEBUG_PER_WINDOW = 25
const DROP_REPORT_INTERVAL_MS = 5000
const INTERACTION_TTL_MS = 2500
const INTERACTION_EVENTS = ['pointerdown', 'click', 'keydown', 'submit']
const INTERACTION_HANDLER_KEY = '__taurhausLoggerInteractionHandler'

let rateWindowStart = Date.now()
let rateWindowCount = 0
let droppedCount = 0
let droppedReasonCounts = {}
let lastDropReportAt = Date.now()
let interactionSequence = 0
let activeInteractionId = null
let activeInteractionAt = 0

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

function normalizeFieldName(key) {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[^a-zA-Z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase()
}

function toJsonSafe(value) {
  if (value === undefined) return undefined
  if (typeof value === 'bigint') return value.toString()
  if (typeof value === 'symbol') return String(value)
  if (typeof value === 'function') return `[function ${value.name || 'anonymous'}]`

  try {
    return JSON.parse(JSON.stringify(value))
  } catch {
    return String(value)
  }
}

function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function extractContext(args) {
  const context = {}
  for (const value of args) {
    if (!isPlainObject(value)) continue
    for (const [rawKey, rawValue] of Object.entries(value)) {
      const key = normalizeFieldName(rawKey)
      if (!key) continue
      const safeValue = toJsonSafe(rawValue)
      if (safeValue !== undefined) context[key] = safeValue
    }
  }
  return Object.keys(context).length > 0 ? context : undefined
}

function classifyForward(level, message, now) {
  if (level === 'warn' || level === 'error') {
    return { forward: true, reason: null }
  }
  if (DROPPED_PREFIXES.some(prefix => message.startsWith(prefix))) {
    return { forward: false, reason: 'prefix_filter' }
  }

  if (now - rateWindowStart >= RATE_WINDOW_MS) {
    rateWindowStart = now
    rateWindowCount = 0
  }
  if (rateWindowCount >= MAX_INFO_DEBUG_PER_WINDOW) {
    return { forward: false, reason: 'rate_limit' }
  }
  rateWindowCount++
  return { forward: true, reason: null }
}

function generateInteractionId(now) {
  interactionSequence++
  return `ix_${now.toString(36)}_${interactionSequence.toString(36)}`
}

function markInteraction() {
  const now = Date.now()
  activeInteractionId = generateInteractionId(now)
  activeInteractionAt = now
}

function currentInteractionId(now) {
  if (!activeInteractionId) return undefined
  if (now - activeInteractionAt > INTERACTION_TTL_MS) return undefined
  return activeInteractionId
}

function installInteractionTracking() {
  if (typeof window === 'undefined') return

  const previousHandler = window[INTERACTION_HANDLER_KEY]
  if (typeof previousHandler === 'function') {
    for (const eventName of INTERACTION_EVENTS) {
      window.removeEventListener(eventName, previousHandler, true)
    }
  }

  window[INTERACTION_HANDLER_KEY] = markInteraction
  for (const eventName of INTERACTION_EVENTS) {
    window.addEventListener(eventName, markInteraction, true)
  }
}

function sendPayload(payload) {
  invoke('frontend_log', { payload }).catch((error) => {
    _warn('[logger] failed to forward frontend log to backend:', error)
  })
}

function noteDrop(reason) {
  droppedCount++
  droppedReasonCounts[reason] = (droppedReasonCounts[reason] ?? 0) + 1
}

function flushDroppedLogs(now) {
  if (droppedCount === 0) return
  if (now - lastDropReportAt < DROP_REPORT_INTERVAL_MS) return

  const payload = {
    level: 'warn',
    component: 'frontend',
    subsystem: 'logger',
    event: 'frontend.logs.dropped',
    message: 'Dropped frontend logs in logger bridge',
    dropped_count: droppedCount,
    dropped_reason_counts: droppedReasonCounts,
  }

  const interactionId = currentInteractionId(now)
  if (interactionId) payload.interaction_id = interactionId

  droppedCount = 0
  droppedReasonCounts = {}
  lastDropReportAt = now
  sendPayload(payload)
}

function forward(level, ...args) {
  const now = Date.now()
  flushDroppedLogs(now)

  const message = serialize(...args)
  const decision = classifyForward(level, message, now)
  if (!decision.forward) {
    noteDrop(decision.reason)
    flushDroppedLogs(now)
    return
  }

  const payload = {
    level,
    component: 'frontend',
    subsystem: 'console',
    event: 'frontend.console.received',
    message,
  }

  const context = extractContext(args)
  if (context) payload.context = context

  const interactionId = currentInteractionId(now)
  if (interactionId) payload.interaction_id = interactionId

  sendPayload(payload)
}

installInteractionTracking()
console.log = (...args) => { _log(...args); forward('info', ...args) }
console.info = (...args) => { _info(...args); forward('info', ...args) }
console.warn = (...args) => { _warn(...args); forward('warn', ...args) }
console.error = (...args) => { _error(...args); forward('error', ...args) }
console.debug = (...args) => { _debug(...args); forward('debug', ...args) }

// Confirm the bridge is active — this line proves IPC is working
console.log('[logger] frontend log bridge initialized')
