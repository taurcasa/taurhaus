import { describe, expect, it, vi } from 'vitest'
import { summarizeSuite, coverageComplete, selectedSpecFiles, finishRun, mochaHooks } from './runSummary.js'
import { resolve } from 'node:path'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'

describe('suite run accounting', () => {
  // Regression: 3329e3e2 counted inherited resume-verification exclusions as
  // unexplained skips; 97000c80 then reached cold-resume on every breadth run.
  it('accounts for the named resume-verification exclusion without inventing execution', () => {
    const file = resolve('e2e/specs/mesh-recovery.js')
    const title = 'Mesh Recovery shows cold-resume controls after a full team stop and reload'
    const suite = { tests: [{ file, pending: true, fullTitle: () => title }], suites: [] }
    const rows = summarizeSuite(suite)
    expect(rows[file]).toMatchObject({ selected: 1, executed: 0, skipped: 1 })
    expect(rows[file].excluded_tests).toEqual([{ test: title, reason: expect.stringMatching(/resume.*verification.*mesh-flake-audit/i) }])
    expect(coverageComplete(rows, 0)).toBe(true)
    expect(coverageComplete(rows, 1)).toBe(false)
    expect(coverageComplete({ 'other.js': rows[file] }, 0)).toBe(false)
    expect(coverageComplete({ [file]: { ...rows[file], unreached: 1 } }, 0)).toBe(false)
    expect(readFileSync(file, 'utf8')).toContain("it.skip('shows cold-resume controls after a full team stop and reload'")
  })
  it('writes the loaded Mocha tree and persists pending cases without inventing passes', () => {
    const dir = mkdtempSync(resolve(tmpdir(), 'taurhaus-summary-test-'))
    const path = resolve(dir, 'run-summary.json')
    const root = { tests: [{ file: resolve('e2e/specs/critical-smoke.js'), title: 'required' }], suites: [] }
    vi.stubEnv('E2E_RUN_SUMMARY', path)
    vi.stubEnv('E2E_SETUP_ERROR', '')
    try {
      writeFileSync(path, JSON.stringify({ specs: {} }))
      mochaHooks.beforeAll.call({ test: { parent: root } })
      expect(JSON.parse(readFileSync(path)).specs['e2e/specs/critical-smoke.js'].unreached).toBe(1)
      root.tests[0].pending = true
      mochaHooks.afterAll()
      expect(JSON.parse(readFileSync(path)).specs['e2e/specs/critical-smoke.js']).toMatchObject({ executed: 0, skipped: 1 })
      vi.stubEnv('E2E_SETUP_ERROR', 'seed failed')
      expect(() => mochaHooks.beforeAll.call({ test: { parent: root } })).toThrow('seed failed')
    } finally {
      vi.unstubAllEnvs()
      rmSync(dir, { recursive: true, force: true })
    }
  })
  it('records whole-run wall time separately from build time and preserves serial headless execution', () => {
    const summary = { started_at: '2026-09-06T12:00:00.000Z', build_ms: 1_500, specs: {} }
    finishRun(summary, 1, Date.parse('2026-09-06T12:00:05.000Z'))
    expect(summary).toMatchObject({ wall_ms: 5_000, build_ms: 1_500, complete: false, exit_code: 1 })
    expect(readFileSync('e2e/wdio.conf.js', 'utf8')).toContain('maxInstances: 1')
    expect(readFileSync('justfile', 'utf8')).toContain('xvfb-run -a')
  })
  it('uses process identity cleanup without a port-pattern kill fallback', () => {
    const source = readFileSync('e2e/wdio.conf.js', 'utf8')
    expect(source).toContain('processLedger?.cleanup()')
    expect(source).not.toContain("spawnSync('pkill'")
  })
  it('uses resolved WDIO selection including explicit spec and exclusions', () => {
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
