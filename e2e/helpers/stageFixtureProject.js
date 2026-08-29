/**
 * The scratch project a managed stage works in, and the two checks that say
 * whether the stage actually delivered.
 *
 * W4 experiment 3 hands a managed Codex member one bounded implementation task
 * and then has to decide, from evidence rather than from the member's own
 * prose, whether the work exists: a commit in this repo, and a test in it that
 * passes. The repo is therefore created here rather than reused — a throwaway
 * git repo under the wdio session's temp root with a `package.json` and one
 * small Svelte file, so it reads as a project to taurhaus's scanners and to the
 * member, and contains nothing real.
 *
 * It deliberately does *not* contain `src/lib/greet.js` or its test: those are
 * the deliverable, and a file that already exists cannot prove a stage created
 * it.
 */

import { execFileSync, spawnSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

/** The command the stage is asked to validate its own work with. */
export const FIXTURE_TEST_COMMAND = 'bun test'

/**
 * The fixture's own committer identity.
 *
 * The member commits inside this repo; without a local identity the commit
 * would depend on the operator's global git config being set, and would be
 * attributed to them.
 */
const FIXTURE_AUTHOR = { name: 'taurhaus-e2e', email: 'e2e@taurhaus.local' }

function git(repoPath, args) {
  return execFileSync('git', ['-C', repoPath, ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout: 30_000,
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: FIXTURE_AUTHOR.name,
      GIT_AUTHOR_EMAIL: FIXTURE_AUTHOR.email,
      GIT_COMMITTER_NAME: FIXTURE_AUTHOR.name,
      GIT_COMMITTER_EMAIL: FIXTURE_AUTHOR.email,
    },
  })
}

/**
 * Create the throwaway project a stage runs in.
 *
 * Returns its path and the commit the stage starts from, which is what makes
 * "the stage added a commit" checkable rather than assumed.
 */
export function createStageFixtureProject(repoPath) {
  mkdirSync(join(repoPath, 'src', 'lib'), { recursive: true })

  writeFileSync(
    join(repoPath, 'package.json'),
    `${JSON.stringify(
      {
        name: 'taurhaus-stage-fixture',
        private: true,
        type: 'module',
        version: '0.0.0',
        scripts: { test: FIXTURE_TEST_COMMAND },
      },
      null,
      2
    )}\n`
  )
  writeFileSync(
    join(repoPath, '.gitignore'),
    'node_modules/\n'
  )
  writeFileSync(
    join(repoPath, 'README.md'),
    '# Stage fixture\n\nThrowaway repository for the taurhaus managed-stage E2E lane.\nNothing here is real; the whole tree is deleted with the run.\n'
  )
  writeFileSync(
    join(repoPath, 'src', 'lib', 'Greeting.svelte'),
    `<script>\n  let { name = 'world' } = $props()\n</script>\n\n<p>Hello, {name}!</p>\n`
  )

  git(repoPath, ['init', '-q'])
  git(repoPath, ['config', '--local', 'user.name', FIXTURE_AUTHOR.name])
  git(repoPath, ['config', '--local', 'user.email', FIXTURE_AUTHOR.email])
  git(repoPath, ['add', '.'])
  git(repoPath, ['commit', '-q', '-m', 'chore: initialize the managed-stage fixture project'])

  return { path: repoPath, headCommit: git(repoPath, ['rev-parse', 'HEAD']).trim() }
}

/**
 * Whether `revision` names a commit object that exists in this repo.
 *
 * `rev-parse --verify <rev>^{commit}` and not `cat-file -e`: the stage reports
 * a commit id in its result JSON, and a symbolic name like `HEAD` would satisfy
 * a looser check while proving nothing about what it wrote.
 */
export function commitExists(repoPath, revision) {
  const candidate = String(revision ?? '').trim()
  if (!/^[0-9a-f]{7,40}$/i.test(candidate)) return false

  const result = spawnSync('git', ['-C', repoPath, 'rev-parse', '--verify', '--quiet', `${candidate}^{commit}`], {
    encoding: 'utf8',
    timeout: 15_000,
  })
  return result.status === 0
}

/**
 * Run the fixture's own test command and report whether it passed.
 *
 * `bun test` needs no install, which is what keeps the assigned task bounded:
 * the member writes one module and one test and runs them, and nothing in the
 * lane depends on a package registry.
 */
export function runFixtureTests(repoPath) {
  const result = spawnSync('bun', ['test'], {
    cwd: repoPath,
    encoding: 'utf8',
    timeout: 120_000,
  })
  const output = `${result.stdout ?? ''}${result.stderr ?? ''}`.trim()
  return {
    command: FIXTURE_TEST_COMMAND,
    passed: result.status === 0,
    output: output || String(result.error?.message ?? 'no output'),
  }
}
