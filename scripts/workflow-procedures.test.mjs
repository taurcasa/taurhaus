// @vitest-environment node
// Behavioural tests for the versioned procedures in .claude/workflows: they run each script
// with the Workflow API stubbed, so the control flow (fail-closed review, fail-closed gate,
// the Codex launcher's quoting and flags) is exercised without spawning a single agent.
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
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

  it('tells the wrapper to report an unavailable Codex instead of inventing an approval', async () => {
    const { calls } = await run('feature-pr.js', { ...BASE_ARGS, implementer: 'opus' })
    const review = calls.find((c) => c.label.startsWith('review:'))
    expect(review.prompt).toMatch(/status='unavailable'|status: 'unavailable'|status=unavailable/)
    expect(review.prompt).not.toMatch(/verdict 'approve'|return verdict "approve"/)
  })
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

    it(`${script} still passes when an optional command is skipped with a reason`, async () => {
      const { result } = await run(script, argsFor(script), {
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
      expect(result.gate.status).toBe('pass')
    })

    it(`${script} takes the required commands from args.requiredGates when a spec names other gates`, async () => {
      const custom = argsFor(script, { gates: "'bun run test'", requiredGates: ['bun run test'] })
      await expect(
        run(script, custom, {
          gate: { status: 'pass', commands: [{ command: 'bun run test', status: 'skipped', detail: 'slow' }], failures: [], diff_stat: '', commits: [] },
        })
      ).rejects.toThrow(/bun run test/)
      const { result } = await run(script, custom, {
        gate: { status: 'pass', commands: [{ command: 'bun run test', status: 'pass' }], failures: [], diff_stat: '', commits: [] },
      })
      expect(result.gate.status).toBe('pass')
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

describe('workflow procedures — the review verdict', () => {
  it('runs a fix round when the reviewer demands one without filing a major', async () => {
    const { result, state } = await run('feature-pr.js', BASE_ARGS, {
      review: (call, s) =>
        s.counts.review <= 2
          ? { status: 'ok', reviewer: 'codex', verdict: 'fix_required', findings: [{ title: 'weak test', severity: 'minor', file: 'a.js:1', evidence: 'e', fix: 'f' }] }
          : OK_REVIEW,
    })
    expect(state.counts.work).toBeGreaterThan(1)
    expect(result.ledger.rounds).toBeGreaterThan(1)
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
