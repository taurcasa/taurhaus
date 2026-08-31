import { describe, it, expect } from 'vitest'
import { basename, resolve } from 'node:path'

import { buildSpecList, listSpecFiles, paidSpecs, specGroups } from './specList.js'

// Vitest runs from the repository root (see CLAUDE.md), which is what makes
// this the real specs directory rather than a fixture.
const specsDir = resolve('e2e/specs')

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
