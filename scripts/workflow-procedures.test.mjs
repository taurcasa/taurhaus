// @vitest-environment node
// Behavioural tests for the versioned procedures in .claude/workflows: they run each script
// with the Workflow API stubbed, so the control flow (fail-closed review, fail-closed gate,
// the Codex launcher's quoting and flags) is exercised without spawning a single agent.
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { parseBody } from './check-workflow-scripts.mjs'

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const WORKFLOWS = path.join(REPO, '.claude/workflows')
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor
const API_GLOBALS = 'agent, parallel, pipeline, phase, log, workflow, args, budget'

function compile(name) {
  return new AsyncFunction(API_GLOBALS, parseBody(fs.readFileSync(path.join(WORKFLOWS, name), 'utf8')))
}

const OK_WORK = {
  status: 'ok',
  summary: 'did the thing',
  commits: ['abc1234 feat: the thing'],
  files_changed: ['src/thing.js'],
  tests_added: ['src/thing.test.js'],
  red_observed: 'the new test failed before the change',
  gate: 'check-quick green',
  deviations: [],
}
const OK_SWEEP = { status: 'ok', summary: 'swept', commits: ['abc1234 docs: sweep'], files_changed: ['README.md'], table_path: '/tmp/t.md', unresolved: [] }
const OK_REVIEW = { status: 'ok', reviewer: 'codex', verdict: 'approve', findings: [] }
const OK_GATE = {
  status: 'pass',
  commands: [
    { command: 'just check-quick', status: 'pass' },
    { command: 'just lint', status: 'pass' },
  ],
  failures: [],
  diff_stat: '2 files changed',
  commits: ['abc1234'],
}

function kindOf(label) {
  if (label.startsWith('review:') || label.startsWith('verify:')) return 'review'
  if (label.startsWith('gate:')) return 'gate'
  if (label.startsWith('research:')) return 'research'
  if (label.startsWith('sweep:')) return 'sweep'
  return 'work'
}

const DEFAULTS = { work: OK_WORK, sweep: OK_SWEEP, review: OK_REVIEW, gate: OK_GATE, research: { status: 'ok', summary: 's', report_path: '/tmp/r.md', key_facts: [], unverified: [] } }

// Runs a workflow script against stubs; `plan` maps a call kind to a value or a (call, state) => value.
async function run(name, workflowArgs, plan = {}) {
  const calls = []
  const logs = []
  const state = { counts: { work: 0, sweep: 0, review: 0, gate: 0, research: 0 } }
  const agent = async (prompt, opts = {}) => {
    const call = { prompt, opts, label: opts.label || '', phase: opts.phase || '' }
    calls.push(call)
    const kind = kindOf(call.label)
    state.counts[kind] += 1
    const responder = Object.prototype.hasOwnProperty.call(plan, kind) ? plan[kind] : DEFAULTS[kind]
    return typeof responder === 'function' ? responder(call, state) : responder
  }
  const parallel = async (thunks) => {
    const out = []
    for (const thunk of thunks) {
      try {
        out.push(await thunk())
      } catch {
        out.push(null)
      }
    }
    return out
  }
  const pipeline = async () => {
    throw new Error('pipeline() is not used by these scripts')
  }
  const workflowFn = async () => {
    throw new Error('workflow() is not used by these scripts')
  }
  const budget = { total: null, spent: () => 0, remaining: () => Infinity }
  const body = compile(name)
  const result = await body(agent, parallel, pipeline, () => {}, (message) => logs.push(String(message)), workflowFn, workflowArgs, budget)
  return { result, calls, logs, state }
}

const BASE_ARGS = { worktree: '/home/dev/checkout', branch: 'feat/x', spec: '/tmp/spec.md' }
const MUTATING = ['feature-pr.js', 'small-change.js', 'fix-round.js', 'docs-sweep.js']
const AUTHORITY_QUESTION =
  'Does the change re-derive a rule another layer owns (frontend vs backend, app vs daemon), or add a view that bypasses the existing authority? Name the authority and cite the duplicate.'
function argsFor(script, extra = {}) {
  const base = { ...BASE_ARGS, ...extra }
  return script === 'fix-round.js'
    ? { ...base, findings: [{ title: 'open', severity: 'major', file: 'a.js:1', evidence: 'e', fix: 'f' }] }
    : base
}

describe('workflow procedures — the shared lib', () => {
  it('is byte-identical in every script (they cannot import it)', () => {
    const section = (name) => {
      const source = fs.readFileSync(path.join(WORKFLOWS, name), 'utf8')
      const start = source.indexOf('// ── lib:')
      const end = source.indexOf('// ── end lib ──')
      expect(start, `${name} has no lib section`).toBeGreaterThan(-1)
      expect(end, `${name} has no end-of-lib marker`).toBeGreaterThan(start)
      return source.slice(start, end)
    }
    const first = section('feature-pr.js')
    for (const name of ['small-change.js', 'fix-round.js', 'research-sweep.js', 'docs-sweep.js']) {
      expect(section(name), `${name} drifted from feature-pr.js`).toBe(first)
    }
  })

  it('rejects an effort outside the harness vocabulary', async () => {
    await expect(run('feature-pr.js', { ...BASE_ARGS, effort: 'turbo' })).rejects.toThrow(/effort/)
  })

  it('normalizes a Windows checkout path to its WSL form', async () => {
    const { calls } = await run('feature-pr.js', { ...BASE_ARGS, worktree: 'C:\\Users\\Jane Doe\\project', implementer: 'codex' })
    expect(calls[0].prompt).toContain('/mnt/c/Users/Jane Doe/project')
    expect(calls[0].prompt).not.toContain('C:\\Users')
  })

  it('normalizes a \\\\wsl$ UNC checkout path', async () => {
    const { calls } = await run('feature-pr.js', { ...BASE_ARGS, worktree: '\\\\wsl$\\Ubuntu\\home\\dev\\proj', implementer: 'codex' })
    expect(calls[0].prompt).toContain('/home/dev/proj')
  })
})

