import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { basename, resolve } from 'node:path'

import { buildSpecList, listSpecFiles, paidSpecs, specGroups } from './specList.js'

// Vitest runs from the repository root (see CLAUDE.md), which is what makes
// this the real specs directory rather than a fixture.
const specsDir = resolve('e2e/specs')
const managedDeadlineSource = readFileSync(resolve(specsDir, 'managed-stage-deadline.js'), 'utf8')

function flatNames(specList) {
  return specList.flat().map((path) => basename(path))
}

function groupedNames() {
  return Object.values(specGroups).flat()
}

describe('default WDIO spec list', () => {
  // Regression: 3b56a3f ("test(e2e): add the live Codex compaction lane driven
  // through the hook bridge") added a spec that spends real Codex and Claude
  // subscription turns without adding it to a group. Ungrouped specs are
  // collected into a catch-all session, so the lane entered the default list
  // and `bunx wdio run e2e/wdio.conf.js` — the documented way to run the suite
  // without a rebuild — started paying for it unannounced. The `just` recipes
  // excluded it by name; nothing else did.
  it('leaves paid lanes out of the list a bare wdio run executes', () => {
    const names = flatNames(buildSpecList(specsDir))
    expect(paidSpecs.length).toBeGreaterThan(0)
    for (const paid of paidSpecs) {
      expect(names).not.toContain(paid)
    }
  })

  it('names paid lanes that actually exist, so the exclusion cannot go stale', () => {
    const present = listSpecFiles(specsDir)
    for (const paid of paidSpecs) {
      expect(present).toContain(paid)
    }
  })

  it('keeps the paid managed deadline measurement named-only', () => {
    expect(paidSpecs).toContain('managed-stage-deadline.js')
  })

  // Regression: e1c38eef registered the suppression case before the primary
  // stall, so attempt 1 assigned the stall while the member's prior completion
  // turn was still closing and the delivered notice was never acted on.
  it('runs the primary stall before the suppression case', () => {
    const stall = managedDeadlineSource.indexOf(
      "it('nudges once, stales once, returns timeout, and preserves the managed session'"
    )
    const suppression = managedDeadlineSource.indexOf(
      "it('suppresses the half-time nudge while the member is actively working, then completes normally'"
    )

    expect(stall).toBeGreaterThanOrEqual(0)
    expect(suppression).toBeGreaterThan(stall)
  })

  // Regression: e1c38eef let the follow-up assignment race a live completion
  // turn. Mesh recorded the notice as delivered, but the member never acted on it.
  it('settles the prior final turn before assigning the follow-up suppression task', () => {
    const suppressionBody = managedDeadlineSource.slice(
      managedDeadlineSource.indexOf('async function runActivitySuppressionCase()'),
      managedDeadlineSource.indexOf('async function runDeadlineStallCase()')
    )
    const settle = suppressionBody.indexOf('await settlePreviousCaseBeforeAssignment()')
    const assignment = suppressionBody.indexOf('const assigned = assignDeadlineTask({')
    const delivery = suppressionBody.indexOf(
      'const attention = await waitForAttentionDelivery(assigned.taskId)'
    )

    expect(settle).toBeGreaterThanOrEqual(0)
    expect(assignment).toBeGreaterThan(settle)
    expect(delivery).toBeGreaterThan(assignment)
    expect(managedDeadlineSource).toContain(
      'waitForTurnAfter(previousCaseFinalTurnBaseline, ONBOARDING_TURN_TIMEOUT_MS)'
    )
    expect(managedDeadlineSource).toContain('browser.pause(FOLLOWUP_ASSIGNMENT_SETTLE_MS)')
    expect(managedDeadlineSource).toMatch(
      /notice delivered into a mid-turn pane can be[\s\S]*swallowed member-side[\s\S]*mesh records it as delivered/i
    )
  })

  it('attaches record diagnostics to an in-progress assignment timeout', () => {
    expect(managedDeadlineSource).toContain('assignmentStartTimeoutProblem({')
    expect(managedDeadlineSource).toContain('turnCountAtAssignment: assignmentTurnCount')
    expect(managedDeadlineSource).toContain('turnCountNow: completedTurns()')
    expect(managedDeadlineSource).toContain('runtime: readRuntimeRecord()')
  })

  // Regression: 13f61dbe added per-case retries, but attempt 1 proved they do
  // not arm under this WDIO lane's mochaOpts.
  it('states the paid retry policy without claiming an unarmed automatic retry', () => {
    expect(managedDeadlineSource).not.toMatch(/\bthis\.retries\(/)
    expect(managedDeadlineSource).not.toContain('currentRetry()')
    expect(managedDeadlineSource).toMatch(/failed paid attempts are re-run manually/i)
  })

  it('documents the measured order and preserves the attempt-1 result block shape', () => {
    const header = managedDeadlineSource.slice(0, managedDeadlineSource.indexOf("import { execFileSync"))
    expect(header).toMatch(/stall runs first, straight after onboarding/i)
    expect(header).toMatch(/fresh member with one assignment/i)
    expect(header).toMatch(/after that stall[\s\S]*three-minute task/i)
    expect(header).toMatch(/failed paid attempts are re-run manually/i)
    expect(header).not.toMatch(/before that stall/i)
    expect(header).not.toMatch(/retries the stall/i)

    expect(managedDeadlineSource).toContain(
      "if (Object.keys(measured).length > 0) {\n" +
        '      console.log(`[e2e] managed deadline measured: ${JSON.stringify(measured, null, 2)}`)\n' +
        '    }'
    )
  })

  // Regression: commit 111c776c appended every ungrouped spec to a catch-all,
  // so a new stateful or paid lane could silently enter the default suite.
  it('rejects an ungrouped spec with instructions for sealing the manifest', () => {
    const files = [...specGroups.content, 'brand-new-spec.js']

    expect(() => buildSpecList(specsDir, files.sort())).toThrow(
      /brand-new-spec\.js.*add.*group.*paidSpecs/is
    )
  })

  it('keeps an explicitly paid lane outside the sealed default list', () => {
    const list = buildSpecList(specsDir, [...specGroups.content, ...paidSpecs].sort())
    expect(list).toHaveLength(1)
    expect(flatNames(list).sort()).toEqual([...specGroups.content].sort())
  })

  it('resolves grouped specs to absolute paths under the specs directory', () => {
    const [firstGroup] = buildSpecList(specsDir, [...specGroups.content])
    expect(firstGroup).toEqual(specGroups.content.map((name) => resolve(specsDir, name)))
  })

  it('makes the default list exactly the declared non-paid group union', () => {
    expect(flatNames(buildSpecList(specsDir)).sort()).toEqual(groupedNames().sort())
  })

  it('deliberately groups stateful additions by ui, templates, mesh, or tmux need', () => {
    expect(Object.keys(specGroups)).toEqual(
      expect.arrayContaining(['ui', 'templates', 'mesh', 'tmux'])
    )
  })
})
