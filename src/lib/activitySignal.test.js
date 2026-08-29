import { describe, expect, it } from 'vitest'

import {
  ACTIVITY_LEVELS,
  activityLevel,
  activitySignal,
  isActiveLevel,
  isLiveLevel,
  isRetainedSignal,
} from './activitySignal.js'

/**
 * Full input matrix for the single presented-activity derivation.
 *
 * Every row is `[name, record, { level, label, confidence, source }]`.
 * Rows are ordered by the precedence documented in activitySignal.js so a
 * reordering of the derivation shows up as a table diff.
 */
const DERIVATION_TABLE = [
  // --- pane liveness wins over everything (PR 7b evidence) ---
  [
    'foreign pane on an active record is offline, not uncertain',
    { state: 'active', pane_foreign: true, activity_attribution: 'attributed' },
    { level: 'offline', label: 'Offline', confidence: 'high', source: 'pane_foreign' },
  ],
  [
    'foreign pane wins over a degraded snapshot',
    { state: 'active', pane_foreign: true, degraded: true },
    { level: 'offline', label: 'Offline', confidence: 'high', source: 'pane_foreign' },
  ],
  [
    'dead pane on an active record is offline',
    { status: 'active', pane_alive: false },
    { level: 'offline', label: 'Offline', confidence: 'high', source: 'pane_dead' },
  ],
  [
    'live pane does not by itself make a record live',
    { status: 'offline', pane_alive: true },
    { level: 'offline', label: 'Offline', confidence: 'medium', source: 'none' },
  ],

  // --- no reported status ---
  [
    'missing record is offline',
    null,
    { level: 'offline', label: 'Offline', confidence: 'medium', source: 'none' },
  ],
  [
    'empty status is offline',
    {},
    { level: 'offline', label: 'Offline', confidence: 'medium', source: 'none' },
  ],
  [
    'unknown status is offline',
    { status: 'zombie' },
    { level: 'offline', label: 'Offline', confidence: 'medium', source: 'none' },
  ],
  [
    'explicit offline status is offline',
    { sessionStatus: 'offline' },
    { level: 'offline', label: 'Offline', confidence: 'medium', source: 'none' },
  ],

  // --- retained, not observed ---
  [
    'degraded snapshot downgrades an active record to uncertain',
    { state: 'active', activity_attribution: 'attributed', degraded: true },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'degraded' },
  ],
  [
    'degraded snapshot downgrades an idle record to uncertain',
    { state: 'idle', degraded: true },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'degraded' },
  ],
  [
    'degraded does not resurrect an offline record',
    { state: 'offline', degraded: true },
    { level: 'offline', label: 'Offline', confidence: 'medium', source: 'none' },
  ],
  [
    'stale presence downgrades an active record to uncertain',
    { state: 'active', activity_attribution: 'attributed', _presenceStale: true },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'stale' },
  ],
  [
    'stale presence status flag downgrades an idle record to uncertain',
    { state: 'idle', _presenceStatus: 'stale' },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'stale' },
  ],
  [
    'degraded is reported ahead of stale presence',
    { state: 'active', degraded: true, _presenceStale: true },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'degraded' },
  ],

  // --- unattributed activity is uncertain, never working ---
  [
    'active but unattributed is uncertain',
    { state: 'active', activity_attribution: 'unattributed', activity_confidence: 'low' },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'project' },
  ],
  [
    'idle with project-level unattributed activity is uncertain',
    { state: 'idle', project_unattributed_active: true, activity_confidence: 'low' },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'project' },
  ],
  [
    'camelCase unattributed flag is honoured',
    { status: 'idle', projectUnattributedActive: true },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'project' },
  ],

  // --- working: reported active with per-session attribution ---
  [
    'attributed active with high confidence is working',
    { state: 'active', activity_attribution: 'attributed', activity_confidence: 'high' },
    { level: 'working', label: 'Working', confidence: 'high', source: 'session' },
  ],
  [
    'attributed active with medium confidence is working',
    { state: 'active', activity_attribution: 'attributed', activity_confidence: 'medium' },
    { level: 'working', label: 'Working', confidence: 'medium', source: 'session' },
  ],
  [
    'attributed active keeps a low reported confidence',
    { state: 'active', activity_attribution: 'attributed', activity_confidence: 'low' },
    { level: 'working', label: 'Working', confidence: 'low', source: 'session' },
  ],
  [
    'the derived level is idempotent',
    { status: 'working' },
    { level: 'working', label: 'Working', confidence: 'medium', source: 'status' },
  ],

  // --- active: reported live with no per-session evidence (mesh members) ---
  [
    'reported active without attribution metadata is active',
    { sessionStatus: 'active' },
    { level: 'active', label: 'Active', confidence: 'medium', source: 'status' },
  ],
  [
    'attribution none is not per-session evidence',
    { state: 'active', activity_attribution: 'none' },
    { level: 'active', label: 'Active', confidence: 'medium', source: 'status' },
  ],
  [
    'recent output does not raise confidence for a status-only record',
    { sessionStatus: 'active', last_output_age_secs: 3 },
    { level: 'active', label: 'Active', confidence: 'medium', source: 'status' },
  ],
  [
    'old output leaves a status-only record at medium confidence',
    { sessionStatus: 'active', lastOutputAgeSecs: 900 },
    { level: 'active', label: 'Active', confidence: 'medium', source: 'status' },
  ],
  [
    'a reported confidence is used without attribution',
    { sessionStatus: 'active', activity_confidence: 'high' },
    { level: 'active', label: 'Active', confidence: 'high', source: 'status' },
  ],
  [
    'starting is live and reported as active',
    { sessionStatus: 'starting' },
    { level: 'active', label: 'Starting', confidence: 'medium', source: 'status' },
  ],

  // --- idle ---
  [
    'attributed idle is idle',
    { state: 'idle', activity_attribution: 'attributed', activity_confidence: 'medium' },
    { level: 'idle', label: 'Idle', confidence: 'medium', source: 'session' },
  ],
  [
    'reported idle without metadata is idle',
    { sessionStatus: 'idle' },
    { level: 'idle', label: 'Idle', confidence: 'medium', source: 'status' },
  ],
  [
    'idle with attribution none stays idle',
    { state: 'idle', activity_attribution: 'none', activity_confidence: 'low' },
    { level: 'idle', label: 'Idle', confidence: 'medium', source: 'status' },
  ],

  // --- explicit uncertain passes through (idempotency for derived nodes) ---
  [
    'an already-derived uncertain level round-trips',
    { status: 'uncertain' },
    { level: 'uncertain', label: 'Uncertain', confidence: 'low', source: 'status' },
  ],
]