describe('workflow procedures — the authority question', () => {
  // Regression: merge commit 2bbe0b4 (PR #75; accounts plan row 20b) needed six review rounds
  // because authority duplication and bypasses were found late instead of by every review lens.
  it('puts the authority question in the small-change review lane', async () => {
    const { calls } = await run('small-change.js', BASE_ARGS)
    const review = calls.find((call) => call.label.startsWith('review:'))
    expect(review.prompt).toContain(AUTHORITY_QUESTION)
  })

  it('puts the authority question in the fix-round conformance lane', async () => {
    const { calls } = await run('fix-round.js', argsFor('fix-round.js'))
    const review = calls.find((call) => call.label.startsWith('review:'))
    expect(review.prompt).toContain(AUTHORITY_QUESTION)
  })

  it('puts the authority question in both feature-pr first-round lanes', async () => {
    const { calls } = await run('feature-pr.js', BASE_ARGS)
    const conformance = calls.find((call) => call.label.includes('conformance-r1'))
    const operational = calls.find((call) => call.label.includes('operational-r1'))
    expect(conformance.prompt).toContain(AUTHORITY_QUESTION)
    expect(operational.prompt).toContain(AUTHORITY_QUESTION)
  })

  it('keeps the authority question in the feature-pr round-2 re-review lane', async () => {
    const major = { title: 'duplicates backend policy', severity: 'major', file: 'a.js:2', evidence: 'e', fix: 'f' }
    const { calls } = await run('feature-pr.js', BASE_ARGS, {
      review: (call) => (call.label.includes('conformance-r1') ? { ...OK_REVIEW, verdict: 'fix_required', findings: [major] } : OK_REVIEW),
    })
    const rereview = calls.find((call) => call.label.includes('conformance-r2'))
    expect(rereview.prompt).toContain(AUTHORITY_QUESTION)
  })

  it('keeps the identical question in every lens-bearing script', () => {
    // Every script in the directory, not a hand-written list: a new lens-bearing
    // script (or a new lens in an old one) must carry the question too. A lens
    // is any 'Lens:' prompt outside the shared lib block.
    const knownLenses = { 'small-change.js': 1, 'fix-round.js': 1, 'feature-pr.js': 2 }
    const scripts = fs.readdirSync(WORKFLOWS).filter((file) => file.endsWith('.js'))
    expect(scripts.length).toBeGreaterThanOrEqual(5)
    for (const script of scripts) {
      const source = fs.readFileSync(path.join(WORKFLOWS, script), 'utf8')
      const end = source.indexOf('// ── end lib ──')
      const afterLib = end >= 0 ? source.slice(end) : source
      const lenses = afterLib.split('Lens:').length - 1
      expect(source.split(AUTHORITY_QUESTION).length - 1, script).toBe(lenses)
      if (script in knownLenses) expect(lenses, script).toBe(knownLenses[script])
    }
  })
})

