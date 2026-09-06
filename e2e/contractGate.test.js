import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

describe('executed contract gate', () => {
  it('disables both bail flags for breadth while keeping named runs fail-fast', () => {
    for (const recipe of ['test-e2e', 'test-e2e-full']) {
      const result = spawnSync('just', ['--dry-run', recipe], { encoding: 'utf8' })
      expect(result.status, result.stderr).toBe(0)
      expect(result.stderr).toContain('E2E_BAIL=0 E2E_MOCHA_BAIL=0')
    }
    const focused = spawnSync('just', ['--dry-run', 'test-e2e-spec', 'critical-smoke'], { encoding: 'utf8' })
    expect(focused.stderr).not.toContain('E2E_BAIL=0')
  })
  it('selects all three contract binaries for execution, without the heavy lane', () => {
    const result = spawnSync('just', ['--dry-run', 'test-contracts'], { encoding: 'utf8' })
    expect(result.status, result.stderr).toBe(0)
    expect(result.stderr).toContain('cargo test --test cli_renderers --test module_boundary_assertions --test harness_conformance')
    expect(result.stderr).not.toMatch(/--no-run|cargo check|--lib/)
  })

  it('includes joiner integrity in the real lead gate, without recursive seeded checks', () => {
    const source = readFileSync('justfile', 'utf8')
    const frontend = source.slice(source.indexOf('    run_frontend_lane()'), source.indexOf('    wait_for_seed_peer()'))
    expect(frontend).toContain('just lint-just-gates')
  })
})
