import { describe, expect, it } from 'vitest'
import { summarizeSuite, coverageComplete, selectedSpecFiles } from './runSummary.js'
import { resolve } from 'node:path'
import { readFileSync } from 'node:fs'

describe('suite run accounting', () => {
  it('uses process identity cleanup without a port-pattern kill fallback', () => {
    const source = readFileSync('e2e/wdio.conf.js', 'utf8')
    expect(source).toContain('processLedger?.cleanup()')
    expect(source).not.toContain("spawnSync('pkill'")
  })
  it('uses WDIO selection including explicit spec and exclusions', () => {
    const a = resolve('e2e/specs/first-run-wizard.js')
    const b = resolve('e2e/specs/settings-persistence.js')
    expect(selectedSpecFiles({ specs: [[a], [b]], exclude: [b] })).toEqual([a])
    expect(selectedSpecFiles({ specs: [a], spec: [a] })).toEqual([a])
  })

  it('distinguishes passed, failed, pending, and unreached tests per file', () => {
    const suite = { tests: [], suites: [
      { tests: [
        { file: 'a.js', state: 'passed', title: 'pass' },
        { file: 'a.js', state: 'failed', title: 'fail' },
        { file: 'a.js', pending: true, title: 'missing prerequisite' },
        { file: 'a.js', title: 'unreached after bail' },
      ], suites: [] },
      { tests: [{ file: 'b.js', state: 'passed', title: 'pass' }], suites: [] },
    ] }
    const result = summarizeSuite(suite)
    expect(result['a.js']).toMatchObject({ selected: 4, executed: 2, passed: 1, failed: 1, skipped: 1, unreached: 1 })
    expect(result['b.js']).toMatchObject({ selected: 1, executed: 1, passed: 1, failed: 0, skipped: 0, unreached: 0 })
    expect(result['a.js'].skipped_tests).toEqual(['missing prerequisite'])
  })

  it('rejects an absent spec, empty spec, pending test, or truncated run even with runner exit 0', () => {
    const pass = { selected: 1, executed: 1, passed: 1, failed: 0, skipped: 0, unreached: 0 }
    expect(coverageComplete({ 'a.js': pass }, 0)).toBe(true)
    for (const bad of [null, { ...pass, selected: 0 }, { ...pass, skipped: 1 }, { ...pass, unreached: 1 }]) {
      expect(coverageComplete({ 'a.js': pass, 'b.js': bad }, 0)).toBe(false)
    }
    expect(coverageComplete({ 'a.js': pass }, 1)).toBe(false)
    expect(coverageComplete({}, 0)).toBe(false)
  })
})