describe('workflow procedures — the Codex lane', () => {
  const spacey = { ...BASE_ARGS, worktree: "/home/dev/Jane's checkout", scratch: '/tmp/scratch dir', implementer: 'codex', effort: 'high', codexModel: 'gpt-5.6-terra' }

  it('passes the requested model and reasoning effort to codex exec', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    expect(calls[0].prompt).toContain("-m 'gpt-5.6-terra'")
    expect(calls[0].prompt).toContain('-c \'model_reasoning_effort="high"\'')
  })

  it('carries the same flags into the resumed turns', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const resume = calls[0].prompt.slice(calls[0].prompt.indexOf('codex exec resume'))
    expect(resume).toContain("-m 'gpt-5.6-terra'")
    expect(resume).toContain('model_reasoning_effort="high"')
  })

  it('quotes every path, so a checkout with a space or an apostrophe survives', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    expect(prompt).toContain("'/home/dev/Jane'\\''s checkout'")
    expect(prompt).toContain("'/tmp/scratch dir/codex-")
    // No bare, unquoted occurrence of a path with a space in a command position.
    expect(prompt).not.toMatch(/-C \/home\/dev\/Jane/)
    expect(prompt).not.toMatch(/mkdir -p \/tmp\/scratch dir/)
    expect(prompt).not.toMatch(/cd \/home\/dev\/Jane's checkout/)
  })

  it('bounds the marker poll with an explicit deadline and failure path', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    expect(prompt).toMatch(/deadline|at most \d+ seconds|Bound the wait/i)
    expect(prompt).toMatch(/pkill|kill the run/i)
  })

  // Regression: the launcher used `pkill -f <runner>`, which kills the runner shell but leaves its
  // `timeout`/codex process group alive — an orphaned agent kept mutating the checkout while the
  // retry started. The runner now records its own pid (it is the process-group leader under setsid)
  // and every give-up path kills that group.
  it('records the detached run\'s pid so the whole process group can be killed', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    expect(prompt).toMatch(/echo \$\$ > "\$PIDFILE"/)
    expect(prompt).toContain("PIDFILE='/tmp/scratch dir/codex-")
    expect(prompt).toMatch(/setsid nohup bash/)
  })

  it('kills the process group rather than the runner shell alone, and never pkill', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    expect(prompt).toMatch(/kill -TERM -"\$PGID"/)
    expect(prompt).toMatch(/kill -KILL -"\$PGID"/)
    expect(prompt).not.toMatch(/pkill/)
  })

  it('kills the run on the deadline, before a retry, and before returning', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    const kills = prompt.match(/kill -TERM -"\$PGID"/g) || []
    expect(kills.length, 'the deadline, the retry and the return path each own a kill').toBeGreaterThanOrEqual(3)
    expect(prompt).toMatch(/deadline/i)
    expect(prompt).toMatch(/still running|outlives|orphan/i)
  })

  it('resumes the session it started instead of whatever ran last in the checkout', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    expect(prompt).toMatch(/codex exec resume <SESSION_ID>/)
    expect(prompt).toMatch(/session id/i)
    expect(prompt).toMatch(/--last[^\n]*(newest|another|someone)/i)
  })

  it('gives concurrent runs their own scratch files when args.stamp is passed', async () => {
    const { calls } = await run('feature-pr.js', { ...spacey, stamp: '20260828T2350' })
    expect(calls[0].prompt).toContain('-20260828T2350.prompt.md')
    const { calls: plain } = await run('feature-pr.js', spacey)
    expect(plain[0].prompt).not.toContain('-20260828T2350')
  })

  it('tells the wrapper to report an unavailable Codex instead of inventing an approval', async () => {
    const { calls } = await run('feature-pr.js', { ...BASE_ARGS, implementer: 'opus' })
    const review = calls.find((c) => c.label.startsWith('review:'))
    expect(review.prompt).toMatch(/status='unavailable'|status: 'unavailable'|status=unavailable/)
    expect(review.prompt).not.toMatch(/verdict 'approve'|return verdict "approve"/)
  })
})

describe('workflow procedures — ownership of what the wrapper launches', () => {
  const spacey = { ...BASE_ARGS, worktree: "/home/dev/Jane's checkout", scratch: '/tmp/scratch dir', implementer: 'codex', effort: 'high', codexModel: 'gpt-5.6-terra' }
  const pidfileOf = (calls) => /^PIDFILE=(.*)$/m.exec(calls[0].prompt)[1]

  // Regression: the scratch names were `codex-<tag>[-<stamp>]`, and `tag` defaults to the branch. Two
  // worktrees of this repo on the same branch — the normal shape of parallel agent work — therefore
  // shared one pidfile in the shared scratch dir: the second run overwrote the first run's pid, and
  // the first run's give-up paths then aimed `kill -TERM -<pgid>` at whatever that pid had become.
  it('derives the ownership file names from the checkout, not the branch alone', async () => {
    const w1 = await run('feature-pr.js', { ...spacey, worktree: '/home/dev/taurhaus-w1' })
    const w2 = await run('feature-pr.js', { ...spacey, worktree: '/home/dev/taurhaus-w2' })
    expect(pidfileOf(w1.calls)).toContain('taurhaus-w1')
    expect(pidfileOf(w2.calls)).toContain('taurhaus-w2')
    expect(pidfileOf(w1.calls), 'two checkouts on one branch must not share a pidfile').not.toBe(pidfileOf(w2.calls))
  })

  it('separates the lanes of one run by tag and two deliberate runs by stamp', async () => {
    const { calls } = await run('feature-pr.js', { ...spacey, worktree: '/home/dev/taurhaus-w1' })
    const stamped = await run('feature-pr.js', { ...spacey, worktree: '/home/dev/taurhaus-w1', stamp: 'r2' })
    expect(pidfileOf(stamped.calls)).toContain('-r2.pid')
    expect(pidfileOf(stamped.calls)).not.toBe(pidfileOf(calls))
    const pids = calls.filter((c) => c.prompt.includes('PIDFILE=')).map((c) => /^PIDFILE=(.*)$/m.exec(c.prompt)[1])
    expect(new Set(pids).size, 'every lane of one run owns its own pidfile').toBe(pids.length)
  })

  it('refuses to launch over a live run that already owns the pidfile', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    expect(prompt, 'the guard reads the owner pid').toMatch(/OWNER=\$\(cat/)
    expect(prompt, 'and proves the pid is still this runner').toMatch(/\/proc\/"\$OWNER"\/cmdline/)
    expect(prompt).toMatch(/do NOT launch/i)
    expect(prompt, 'a busy pidfile is reported, not overwritten').toMatch(/status='unavailable'/)
    expect(prompt.indexOf('BUSY'), 'the guard runs before anything is launched').toBeLessThan(prompt.indexOf('setsid nohup bash'))
    expect(prompt.indexOf('BUSY'), 'and before the launch line clears the old artifacts').toBeLessThan(prompt.indexOf('chmod +x'))
  })

  // Regression: every give-up path killed `-$(cat <pidfile>)` on sight. A pidfile that outlived its
  // run — or that another run had written — pointed the kill at a recycled or foreign pid, and the
  // wrapper took down a process it had never started.
  it('proves the pid is still the runner it started before every kill', async () => {
    const { calls } = await run('feature-pr.js', spacey)
    const prompt = calls[0].prompt
    const runnerStart = prompt.indexOf('#!/usr/bin/env bash')
    const runnerEnd = prompt.indexOf('echo "EXIT=$?" >> "$LOG"', runnerStart)
    // The runner's own watchdog and TERM trap kill $$ — the group it leads — so only the kills the
    // wrapper performs from outside need to prove ownership first.
    const outside = prompt.slice(0, runnerStart) + prompt.slice(runnerEnd)
    const kills = [...outside.matchAll(/kill -TERM -"\$PGID"/g)]
    expect(kills.length, 'the deadline, the retry and the return path each own a kill').toBeGreaterThanOrEqual(3)
    for (const kill of kills) {
      const before = outside.slice(Math.max(0, kill.index - 400), kill.index)
      expect(before, 'a kill with no cmdline check in front of it').toMatch(/\/proc\/"\$PGID"\/cmdline/)
      expect(before, 'the cmdline has to name this run\'s runner').toContain('.run.sh')
    }
  })
})

