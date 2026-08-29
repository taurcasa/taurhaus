import { describe, expect, it, beforeAll, afterAll } from 'vitest'
import { execFileSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { commitExists, createStageFixtureProject, runFixtureTests } from './stageFixtureProject.js'

let root

beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), 'taurhaus-stage-fixture-'))
})

afterAll(() => {
  rmSync(root, { recursive: true, force: true })
})

describe('createStageFixtureProject', () => {
  it('creates a committed git repo with a package.json and one Svelte file', () => {
    const repo = join(root, 'stage-a')
    const created = createStageFixtureProject(repo)

    expect(created.path).toBe(repo)
    expect(existsSync(join(repo, '.git'))).toBe(true)
    expect(JSON.parse(readFileSync(join(repo, 'package.json'), 'utf8')).private).toBe(true)
    expect(readFileSync(join(repo, 'src/lib/Greeting.svelte'), 'utf8')).toContain('$props()')
    expect(created.headCommit).toMatch(/^[0-9a-f]{40}$/)
  })

  it('carries its own committer identity so the fixture never depends on host git config', () => {
    const repo = join(root, 'stage-b')
    createStageFixtureProject(repo)
    const email = execFileSync('git', ['-C', repo, 'config', '--local', 'user.email'], { encoding: 'utf8' })
    expect(email.trim()).toBe('e2e@taurhaus.local')
  })

  it('leaves nothing for the stage to collide with', () => {
    const repo = join(root, 'stage-c')
    createStageFixtureProject(repo)
    expect(existsSync(join(repo, 'src/lib/greet.js'))).toBe(false)
    expect(existsSync(join(repo, 'src/lib/greet.test.js'))).toBe(false)
  })
})

describe('commitExists', () => {
  it('accepts the head it just created and rejects anything else', () => {
    const repo = join(root, 'stage-d')
    const created = createStageFixtureProject(repo)
    expect(commitExists(repo, created.headCommit)).toBe(true)
    expect(commitExists(repo, created.headCommit.slice(0, 8))).toBe(true)
    expect(commitExists(repo, '0'.repeat(40))).toBe(false)
    expect(commitExists(repo, '')).toBe(false)
    expect(commitExists(repo, 'HEAD')).toBe(false)
  })
})

describe('runFixtureTests', () => {
  it('passes for the deliverable shape the stage is asked for', () => {
    const repo = join(root, 'stage-e')
    createStageFixtureProject(repo)
    writeFileSync(join(repo, 'src/lib/greet.js'), 'export function greet(name) {\n  return `Hello, ${name}!`\n}\n')
    writeFileSync(
      join(repo, 'src/lib/greet.test.js'),
      "import { expect, test } from 'bun:test'\nimport { greet } from './greet.js'\n\ntest('greets', () => {\n  expect(greet('ada')).toBe('Hello, ada!')\n})\n"
    )

    const result = runFixtureTests(repo)
    expect(result.passed).toBe(true)
    expect(result.command).toBe('bun test')
  })

  it('fails, with output, when the deliverable does not hold up', () => {
    const repo = join(root, 'stage-f')
    createStageFixtureProject(repo)
    writeFileSync(join(repo, 'src/lib/greet.js'), 'export function greet() {\n  return "nope"\n}\n')
    writeFileSync(
      join(repo, 'src/lib/greet.test.js'),
      "import { expect, test } from 'bun:test'\nimport { greet } from './greet.js'\n\ntest('greets', () => {\n  expect(greet('ada')).toBe('Hello, ada!')\n})\n"
    )

    const result = runFixtureTests(repo)
    expect(result.passed).toBe(false)
    expect(result.output.length).toBeGreaterThan(0)
  })
})
