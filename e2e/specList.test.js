import { describe, it, expect } from 'vitest'
import { basename, resolve } from 'node:path'

import { buildSpecList, listSpecFiles, paidSpecs, specGroups } from './specList.js'

// Vitest runs from the repository root (see CLAUDE.md), which is what makes
// this the real specs directory rather than a fixture.
const specsDir = resolve('e2e/specs')

function flatNames(specList) {
  return specList.flat().map((path) => basename(path))
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

  it('still collects an ordinary ungrouped spec into the catch-all session', () => {
    const files = [...specGroups[0], 'brand-new-spec.js']
    const list = buildSpecList(specsDir, files.sort())
    expect(flatNames(list).at(-1)).toBe('brand-new-spec.js')
    expect(list).toHaveLength(2)
  })

  it('drops a paid lane from the catch-all session it would otherwise form', () => {
    const list = buildSpecList(specsDir, [...specGroups[0], ...paidSpecs].sort())
    expect(list).toHaveLength(1)
    expect(flatNames(list).sort()).toEqual([...specGroups[0]].sort())
  })

  it('resolves grouped specs to absolute paths under the specs directory', () => {
    const [firstGroup] = buildSpecList(specsDir, [...specGroups[0]])
    expect(firstGroup).toEqual(specGroups[0].map((name) => resolve(specsDir, name)))
  })
})