describe('workflow procedures — the ownership lease', () => {
  const alive = (pid) => {
    try {
      process.kill(pid, 0)
      return true
    } catch (error) {
      return error.code === 'EPERM'
    }
  }
  const until = async (predicate, timeoutMs, message) => {
    const deadline = Date.now() + timeoutMs
    for (;;) {
      if (predicate()) return
      if (Date.now() > deadline) throw new Error(message)
      await new Promise((resolve) => setTimeout(resolve, 200))
    }
  }
  const unquote = (value) => value.replace(/^'|'$/g, '').replace(/'\\''/g, "'")

  // Stages the runner the wrapper is told to write — verbatim, from the prompt the script generates —
  // with the codex line swapped for a dummy that spawns a descendant, so a test proves the whole
  // process GROUP dies rather than one shell. The paths carry a space and an apostrophe, like the WSL
  // checkouts this quoting exists for.
  const stage = async (dummy) => {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), 'wf lease '))
    const worktree = path.join(home, "Jane's checkout")
    const scratch = path.join(home, 'scratch dir')
    fs.mkdirSync(worktree)
    fs.mkdirSync(scratch)
    const { calls } = await run('feature-pr.js', { ...BASE_ARGS, worktree, scratch, implementer: 'codex' })
    const prompt = calls[0].prompt
    const start = prompt.indexOf('#!/usr/bin/env bash')
    const endMarker = 'echo "EXIT=$?" >> "$LOG"'
    expect(start, 'the wrapper prompt carries a runner script').toBeGreaterThan(-1)
    const source = prompt.slice(start, prompt.indexOf(endMarker, start) + endMarker.length)
    const dummyPath = path.join(scratch, 'child.sh')
    fs.writeFileSync(dummyPath, dummy)
    const runnerSource = source.replace(/^timeout .*$/m, `bash '${dummyPath}' >> "$LOG" 2>&1`)
    expect(runnerSource, 'the codex invocation is replaced by the dummy').not.toContain('codex exec')
    const runner = path.join(scratch, 'lease-runner.sh')
    fs.writeFileSync(runner, runnerSource + '\n', { mode: 0o755 })
    const grab = (key) => unquote(new RegExp('^' + key + '=(.*)$', 'm').exec(runnerSource)[1])
    const logFile = grab('LOG')
    return {
      home,
      pidFile: grab('PIDFILE'),
      leaseFile: grab('LEASE'),
      readLog: () => (fs.existsSync(logFile) ? fs.readFileSync(logFile, 'utf8') : ''),
      // Launched exactly as the wrapper is told to launch it.
      launch: () => {
        const launcher = spawn('setsid', ['nohup', 'bash', runner], {
          detached: true,
          stdio: 'ignore',
          env: { ...process.env, TAURHAUS_WORKFLOW_LEASE_TTL: '2', TAURHAUS_WORKFLOW_LEASE_POLL: '1' },
        })
        launcher.unref()
      },
    }
  }
  const cleanup = (staged, pids) => {
    for (const pid of pids) {
      if (pid > 0) {
        try {
          process.kill(-pid, 'SIGKILL')
        } catch {
          /* already gone */
        }
      }
    }
    fs.rmSync(staged.home, { recursive: true, force: true })
  }

  // Regression: cleanup lived only in natural-language instructions for the wrapper agent ("kill the
  // group before you return"). A lane aborted after the launch never ran them, so the detached setsid
  // group kept mutating the checkout until its own 1700-3300s timeout, and the runner's trap only
  // removed the pidfile. The runner now holds an ownership lease the wrapper refreshes while it
  // polls, and kills its own process group when it goes stale — cleanup that does not need its owner.
  it('kills the whole detached group when its owner stops refreshing the lease', async () => {
    const staged = await stage('#!/usr/bin/env bash\nsleep 300 &\necho "CHILD=$!"\nwait\n')
    let pgid = 0
    let childPid = 0
    try {
      staged.launch()
      await until(() => fs.existsSync(staged.pidFile) && /CHILD=\d+/.test(staged.readLog()), 20000, 'the dummy run never started')
      pgid = Number(fs.readFileSync(staged.pidFile, 'utf8').trim())
      childPid = Number(/CHILD=(\d+)/.exec(staged.readLog())[1])
      expect(alive(pgid), 'the runner is up').toBe(true)
      expect(alive(childPid), 'its descendant is up').toBe(true)

      // The owner is gone: from here nothing refreshes the lease.
      await until(() => !alive(pgid) && !alive(childPid), 30000, 'the abandoned run outlived its owner: nothing killed the process group')
      expect(staged.readLog(), 'the lease watchdog is what ended it').toMatch(/EXIT=98/)
    } finally {
      cleanup(staged, [pgid, childPid])
    }
  }, 90000)

  it('leaves a run alone while its owner keeps the lease fresh', async () => {
    const staged = await stage('#!/usr/bin/env bash\nsleep 6 &\necho "CHILD=$!"\nwait\n')
    let pgid = 0
    try {
      staged.launch()
      await until(() => fs.existsSync(staged.pidFile) && /CHILD=\d+/.test(staged.readLog()), 20000, 'the dummy run never started')
      pgid = Number(fs.readFileSync(staged.pidFile, 'utf8').trim())
      // The wrapper's poll loop: `until grep -q "^EXIT=" LOG; do touch LEASE; sleep 20; done`.
      const heartbeat = setInterval(() => {
        try {
          fs.utimesSync(staged.leaseFile, new Date(), new Date())
        } catch {
          /* the run ended */
        }
      }, 300)
      try {
        await until(() => /^EXIT=/m.test(staged.readLog()), 30000, 'the dummy run never finished')
      } finally {
        clearInterval(heartbeat)
      }
      expect(staged.readLog(), 'a lease that stays fresh never trips the watchdog').not.toMatch(/EXIT=98/)
      expect(staged.readLog(), 'the run reached its own exit').toMatch(/EXIT=0/)
    } finally {
      cleanup(staged, [pgid])
    }
  }, 90000)
})

