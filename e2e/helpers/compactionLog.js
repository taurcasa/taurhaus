/**
 * Reading the structured JSONL log an E2E app session writes.
 *
 * The compaction hook runs in its own process (`taurhaus --compact-hook`), so
 * its evidence never reaches the WebDriver session — it only reaches
 * `<TAURHAUS_DATA_DIR>/taurhaus.log.jsonl`. These helpers read that file
 * incrementally so one test case only sees the events its own actions produced.
 */

import { openSync, readSync, closeSync, statSync } from 'node:fs'

/** Parse newline-delimited JSON, dropping blank and malformed lines. */
export function parseLogEvents(raw) {
  return String(raw ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      try {
        const parsed = JSON.parse(line)
        return parsed && typeof parsed === 'object' ? [parsed] : []
      } catch {
        return []
      }
    })
}

/**
 * Read the records appended to `path` after `offset`.
 *
 * Returns the new events plus the offset to pass on the next call. A trailing
 * partial line is left unconsumed so a record still being written is read whole
 * on the next call. A file shorter than the offset was rotated, so it restarts
 * from the beginning.
 */
export function readLogEventsSince(path, offset = 0) {
  let size
  try {
    size = statSync(path).size
  } catch {
    return { events: [], offset }
  }

  const start = size < offset ? 0 : offset
  if (size === start) return { events: [], offset: start }

  const buffer = Buffer.alloc(size - start)
  const fd = openSync(path, 'r')
  let bytesRead
  try {
    bytesRead = readSync(fd, buffer, 0, buffer.length, start)
  } finally {
    closeSync(fd)
  }

  const chunk = buffer.subarray(0, bytesRead).toString('utf8')
  const lastNewline = chunk.lastIndexOf('\n')
  if (lastNewline < 0) return { events: [], offset: start }

  const complete = chunk.slice(0, lastNewline + 1)
  return {
    events: parseLogEvents(complete),
    offset: start + Buffer.byteLength(complete, 'utf8'),
  }
}

/**
 * Filter records by event name (exact or prefix) and top-level field equality.
 * `emit_global` flattens its context fields onto the record, so `match` reads
 * them directly.
 */
export function selectEvents(events, { event, eventPrefix, match } = {}) {
  const entries = Object.entries(match ?? {})
  return (events ?? []).filter((record) => {
    const name = record?.event
    if (event && name !== event) return false
    if (eventPrefix && !String(name ?? '').startsWith(eventPrefix)) return false
    return entries.every(([key, value]) => record?.[key] === value)
  })
}
