import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync, appendFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { parseLogEvents, readLogEventsSince, selectEvents } from './compactionLog.js'

let root

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'taurhaus-compaction-log-'))
})

afterEach(() => {
  rmSync(root, { recursive: true, force: true })
})

describe('parseLogEvents', () => {
  it('parses one record per line and keeps top-level context fields', () => {
    const raw = [
      '{"event":"compaction.codex_hook.received","tool":"codex","session_id":"s-1"}',
      '{"event":"compaction.codex_hook.delivered","tool":"codex","additional_context_bytes":42}',
    ].join('\n')

    const events = parseLogEvents(raw)

    expect(events).toHaveLength(2)
    expect(events[0].event).toBe('compaction.codex_hook.received')
    expect(events[0].session_id).toBe('s-1')
    expect(events[1].additional_context_bytes).toBe(42)
  })

  it('skips blank and malformed lines instead of throwing', () => {
    const raw = ['', '{"event":"a"}', 'not json at all', '   ', '{"event":"b"}'].join('\n')

    expect(parseLogEvents(raw).map((event) => event.event)).toEqual(['a', 'b'])
  })

  it('returns an empty list for empty input', () => {
    expect(parseLogEvents('')).toEqual([])
    expect(parseLogEvents(undefined)).toEqual([])
  })
})

describe('readLogEventsSince', () => {
  it('reads only the bytes appended after the given offset', () => {
    const logPath = join(root, 'taurhaus.log.jsonl')
    writeFileSync(logPath, '{"event":"before"}\n')

    const first = readLogEventsSince(logPath, 0)
    expect(first.events.map((event) => event.event)).toEqual(['before'])
    expect(first.offset).toBeGreaterThan(0)

    appendFileSync(logPath, '{"event":"after"}\n')

    const second = readLogEventsSince(logPath, first.offset)
    expect(second.events.map((event) => event.event)).toEqual(['after'])
    expect(second.offset).toBeGreaterThan(first.offset)
  })

  it('treats a missing log file as empty and keeps the offset', () => {
    const result = readLogEventsSince(join(root, 'absent.jsonl'), 17)

    expect(result.events).toEqual([])
    expect(result.offset).toBe(17)
  })

  it('restarts from zero when the file was rotated below the offset', () => {
    const logPath = join(root, 'taurhaus.log.jsonl')
    writeFileSync(logPath, '{"event":"rotated"}\n')

    const result = readLogEventsSince(logPath, 10_000)

    expect(result.events.map((event) => event.event)).toEqual(['rotated'])
  })

  it('does not consume a trailing partial line', () => {
    const logPath = join(root, 'taurhaus.log.jsonl')
    writeFileSync(logPath, '{"event":"complete"}\n{"event":"parti')

    const first = readLogEventsSince(logPath, 0)
    expect(first.events.map((event) => event.event)).toEqual(['complete'])

    appendFileSync(logPath, 'al"}\n')
    const second = readLogEventsSince(logPath, first.offset)
    expect(second.events.map((event) => event.event)).toEqual(['partial'])
  })
})

describe('selectEvents', () => {
  const events = [
    { event: 'compaction.codex_hook.received', member_name: 'architect', session_id: 's-1' },
    { event: 'compaction.codex_hook.delivered', member_name: 'architect', session_id: 's-1' },
    { event: 'compaction.codex_hook.delivered', member_name: 'other', session_id: 's-2' },
    { event: 'compaction.signal_emitted', member_name: 'architect' },
  ]

  it('selects by exact event name', () => {
    expect(selectEvents(events, { event: 'compaction.codex_hook.received' })).toHaveLength(1)
  })

  it('selects by event prefix', () => {
    expect(selectEvents(events, { eventPrefix: 'compaction.codex_hook.' })).toHaveLength(3)
  })

  it('narrows by field equality', () => {
    const selected = selectEvents(events, {
      event: 'compaction.codex_hook.delivered',
      match: { member_name: 'architect' },
    })

    expect(selected).toHaveLength(1)
    expect(selected[0].session_id).toBe('s-1')
  })

  it('returns nothing when a matched field is absent', () => {
    expect(selectEvents(events, { event: 'compaction.signal_emitted', match: { session_id: 's-1' } })).toEqual([])
  })
})