describe('workflow procedures — fail closed', () => {
  for (const script of MUTATING) {
    it(`${script} fails when the reviewer lane is unavailable`, async () => {
      await expect(
        run(script, argsFor(script), { review: { status: 'unavailable', error: 'codex exited 124', findings: [], verdict: 'approve' } })
      ).rejects.toThrow(/unavailable/i)
    })

    it(`${script} fails when the reviewer agent returns nothing`, async () => {
      await expect(run(script, argsFor(script), { review: null })).rejects.toThrow(/no result|unavailable/i)
    })

    it(`${script} fails when the reviewer returns an invalid verdict`, async () => {
      await expect(run(script, argsFor(script), { review: { status: 'ok', findings: [], verdict: 'looks fine' } })).rejects.toThrow(/verdict/i)
    })

    // Regression: `fix_required` with an empty findings array validated, and actionableFrom then had
    // nothing to hand the fixer — so the fix loop was skipped and a green gate completed the run over
    // a reviewer that had explicitly withheld approval.
    it(`${script} fails when the reviewer demands a fix but files nothing`, async () => {
      await expect(run(script, argsFor(script), { review: { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [] } })).rejects.toThrow(
        /fix_required/i
      )
    })

    it(`${script} fails when the reviewer demands a fix and files only nits`, async () => {
      await expect(
        run(script, argsFor(script), {
          review: { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [{ title: 'spelling', severity: 'nit', file: 'a.js:1', evidence: 'e', fix: 'f' }] },
        })
      ).rejects.toThrow(/fix_required/i)
    })

    it(`${script} fails when a gate command fails`, async () => {
      await expect(
        run(script, argsFor(script), {
          gate: { status: 'fail', commands: [{ command: 'just check-quick', status: 'fail', detail: '3 tests failed' }], failures: ['3 tests failed'], diff_stat: '', commits: [] },
        })
      ).rejects.toThrow(/gate/i)
    })

    it(`${script} fails when the gate reports no commands at all`, async () => {
      await expect(run(script, argsFor(script), { gate: { status: 'pass', commands: [], failures: [], diff_stat: '', commits: [] } })).rejects.toThrow(/gate/i)
    })

    // Regression: a gate that reports `status: 'pass'` while a required command was skipped is not a
    // gate. Before this, gateProblem excluded every `skipped` command from the failure set, so a run
    // could complete green with `just check-quick` never executed.
    it(`${script} fails when a required gate command is skipped`, async () => {
      await expect(
        run(script, argsFor(script), {
          gate: {
            status: 'pass',
            commands: [
              { command: 'just check-quick', status: 'skipped', detail: 'no Rust file changed' },
              { command: 'just lint', status: 'pass' },
            ],
            failures: [],
            diff_stat: '',
            commits: [],
          },
        })
      ).rejects.toThrow(/required gate/i)
    })

    it(`${script} fails when a required gate command never ran at all`, async () => {
      await expect(
        run(script, argsFor(script), {
          gate: { status: 'pass', commands: [{ command: 'bunx vitest run src/lib', status: 'pass' }], failures: [], diff_stat: '', commits: [] },
        })
      ).rejects.toThrow(/required gate/i)
    })

    // Regression: gateProblem excluded every `skipped` command from the failure set, so a gate could
    // report `just check-quick` and `just lint` green while the targeted cargo tests the spec asked
    // for were never run, and the workflow still returned a green ledger. A command that did not
    // apply is left off the list; one that is listed has to have passed.
    it(`${script} fails when a listed gate command is skipped, required or not`, async () => {
      await expect(
        run(script, argsFor(script), {
          gate: {
            status: 'pass',
            commands: [
              { command: 'just check-quick', status: 'pass' },
              { command: 'just lint', status: 'pass' },
              { command: 'cd src-tauri && cargo test coordination', status: 'skipped', detail: 'no Rust file changed' },
            ],
            failures: [],
            diff_stat: '',
            commits: [],
          },
        })
      ).rejects.toThrow(/cargo test coordination/)
    })

    it(`${script} passes when the gate lists only commands that ran and passed`, async () => {
      const { result } = await run(script, argsFor(script), {
        gate: {
          status: 'pass',
          commands: [
            { command: 'just check-quick', status: 'pass' },
            { command: 'just lint', status: 'pass' },
            { command: 'cd src-tauri && cargo test coordination', status: 'pass' },
          ],
          failures: [],
          diff_stat: '',
          commits: [],
        },
      })
      expect(result.gate.status).toBe('pass')
    })

    // Regression: a non-empty `failures` was read only when the top-level status was not `pass`, and
    // `error` only alongside a non-pass status — so a gate that contradicted itself (`status: 'pass'`
    // next to the failures it collected) was accepted as green.
    it(`${script} fails when a passing gate still reports failures`, async () => {
      await expect(
        run(script, argsFor(script), {
          gate: {
            status: 'pass',
            commands: [
              { command: 'just check-quick', status: 'pass' },
              { command: 'just lint', status: 'pass' },
            ],
            failures: ['cd src-tauri && cargo test coordination: 2 failed'],
            diff_stat: '',
            commits: [],
          },
        })
      ).rejects.toThrow(/2 failed/)
    })

    it(`${script} fails when a passing gate still reports an error`, async () => {
      await expect(
        run(script, argsFor(script), {
          gate: {
            status: 'pass',
            commands: [
              { command: 'just check-quick', status: 'pass' },
              { command: 'just lint', status: 'pass' },
            ],
            failures: [],
            error: 'the lint lane never returned',
            diff_stat: '',
            commits: [],
          },
        })
      ).rejects.toThrow(/never returned/)
    })

    it(`${script} adds args.requiredGates on top of the two default gates, which cannot be opted out of`, async () => {
      const defaults = [{ command: 'just check-quick', status: 'pass' }, { command: 'just lint', status: 'pass' }]
      const custom = argsFor(script, { gates: "'bun run test'", requiredGates: ['bun run test'] })
      await expect(
        run(script, custom, {
          gate: { status: 'pass', commands: defaults.concat([{ command: 'bun run test', status: 'skipped', detail: 'slow' }]), failures: [], diff_stat: '', commits: [] },
        })
      ).rejects.toThrow(/bun run test/)
      await expect(
        run(script, custom, {
          gate: { status: 'pass', commands: [{ command: 'bun run test', status: 'pass' }], failures: [], diff_stat: '', commits: [] },
        })
      ).rejects.toThrow(/just check-quick/)
      const { result } = await run(script, custom, {
        gate: { status: 'pass', commands: defaults.concat([{ command: 'bun run test', status: 'pass' }]), failures: [], diff_stat: '', commits: [] },
      })
      expect(result.gate.status).toBe('pass')

      // `[]` is not an opt-out: the defaults still have to run.
      await expect(
        run(script, argsFor(script, { requiredGates: [] }), {
          gate: { status: 'pass', commands: [{ command: 'bun run test', status: 'pass' }], failures: [], diff_stat: '', commits: [] },
        })
      ).rejects.toThrow(/just check-quick/)
    })

    it(`${script} tells the gate agent which commands must actually run`, async () => {
      const { calls } = await run(script, argsFor(script))
      const gate = calls.find((c) => c.label.startsWith('gate:'))
      expect(gate.prompt).toMatch(/required and must actually run/i)
      expect(gate.prompt).toContain('just check-quick')
    })

    it(`${script} asks the gate for a structured result`, async () => {
      const { calls } = await run(script, argsFor(script))
      const gate = calls.find((c) => c.label.startsWith('gate:'))
      expect(gate.opts.schema, 'the gate must return a schema-validated result').toBeTruthy()
      expect(gate.opts.schema.required).toContain('status')
    })

    it(`${script} returns a green ledger when review and gate pass`, async () => {
      const { result } = await run(script, argsFor(script))
      expect(result.gate.status).toBe('pass')
      expect(result.ledger.reviewers.length).toBeGreaterThan(0)
    })
  }
})