describe('activitySignal derivation table', () => {
  it.each(DERIVATION_TABLE)('%s', (_name, record, expected) => {
    expect(activitySignal(record)).toEqual(expected)
  })

  it('only ever returns a known level', () => {
    for (const [, record] of DERIVATION_TABLE) {
      expect(ACTIVITY_LEVELS).toContain(activitySignal(record).level)
    }
  })
})

describe('activitySignal invariants', () => {
  // Regression: `recent_io` flips on every poll, which is why the daemon
  // leaves it out of `SessionEventSignature` (session_activity.rs, 3f0d541).
  // A presented level that reads it would flicker at the scan cadence.
  it('never lets recent_io change the presented signal', () => {
    const records = [
      { state: 'active', activity_attribution: 'attributed', activity_confidence: 'high' },
      { state: 'idle', activity_attribution: 'attributed' },
      { state: 'idle', project_unattributed_active: true },
      { sessionStatus: 'active' },
      { sessionStatus: 'offline' },
    ]

    for (const record of records) {
      expect(activitySignal({ ...record, recent_io: true }))
        .toEqual(activitySignal({ ...record, recent_io: false }))
    }
  })

  // Regression: 6c6f1cb promoted a status-only record to high confidence when
  // `last_output_age_secs <= 10`. The daemon deliberately keeps output age out
  // of `SessionEventSignature` (session_activity.rs) because it changes on
  // every poll, so the frontend holds the number frozen at the last real event
  // and would report "high confidence" forever once it had been recent. Only
  // change-gated evidence may set the presented confidence.
  it('never lets last_output_age_secs change the presented signal', () => {
    const records = [
      { sessionStatus: 'active' },
      { sessionStatus: 'idle' },
      { state: 'active', activity_attribution: 'attributed', activity_confidence: 'medium' },
      { state: 'idle', project_unattributed_active: true },
    ]

    for (const record of records) {
      expect(activitySignal({ ...record, last_output_age_secs: 1 }))
        .toEqual(activitySignal({ ...record, last_output_age_secs: 3600 }))
    }
  })

  // Regression: a member whose tmux pane was reused by another team's process
  // must read as offline, not as an uncertain live member (aecc8ac, 3e2375a).
  it('reports a reused pane as offline with a foreign source', () => {
    const signal = activitySignal({
      sessionStatus: 'active',
      paneAlive: true,
      paneForeign: true,
    })

    expect(signal.level).toBe('offline')
    expect(signal.source).toBe('pane_foreign')
    expect(isLiveLevel(signal.level)).toBe(false)
  })

  it('re-deriving a derived level is a no-op', () => {
    for (const level of ACTIVITY_LEVELS) {
      expect(activityLevel({ status: level })).toBe(level)
    }
  })

  it('classifies live and active levels', () => {
    expect(ACTIVITY_LEVELS.filter(isLiveLevel)).toEqual([
      'working',
      'active',
      'idle',
      'uncertain',
    ])
    expect(ACTIVITY_LEVELS.filter(isActiveLevel)).toEqual(['working', 'active'])
  })

  it('names both retained sources and nothing else', () => {
    expect(isRetainedSignal(activitySignal({ state: 'active', _presenceStale: true }))).toBe(true)
    expect(isRetainedSignal(activitySignal({ state: 'active', degraded: true }))).toBe(true)
    expect(
      isRetainedSignal(activitySignal({ state: 'idle', project_unattributed_active: true }))
    ).toBe(false)
    expect(isRetainedSignal(activitySignal({ state: 'active' }))).toBe(false)
    expect(isRetainedSignal(undefined)).toBe(false)
  })
})

