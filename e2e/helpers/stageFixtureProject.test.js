import { describe, expect, it, beforeAll, afterAll } from 'vitest'
import { execFileSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  commitExists,
  createStageFixtureProject,
  filesAddedByCommit,
  runFixtureTests,
  runFixtureTestsAtCommit,
} from './stageFixtureProject.js'

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

describe('filesAddedByCommit', () => {
  // Regression: 249227f accepted any commit the stage named, so a member could
  // report the fixture's own baseline commit — which adds no deliverable at all
  // — and still be believed. The commit has to carry the files.
  it('names what the commit added and nothing it left alone', () => {
    const repo = join(root, 'stage-g')
    const created = createStageFixtureProject(repo)
    writeFileSync(join(repo, 'src/lib/greet.js'), 'export function greet(name) {\n  return `Hello, ${name}!`\n}\n')
    execFileSync('git', ['-C', repo, 'add', 'src/lib/greet.js'], { encoding: 'utf8' })
    execFileSync('git', ['-C', repo, 'commit', '-q', '-m', 'feat: greet'], { encoding: 'utf8' })
    const head = execFileSync('git', ['-C', repo, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()

    expect(filesAddedByCommit(repo, head)).toEqual(['src/lib/greet.js'])
    expect(filesAddedByCommit(repo, created.headCommit)).not.toContain('src/lib/greet.js')
  })

  it('is empty for a revision the repo does not have', () => {
    const repo = join(root, 'stage-h')
    createStageFixtureProject(repo)
    expect(filesAddedByCommit(repo, '0'.repeat(40))).toEqual([])
  })
})

describe('runFixtureTestsAtCommit', () => {
  const GREET = 'export function greet(name) {\n  return `Hello, ${name}!`\n}\n'
  const GREET_TEST =
    "import { expect, test } from 'bun:test'\nimport { greet } from './greet.js'\n\ntest('greets', () => {\n  expect(greet('ada')).toBe('Hello, ada!')\n})\n"

  it('passes when the deliverable is in the commit it is given', () => {
    const repo = join(root, 'stage-i')
    createStageFixtureProject(repo)
    writeFileSync(join(repo, 'src/lib/greet.js'), GREET)
    writeFileSync(join(repo, 'src/lib/greet.test.js'), GREET_TEST)
    execFileSync('git', ['-C', repo, 'add', '.'], { encoding: 'utf8' })
    execFileSync('git', ['-C', repo, 'commit', '-q', '-m', 'feat: greet'], { encoding: 'utf8' })
    const head = execFileSync('git', ['-C', repo, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()

    const result = runFixtureTestsAtCommit(repo, head)
    expect(result.passed).toBe(true)
    expect(result.command).toBe('bun test')
  })

  // Regression: 249227f validated the working tree, so uncommitted work passed
  // the acceptance check while the commit the stage reported contained none of
  // it. A clean checkout of the reported commit is what the deliverable claims.
  it('fails on work left uncommitted, where a working-tree run would pass', () => {
    const repo = join(root, 'stage-j')
    const created = createStageFixtureProject(repo)
    writeFileSync(join(repo, 'src/lib/greet.js'), GREET)
    writeFileSync(join(repo, 'src/lib/greet.test.js'), GREET_TEST)

    expect(runFixtureTests(repo).passed).toBe(true)
    expect(runFixtureTestsAtCommit(repo, created.headCommit).passed).toBe(false)
  })

  it('leaves no worktree behind', () => {
    const repo = join(root, 'stage-k')
    const created = createStageFixtureProject(repo)
    runFixtureTestsAtCommit(repo, created.headCommit)
    const listed = execFileSync('git', ['-C', repo, 'worktree', 'list'], { encoding: 'utf8' }).trim().split('\n')
    expect(listed).toHaveLength(1)
  })
})