describe('workflow procedures — the ledger', () => {
  it('records the resolved Codex model and effort instead of a hard-coded claim', async () => {
    const { result } = await run('feature-pr.js', { ...BASE_ARGS, effort: 'xhigh', codexModel: 'gpt-5.6-luna' })
    expect(result.ledger.effort).toBe('xhigh')
    expect(JSON.stringify(result.ledger.models)).toContain('gpt-5.6-luna')
  })

  it('records the CLI default honestly when no Codex model is pinned', async () => {
    const { result } = await run('feature-pr.js', BASE_ARGS)
    expect(JSON.stringify(result.ledger.models)).toMatch(/cli default/i)
    expect(JSON.stringify(result.ledger.models)).not.toContain('gpt-5.6-terra')
  })

  it('only records a reviewer that actually returned a result', async () => {
    const seen = []
    await expect(
      run('feature-pr.js', BASE_ARGS, {
        review: (call, state) => {
          seen.push(call.label)
          return state.counts.review === 1 ? OK_REVIEW : { status: 'unavailable', error: 'boom', findings: [], verdict: 'approve' }
        },
      })
    ).rejects.toThrow(/unavailable/i)
    expect(seen.length).toBeGreaterThan(1)
  })
})

describe('workflow procedures — the outcome', () => {
  const major = { title: 'restart bypasses the notice', severity: 'major', file: 'a.js:2', evidence: 'e', fix: 'f' }
  const minor = { title: 'warning copy is vague', severity: 'minor', file: 'a.js:3', evidence: 'e', fix: 'f' }

  // Regression: merge commit 2bbe0b4 (PR #75; accounts plan row 20b) followed three feature-pr
  // rounds and two fix-round rounds, while an open major could still sit under a completed ledger.
  for (const script of MUTATING) {
    it(`${script} requires follow-up when every review round leaves a major open`, async () => {
      const { result } = await run(script, argsFor(script), {
        review: { ...OK_REVIEW, verdict: 'fix_required', findings: [major] },
      })
      expect(result.outcome).toBe('followup_required')
      expect(result.ledger.remaining.map((finding) => finding.title)).toContain(major.title)
      expect(result.gate.status).toBe('pass')
      // Runnable as handed back: fix-round needs the checkout, and the branch
      // and spec keep the next round on the same work.
      expect(result.followup).toEqual({
        name: 'fix-round',
        args: {
          worktree: BASE_ARGS.worktree,
          branch: BASE_ARGS.branch,
          base: 'main',
          spec: BASE_ARGS.spec,
          title: result.ledger.title,
          findings: result.ledger.remaining,
          startRound: result.ledger.rounds + 1,
        },
      })
    })

    it(`${script} completes a clean run without a follow-up`, async () => {
      const { result } = await run(script, argsFor(script))
      expect(result.outcome).toBe('complete')
      expect(result).not.toHaveProperty('followup')
    })

    it(`${script} completes when only a minor remains`, async () => {
      const { result } = await run(script, argsFor(script), {
        review: { ...OK_REVIEW, findings: [minor] },
      })
      expect(result.outcome).toBe('complete')
      expect(result.ledger.remaining.map((finding) => finding.title)).toContain(minor.title)
      expect(result).not.toHaveProperty('followup')
    })
  }

  it('fix-round preserves an open major and requires another call at maxRounds 1', async () => {
    const { result } = await run(
      'fix-round.js',
      argsFor('fix-round.js', { maxRounds: 1 }),
      { review: { ...OK_REVIEW, verdict: 'fix_required', findings: [major] } }
    )
    expect(result.outcome).toBe('followup_required')
    expect(result.ledger.remaining).toEqual([expect.objectContaining(major)])
    expect(result.followup).toEqual({
      name: 'fix-round',
      args: {
        worktree: BASE_ARGS.worktree,
        branch: BASE_ARGS.branch,
        base: 'main',
        spec: BASE_ARGS.spec,
        title: result.ledger.title,
        findings: result.ledger.remaining,
        startRound: result.ledger.rounds + 1,
      },
    })
  })
})

