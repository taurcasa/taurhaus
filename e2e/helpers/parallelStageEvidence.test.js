import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import {
  completedParallelRunSummary,
  managedStageVocabulary,
  stageWindowOverlap,
} from './parallelStageEvidence.js'

describe('stageWindowOverlap', () => {
  it('returns the real intersection of two delivered-to-RESULT windows', () => {
    expect(
      stageWindowOverlap(
        { deliveredAt: '2026-08-31T10:00:00.000Z', resultAt: '2026-08-31T10:00:40.000Z' },
        { deliveredAt: '2026-08-31T10:00:03.000Z', resultAt: '2026-08-31T10:00:30.000Z' }
      )
    ).toEqual({
      startAt: '2026-08-31T10:00:03.000Z',
      endAt: '2026-08-31T10:00:30.000Z',
      durationMs: 27_000,
    })
  })

  it('rejects serialized, touching, or malformed windows', () => {
    expect(
      stageWindowOverlap(
        { deliveredAt: '2026-08-31T10:00:00.000Z', resultAt: '2026-08-31T10:00:10.000Z' },
        { deliveredAt: '2026-08-31T10:00:10.000Z', resultAt: '2026-08-31T10:00:20.000Z' }
      )
    ).toBeNull()
    expect(stageWindowOverlap({ deliveredAt: 'bad', resultAt: 'also bad' }, {})).toBeNull()
  })

  // Regression: b23cbbdb started both windows at assignment time, so two
  // simultaneously assigned but strictly serialized member turns overlapped
  // by construction and were misreported as concurrent.
  it('rejects serialized work even when both assignments happened first', () => {
    expect(
      stageWindowOverlap(
        {
          assignedAt: '2026-08-31T10:00:00.000Z',
          deliveredAt: '2026-08-31T10:00:01.000Z',
          resultAt: '2026-08-31T10:00:10.000Z',
        },
        {
          assignedAt: '2026-08-31T10:00:00.010Z',
          deliveredAt: '2026-08-31T10:00:10.000Z',
          resultAt: '2026-08-31T10:00:20.000Z',
        }
      )
    ).toBeNull()
  })
})

describe('completedParallelRunSummary', () => {
  const input = {
    runId: 'parallel-run-1',
    workflowName: 'feature-pr-parallel-isolation',
    startedAt: '2026-08-31T10:00:00.000Z',
    finishedAt: '2026-08-31T10:00:40.000Z',
    stages: [
      { key: 'alpha', taskId: '1', resultAt: '2026-08-31T10:00:35.000Z' },
      { key: 'beta', taskId: '2', resultAt: '2026-08-31T10:00:40.000Z' },
    ],
  }

  // Regression: b23cbbdb duplicated `stage:codex:*` and `Managed stage`
  // instead of checking the workflow emitter that owns those strings.
  it('captures managed-stage vocabulary from the production workflow emitter', () => {
    const source = readFileSync(
      resolve(import.meta.dirname, '..', '..', '.claude', 'workflows', 'feature-pr.js'),
      'utf8'
    )
    expect(managedStageVocabulary(source, 'codex')).toEqual({
      labelPrefix: 'stage:codex:',
      phaseTitle: 'Managed stage',
    })
  })

  // Regression: b23cbbdb hardcoded the same phase and label vocabulary it
  // asserted, so the synthesized scanner fixture could not detect workflow
  // vocabulary drift and presented its own output as lead-run evidence.
  it('requires externally captured production vocabulary and labels its evidence source', () => {
    expect(() => completedParallelRunSummary(input)).toThrow(/vocabulary/)

    const summary = completedParallelRunSummary({
      ...input,
      vocabulary: { labelPrefix: 'stage:codex:', phaseTitle: 'Managed stage' },
    })
    expect(summary.result.evidenceSource).toBe('synthesized-scanner-contract')
  })

  it('records both managed couriers under the W2 run-tree phase', () => {
    const summary = completedParallelRunSummary({
      ...input,
      vocabulary: { labelPrefix: 'stage:codex:', phaseTitle: 'Managed stage' },
    })

    expect(summary.phases).toEqual([{ title: 'Managed stage' }])
    expect(summary.workflowProgress.map((agent) => agent.label)).toEqual([
      'stage:codex:alpha',
      'stage:codex:beta',
    ])
    expect(summary.workflowProgress.every((agent) => agent.phaseTitle === 'Managed stage')).toBe(true)
    expect(summary.agentCount).toBe(2)
    expect(summary.durationMs).toBe(40_000)
  })
})
