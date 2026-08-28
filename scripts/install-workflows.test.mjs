// @vitest-environment node
// The installer only ever writes under the directory it is given, and every test here points it at a
// tempdir — nothing in this file may touch a real ~/.claude, ~/.codex, ~/.gemini or ~/.grok.
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { resolveAccountDir, ownershipProblem, installWorkflows, WORKFLOWS_SUBDIR } from './install-workflows.mjs'
import { checkWorkflowSource } from './check-workflow-scripts.mjs'

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const SOURCE = path.join(REPO, '.claude/workflows')

let home
beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'workflow-install-'))
})
afterEach(() => {
  fs.rmSync(home, { recursive: true, force: true })
})

const modeOf = (file) => fs.statSync(file).mode & 0o777
const install = (over = {}) => installWorkflows({ source: SOURCE, accountDir: home, ...over })

describe('install-workflows — where it installs', () => {
  it('prefers --account-dir, then CLAUDE_CONFIG_DIR, then ~/.claude', () => {
    expect(resolveAccountDir({ argv: ['--account-dir', '/tmp/pinned'], env: { CLAUDE_CONFIG_DIR: '/tmp/env' }, home: '/tmp/home' })).toBe('/tmp/pinned')
    expect(resolveAccountDir({ argv: [], env: { CLAUDE_CONFIG_DIR: '/tmp/env' }, home: '/tmp/home' })).toBe('/tmp/env')
    expect(resolveAccountDir({ argv: [], env: {}, home: '/tmp/home' })).toBe('/tmp/home/.claude')
  })

  it('installs into <account dir>/workflows', () => {
    const { target } = install()
    expect(target).toBe(path.join(home, WORKFLOWS_SUBDIR))
  })
})

describe('install-workflows — installing', () => {
  it('copies every procedure and the README, and they still pass the script check', () => {
    const { actions, target } = install()
    const installed = actions.filter((a) => a.action === 'installed').map((a) => a.file)
    expect(installed).toContain('feature-pr.js')
    expect(installed).toContain('README.md')
    expect(installed.length).toBe(fs.readdirSync(SOURCE).length)
    for (const file of fs.readdirSync(target).filter((f) => f.endsWith('.js'))) {
      expect(checkWorkflowSource(file, fs.readFileSync(path.join(target, file), 'utf8'))).toEqual([])
    }
  })

  it('creates the workflows directory private to the account', () => {
    const { target } = install()
    expect(modeOf(target)).toBe(0o700)
  })

  it('is idempotent — a second run rewrites nothing', () => {
    install()
    const before = fs.statSync(path.join(home, WORKFLOWS_SUBDIR, 'feature-pr.js'))
    const { actions } = install()
    expect(actions.every((a) => a.action === 'unchanged')).toBe(true)
    const after = fs.statSync(path.join(home, WORKFLOWS_SUBDIR, 'feature-pr.js'))
    expect(after.mtimeMs).toBe(before.mtimeMs)
    expect(after.ino).toBe(before.ino)
  })

  it('updates a drifted file and keeps the permissions it already had', () => {
    const target = path.join(home, WORKFLOWS_SUBDIR)
    fs.mkdirSync(target, { recursive: true, mode: 0o700 })
    const file = path.join(target, 'feature-pr.js')
    fs.writeFileSync(file, '// an older copy\n')
    fs.chmodSync(file, 0o640)
    const { actions } = install()
    expect(actions.find((a) => a.file === 'feature-pr.js').action).toBe('updated')
    expect(fs.readFileSync(file, 'utf8')).toBe(fs.readFileSync(path.join(SOURCE, 'feature-pr.js'), 'utf8'))
    expect(modeOf(file)).toBe(0o640)
  })

  it('writes through a symlinked destination instead of replacing the link', () => {
    const target = path.join(home, WORKFLOWS_SUBDIR)
    const real = path.join(home, 'elsewhere')
    fs.mkdirSync(target, { recursive: true, mode: 0o700 })
    fs.mkdirSync(real, { recursive: true })
    const realFile = path.join(real, 'feature-pr.js')
    fs.writeFileSync(realFile, '// an older copy\n')
    fs.symlinkSync(realFile, path.join(target, 'feature-pr.js'))
    install()
    expect(fs.lstatSync(path.join(target, 'feature-pr.js')).isSymbolicLink()).toBe(true)
    expect(fs.readFileSync(realFile, 'utf8')).toBe(fs.readFileSync(path.join(SOURCE, 'feature-pr.js'), 'utf8'))
  })

  it('swaps a drifted file in by rename, so a killed run never leaves a partial one', () => {
    install()
    const file = path.join(home, WORKFLOWS_SUBDIR, 'feature-pr.js')
    const before = fs.statSync(file).ino
    fs.writeFileSync(file, '// drifted\n')
    expect(fs.statSync(file).ino).toBe(before)
    install()
    expect(fs.statSync(file).ino).not.toBe(before)
  })

  it('leaves no temporary files behind', () => {
    install()
    install()
    expect(fs.readdirSync(path.join(home, WORKFLOWS_SUBDIR)).filter((f) => f.includes('.tmp.'))).toEqual([])
  })

  it('leaves files it does not manage alone', () => {
    const target = path.join(home, WORKFLOWS_SUBDIR)
    fs.mkdirSync(target, { recursive: true, mode: 0o700 })
    fs.writeFileSync(path.join(target, 'my-own.js'), '// mine\n')
    install()
    expect(fs.readFileSync(path.join(target, 'my-own.js'), 'utf8')).toBe('// mine\n')
  })

  it('reports what it would do without writing anything on --dry-run', () => {
    const { actions, target } = install({ dryRun: true })
    expect(actions.every((a) => a.action === 'installed')).toBe(true)
    expect(fs.existsSync(target)).toBe(false)
  })
})

describe('install-workflows — ownership', () => {
  it('refuses to write a path owned by another user', () => {
    expect(ownershipProblem({ uid: 4242 }, 1000)).toMatch(/owned by uid 4242/)
    expect(ownershipProblem({ uid: 1000 }, 1000)).toBe('')
    // On a platform without uids (Windows) there is nothing to prove and nothing to refuse.
    expect(ownershipProblem({ uid: 4242 }, null)).toBe('')
  })

  it('fails loudly rather than overwriting a foreign directory', () => {
    const target = path.join(home, WORKFLOWS_SUBDIR)
    fs.mkdirSync(target, { recursive: true, mode: 0o700 })
    expect(() => installWorkflows({ source: SOURCE, accountDir: home, uid: 4242 })).toThrow(/owned by uid/)
  })
})

describe('install-workflows — uninstalling', () => {
  it('removes the copies it installed and keeps one the user edited', () => {
    install()
    const target = path.join(home, WORKFLOWS_SUBDIR)
    fs.writeFileSync(path.join(target, 'small-change.js'), '// locally edited\n')
    const { actions } = install({ uninstall: true })
    expect(actions.find((a) => a.file === 'feature-pr.js').action).toBe('removed')
    expect(actions.find((a) => a.file === 'small-change.js').action).toBe('kept-modified')
    expect(fs.existsSync(path.join(target, 'feature-pr.js'))).toBe(false)
    expect(fs.readFileSync(path.join(target, 'small-change.js'), 'utf8')).toBe('// locally edited\n')
  })

  it('is idempotent when nothing is installed', () => {
    const { actions } = install({ uninstall: true })
    expect(actions.every((a) => a.action === 'absent')).toBe(true)
  })
})
