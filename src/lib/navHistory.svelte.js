/**
 * Navigation history store — tracks view state for back/forward navigation.
 *
 * Entry shape: { tab, file?, lineNumber?, commit?, rangeFilter?, subTab? }
 *
 * Usage:
 *   import { push, goBack, goForward, canGoBack, canGoForward, reset, withSuppressed } from './navHistory.svelte.js'
 *   push({ tab: 'git', commit: 'abc123' })
 *   const entry = goBack()   // or goForward()
 *   withSuppressed(() => { ... })  // replay without recording
 */

const MAX_STACK = 50

let stack = $state([])
let cursor = $state(-1)
let suppressed = false

function entriesEqual(a, b) {
  if (!a || !b) return false
  return a.tab === b.tab &&
    a.file === b.file &&
    a.lineNumber === b.lineNumber &&
    a.commit === b.commit &&
    a.subTab === b.subTab &&
    JSON.stringify(a.rangeFilter) === JSON.stringify(b.rangeFilter)
}

/** Record a navigation action. Truncates forward history. */
export function push(entry) {
  if (suppressed) return
  // Dedup: skip if identical to current position
  if (cursor >= 0 && entriesEqual(stack[cursor], entry)) return

  // Truncate forward history and append
  stack = [...stack.slice(0, cursor + 1), entry]
  cursor = stack.length - 1

  // Cap at max
  if (stack.length > MAX_STACK) {
    stack = stack.slice(stack.length - MAX_STACK)
    cursor = stack.length - 1
  }
}

/** Navigate back. Returns the entry to restore, or null. */
export function goBack() {
  if (cursor <= 0) return null
  cursor--
  return stack[cursor]
}

/** Navigate forward. Returns the entry to restore, or null. */
export function goForward() {
  if (cursor >= stack.length - 1) return null
  cursor++
  return stack[cursor]
}

/** Whether back navigation is possible. */
/** @public Test and shell consumers can query back-navigation availability. */
export function canGoBack() {
  return cursor > 0
}

/** Whether forward navigation is possible. */
/** @public Test and shell consumers can query forward-navigation availability. */
export function canGoForward() {
  return cursor < stack.length - 1
}

/** Clear all history (called on project switch). */
export function reset() {
  stack = []
  cursor = -1
}

/** Execute fn without recording pushes (for replay during back/forward). */
export function withSuppressed(fn) {
  suppressed = true
  try {
    fn()
  } finally {
    suppressed = false
  }
}
