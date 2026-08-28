import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mkdtempSync, mkdirSync, rmSync, statSync, writeFileSync, readFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  CODEX_SCRATCH_FILES,
  createCodexScratchHome,
  setAutoCompactTokenLimit,
  trustProject,
} from './codexScratchHome.js'

let root

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'taurhaus-codex-scratch-'))
})

afterEach(() => {
  rmSync(root, { recursive: true, force: true })
})

function realHomeFixture() {
  const home = join(root, 'real-codex')
  mkdirSync(join(home, 'sessions', '2026'), { recursive: true })
  writeFileSync(join(home, 'auth.json'), '{"tokens":{"access_token":"secret"}}')
  writeFileSync(
    join(home, 'config.toml'),
    'model = "gpt-5.4"\nnotify = ["/home/operator/bin/my-notifier"]\n\n[projects."/x"]\ntrust_level = "trusted"\n'
  )
  writeFileSync(join(home, 'history.jsonl'), '{"text":"private"}\n')
  writeFileSync(join(home, 'sessions', '2026', 'rollout-x.jsonl'), '{}\n')
  return home
}

describe('createCodexScratchHome', () => {
  it('copies only the credential file', () => {
    const home = realHomeFixture()
    const scratch = join(root, 'scratch-codex')

    const result = createCodexScratchHome(home, scratch)

    expect(result.copied.sort()).toEqual([...CODEX_SCRATCH_FILES].sort())
    expect(existsSync(join(scratch, 'auth.json'))).toBe(true)
    expect(existsSync(join(scratch, 'config.toml'))).toBe(true)
    expect(existsSync(join(scratch, 'history.jsonl'))).toBe(false)
    expect(existsSync(join(scratch, 'sessions'))).toBe(false)
  })

  // Regression: 7f105bb copied the operator's whole config.toml into the scratch
  // home. A config.toml carrying `notify` makes taurhaus preserve the operator's
  // notifier instead of installing its own, and the lane's only turn signal
  // (`codex-notify.jsonl`) then stays empty for the whole paid run. Anything the
  // operator can make Codex execute — notify, MCP servers — is left behind now.
  it('generates its own config instead of copying the operator integrations', () => {
    const home = realHomeFixture()
    const scratch = join(root, 'scratch-codex')

    const result = createCodexScratchHome(home, scratch)

    expect(result.copied).toEqual(['auth.json'])
    expect(result.generated).toEqual(['config.toml'])
    const config = readFileSync(join(scratch, 'config.toml'), 'utf8')
    const settings = config.split('\n').filter((line) => !line.trimStart().startsWith('#'))
    expect(settings.some((line) => /^\s*notify\s*=/.test(line))).toBe(false)
    expect(config).not.toContain('gpt-5.4')
    expect(config).not.toContain('[projects."/x"]')
  })

  // Regression: 7f105bb copied credentials with copyFileSync alone, which takes the
  // source mode only when the target is new — a scratch home reused across runs kept
  // whatever mode was there. The copy holds a live subscription token.
  it('keeps the copied credentials and the generated config private', () => {
    const home = realHomeFixture()
    const scratch = join(root, 'scratch-codex')

    createCodexScratchHome(home, scratch)

    expect(statSync(join(scratch, 'auth.json')).mode & 0o777).toBe(0o600)
    expect(statSync(join(scratch, 'config.toml')).mode & 0o777).toBe(0o600)
  })

  it('reports a missing credential file instead of throwing', () => {
    const home = join(root, 'empty-codex')
    mkdirSync(home, { recursive: true })

    const result = createCodexScratchHome(home, join(root, 'scratch-codex'))

    expect(result.copied).toEqual([])
    expect(result.missing).toContain('auth.json')
  })

  it('never writes back into the source home', () => {
    const home = realHomeFixture()
    const before = readFileSync(join(home, 'config.toml'), 'utf8')
    const scratch = join(root, 'scratch-codex')

    createCodexScratchHome(home, scratch)
    setAutoCompactTokenLimit(join(scratch, 'config.toml'), 9_000)

    expect(readFileSync(join(home, 'config.toml'), 'utf8')).toBe(before)
  })
})

describe('setAutoCompactTokenLimit', () => {
  it('inserts the key above the first table so it stays top-level', () => {
    const configPath = join(root, 'config.toml')
    writeFileSync(configPath, 'model = "gpt-5.4"\n\n[projects."/x"]\ntrust_level = "trusted"\n')

    setAutoCompactTokenLimit(configPath, 9_000)

    const lines = readFileSync(configPath, 'utf8').split('\n')
    const keyIndex = lines.findIndex((line) => line.startsWith('model_auto_compact_token_limit'))
    const tableIndex = lines.findIndex((line) => line.startsWith('['))
    expect(keyIndex).toBeGreaterThanOrEqual(0)
    expect(keyIndex).toBeLessThan(tableIndex)
    expect(lines[keyIndex]).toBe('model_auto_compact_token_limit = 9000')
  })

  it('replaces an existing top-level value rather than adding a duplicate', () => {
    const configPath = join(root, 'config.toml')
    writeFileSync(configPath, 'model_auto_compact_token_limit = 400000\nmodel = "gpt-5.4"\n')

    setAutoCompactTokenLimit(configPath, 9_000)

    const contents = readFileSync(configPath, 'utf8')
    expect(contents.match(/model_auto_compact_token_limit/g)).toHaveLength(1)
    expect(contents).toContain('model_auto_compact_token_limit = 9000')
  })

  it('writes the key into a config that has no keys yet', () => {
    const configPath = join(root, 'config.toml')
    writeFileSync(configPath, '')

    setAutoCompactTokenLimit(configPath, 9_000)

    expect(readFileSync(configPath, 'utf8').trim()).toBe('model_auto_compact_token_limit = 9000')
  })
})

describe('trustProject', () => {
  it('adds a trusted project table so Codex does not stop at its trust prompt', () => {
    const configPath = join(root, 'config.toml')
    writeFileSync(configPath, 'model = "gpt-5.4"\n')

    trustProject(configPath, '/tmp/taurhaus-e2e-abc/projects/taurhaus')

    const contents = readFileSync(configPath, 'utf8')
    expect(contents).toContain('[projects."/tmp/taurhaus-e2e-abc/projects/taurhaus"]')
    expect(contents).toContain('trust_level = "trusted"')
    expect(contents.startsWith('model = "gpt-5.4"')).toBe(true)
  })

  it('is idempotent for a path that is already trusted', () => {
    const configPath = join(root, 'config.toml')
    writeFileSync(configPath, '[projects."/work/repo"]\ntrust_level = "trusted"\n')

    trustProject(configPath, '/work/repo')

    const contents = readFileSync(configPath, 'utf8')
    expect(contents.match(/\[projects\."\/work\/repo"\]/g)).toHaveLength(1)
  })

  it('writes a config that does not exist yet', () => {
    const configPath = join(root, 'fresh.toml')

    trustProject(configPath, '/work/repo')

    expect(readFileSync(configPath, 'utf8')).toContain('[projects."/work/repo"]')
  })
})