describe('workflow procedures — the verdict contract', () => {
  const minor = { title: 'weak test', severity: 'minor', file: 'a.js:1', evidence: 'e', fix: 'f' }
  const nit = { title: 'spelling', severity: 'nit', file: 'a.js:1', evidence: 'e', fix: 'f' }
  const major = { title: 'drops the second event', severity: 'major', file: 'a.js:2', evidence: 'e', fix: 'f' }

  for (const script of MUTATING) {
    // Regression: a `fix_required` carrying only minors validated, and actionableFrom widened the
    // actionable set whenever a fix_required verdict was present — so a minor-only withheld approval
    // ran a fix round over trivia in one script and, where the loop filtered to blocker/major, let a
    // green gate complete the run over a review that never approved. The contract is now: fix_required
    // requires at least one blocker or major, and anything else is malformed.
    it(`${script} fails when a fix_required files no blocker or major`, async () => {
      await expect(
        run(script, argsFor(script), { review: { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [minor, nit] } })
      ).rejects.toThrow(/fix_required/i)
    })

    it(`${script} fails when a fix_required files only a minor`, async () => {
      await expect(run(script, argsFor(script), { review: { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [minor] } })).rejects.toThrow(
        /fix_required/i
      )
    })

    it(`${script} re-requests a malformed review exactly once, with the contract restated`, async () => {
      const seen = []
      await expect(
        run(script, argsFor(script), {
          review: (call) => {
            seen.push(call)
            return { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [minor] }
          },
        })
      ).rejects.toThrow(/fix_required/i)
      const retries = seen.filter((c) => c.label.includes('recontract'))
      expect(retries.length, 'every malformed lane is re-requested').toBeGreaterThan(0)
      expect(retries.length, 'and re-requested only once each').toBe(seen.length - retries.length)
      expect(retries[0].prompt, 'the re-request says the previous review was rejected').toMatch(/RE-REQUEST/)
      expect(retries[0].prompt, 'and restates the contract it broke').toMatch(/blocker or major/i)
    })

    it(`${script} runs the fix loop for a fix_required that files a major`, async () => {
      const { state, result } = await run(script, argsFor(script), {
        review: (call, s) => (s.counts.review === 1 ? { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [major] } : OK_REVIEW),
      })
      expect(state.counts.review, 'the re-review must run').toBeGreaterThan(1)
      expect(result.ledger.majors).toBeGreaterThan(0)
    })

    it(`${script} keeps a minor filed under approve and hands it back in remaining`, async () => {
      const { result } = await run(script, argsFor(script), { review: { status: 'ok', reviewer: 'codex', verdict: 'approve', findings: [minor, nit] } })
      expect(result.gate.status).toBe('pass')
      expect(result.ledger.remaining.map((f) => f.title)).toContain('weak test')
    })

    it(`${script} states the verdict contract to the reviewer`, async () => {
      const { calls } = await run(script, argsFor(script))
      const review = calls.find((c) => c.label.startsWith('review:') || c.label.startsWith('verify:'))
      expect(review.prompt).toMatch(/fix_required/)
      expect(review.prompt).toMatch(/at least one blocker or major/i)
    })
  }
})

