import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { countCompactionBoundaries, pathsContainingMarker, rolloutPaths } from './codexRollout.js'

let home

beforeEach(() => {
  home = mkdtempSync(join(tmpdir(), 'taurhaus-codex-rollout-'))
})

afterEach(() => {
  rmSync(home, { recursive: true, force: true })
})

function writeRollout(name, lines) {
  const dir = join(home, 'sessions', '2026', '08', '28')
  mkdirSync(dir, { recursive: true })
  const path = join(dir, name)
  writeFileSync(path, `${lines.join('\n')}\n`)
  return path
}

describe('rolloutPaths', () => {
  it('finds every rollout transcript under the home, at any depth', () => {
    const first = writeRollout('rollout-2026-08-28T10-00-00-aaa.jsonl', ['{}'])
    const second = writeRollout('rollout-2026-08-28T11-00-00-bbb.jsonl', ['{}'])
    writeFileSync(join(home, 'sessions', 'notes.txt'), 'not a rollout')

    expect(rolloutPaths(home).sort()).toEqual([first, second].sort())
  })

  it('is empty for a scratch home Codex has not written to yet', () => {
    expect(rolloutPaths(home)).toEqual([])
  })
})

describe('countCompactionBoundaries', () => {
  it('counts boundaries across every rollout, not just the newest', () => {
    const first = writeRollout('rollout-a.jsonl', ['{"type":"message"}', '{"type":"compacted"}'])
    const second = writeRollout('rollout-b.jsonl', ['{"type":"compacted"}', '{"type" : "compacted"}'])

    expect(countCompactionBoundaries([first, second])).toBe(3)
  })

  it('ignores a path it cannot read rather than throwing mid-poll', () => {
    const present = writeRollout('rollout-a.jsonl', ['{"type":"compacted"}'])

    expect(countCompactionBoundaries([present, join(home, 'gone.jsonl')])).toBe(1)
  })
})

describe('pathsContainingMarker', () => {
  // Regression: 3b56a3f accepted the live lane on taurhaus's own log records
  // alone. `compaction.codex_hook.delivered` is emitted before the response is
  // serialized and written to stdout (compact_hook.rs), so a broken or ignored
  // response would leave the lane green while the member gets no context back.
  // The card carries a marker that exists nowhere but the operational snapshot,
  // and Codex's own transcript is where consumption can be observed.
  it('names the transcripts that carry the restored-context marker', () => {
    const withMarker = writeRollout('rollout-a.jsonl', [
      '{"type":"message","text":"Current task: #taurhaus-e2e-restored-context-42 — resume"}',
    ])
    const without = writeRollout('rollout-b.jsonl', ['{"type":"message","text":"unrelated"}'])

    expect(pathsContainingMarker([withMarker, without], 'taurhaus-e2e-restored-context-42')).toEqual([withMarker])
  })

  it('ignores a path it cannot read rather than throwing mid-poll', () => {
    const present = writeRollout('rollout-a.jsonl', ['{"text":"marker-1"}'])

    expect(pathsContainingMarker([present, join(home, 'gone.jsonl')], 'marker-1')).toEqual([present])
  })
})