describe('activitySignal workflow writes', () => {
  function withWorkflow(record, { liveRuns = 1, secondsAgo = 1 } = {}) {
    return {
      ...record,
      workflow_activity: {
        live_runs: liveRuns,
        last_write_at: Date.now() - secondsAgo * 1000,
      },
    }
  }

  it('reads an idle session with a fresh workflow write as working', () => {
    expect(activitySignal(withWorkflow({ state: 'idle' }))).toEqual({
      level: 'working',
      label: 'Working',
      confidence: 'high',
      source: 'workflow',
    })
  })

  it('grades confidence by how recent the last agent write is', () => {
    expect(activitySignal(withWorkflow({ state: 'idle' }, { secondsAgo: 5 })).confidence).toBe('high')
    expect(activitySignal(withWorkflow({ state: 'idle' }, { secondsAgo: 30 })).confidence).toBe('medium')
    expect(activitySignal(withWorkflow({ state: 'idle' }, { secondsAgo: 55 })).confidence).toBe('low')
  })

  it('ignores a write older than the sixty-second window', () => {
    expect(activitySignal(withWorkflow({ state: 'idle' }, { secondsAgo: 61 }))).toEqual({
      level: 'idle',
      label: 'Idle',
      confidence: 'medium',
      source: 'status',
    })
  })

  it('ignores an activity hint that counts no live run', () => {
    expect(activitySignal(withWorkflow({ state: 'idle' }, { liveRuns: 0 })).level).toBe('idle')
  })

  it('accepts the camelCase spelling of the hint', () => {
    const record = {
      state: 'idle',
      workflowActivity: { liveRuns: 2, lastWriteAt: Date.now() - 1000 },
    }
    expect(activitySignal(record).level).toBe('working')
    expect(activitySignal(record).source).toBe('workflow')
  })

  it('leaves every stronger reading unchanged', () => {
    expect(activitySignal(withWorkflow({ state: 'active', pane_foreign: true })).level).toBe('offline')
    expect(activitySignal(withWorkflow({ state: 'offline' })).level).toBe('offline')
    expect(activitySignal(withWorkflow({ state: 'idle', degraded: true })).source).toBe('degraded')
    expect(activitySignal(withWorkflow({ state: 'idle', _presenceStale: true })).source).toBe('stale')
    expect(
      activitySignal(withWorkflow({ state: 'active', project_unattributed_active: true })).source
    ).toBe('project')
  })

  it('names the workflow write as the evidence even for an attributed session', () => {
    const attributed = {
      state: 'active',
      activity_attribution: 'attributed',
      activity_confidence: 'medium',
    }
    expect(activitySignal(withWorkflow(attributed)).source).toBe('workflow')
    expect(activitySignal(attributed).source).toBe('session')
  })

  it('tolerates a malformed hint', () => {
    expect(activitySignal({ state: 'idle', workflow_activity: {} }).level).toBe('idle')
    expect(activitySignal({ state: 'idle', workflow_activity: null }).level).toBe('idle')
    expect(
      activitySignal({ state: 'idle', workflow_activity: { live_runs: 1, last_write_at: 'soon' } }).level
    ).toBe('idle')
  })
})