describe('workflow procedures — attribution', () => {
  // Regression: the wrapper was told to claim reviewer 'codex gpt-5.6' whatever args.codexModel
  // pinned, so a Luna or Terra run filed findings under a model that never ran.
  it('names the requested Codex model as the reviewer, never a hard-coded one', async () => {
    for (const script of MUTATING) {
      const { calls } = await run(script, argsFor(script, { codexModel: 'gpt-5.6-luna' }))
      const prompts = calls.map((c) => c.prompt).join('\n')
      expect(prompts, script).toContain('gpt-5.6-luna')
      expect(prompts.match(/gpt-5\.6(?!-)/g), `${script} claims a model nobody pinned`).toBeNull()
    }
  })

  it('claims no Codex model at all when none is pinned', async () => {
    for (const script of MUTATING) {
      const { calls } = await run(script, argsFor(script))
      const prompts = calls.map((c) => c.prompt).join('\n')
      expect(prompts.match(/gpt-5\.6/g), `${script} invents a Codex model`).toBeNull()
    }
  })

  it('requires every review lane to report the model that ran it', async () => {
    for (const script of MUTATING) {
      const { calls } = await run(script, argsFor(script))
      const review = calls.find((c) => c.label.startsWith('review:') || c.label.startsWith('verify:'))
      expect(review.opts.schema.required, script).toContain('model_used')
    }
  })
})

describe('fix-round — the handover', () => {
  const minor = { title: 'weak test', severity: 'minor', file: 'a.js:1', evidence: 'e', fix: 'f' }

  // Regression: only blocker/major findings were actionable, so a minor-only handover (which the
  // other procedures do emit as `remaining` when a reviewer returns fix_required) skipped every fix
  // and re-review and returned remaining: [] — the finding vanished.
  it('fixes an open minor handed over from a run that stopped short', async () => {
    const { state } = await run('fix-round.js', { ...BASE_ARGS, findings: [minor] })
    expect(state.counts.work, 'the fixer must run').toBe(1)
    expect(state.counts.review, 'the re-review must run').toBe(1)
  })

  // A re-review that approves while filing a minor is well-formed: the minor is not what another fix
  // round is for, so it rides back in `remaining` instead of looping (or vanishing).
  it('carries an unclosed minor back in remaining', async () => {
    const { result } = await run(
      'fix-round.js',
      { ...BASE_ARGS, findings: [minor], maxRounds: 1 },
      { review: { status: 'ok', reviewer: 'codex', verdict: 'approve', findings: [minor] } }
    )
    expect(result.ledger.remaining.map((f) => f.title)).toContain('weak test')
  })

  it('has nothing to fix when only nits are handed over', async () => {
    const { state, logs } = await run('fix-round.js', { ...BASE_ARGS, findings: [{ ...minor, severity: 'nit' }] })
    expect(state.counts.work).toBe(0)
    expect(logs.join('\n')).toMatch(/nit/i)
  })
})

describe('research-sweep', () => {
  it('reports an unavailable researcher instead of dropping it silently', async () => {
    const { result, logs } = await run(
      'research-sweep.js',
      { ...BASE_ARGS, question: 'what?', researchers: [{ family: 'codex', label: 'one', prompt: 'look' }, { family: 'opus', label: 'two', prompt: 'look' }] },
      { research: (call, s) => (s.counts.research === 1 ? { status: 'unavailable', error: 'codex exited 124', summary: '', report_path: '', key_facts: [], unverified: [] } : DEFAULTS.research) }
    )
    expect(result.failed.map((f) => f.label)).toContain('one')
    expect(logs.join('\n')).toMatch(/unavailable|no result/i)
  })
})
