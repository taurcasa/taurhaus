export const meta = {
  name: 'docs-sweep',
  description: 'Documentation drift sweep in a worktree: sweep the docs against the code, verify every claim cross-family, gate',
  phases: [
    { title: 'Sweep', detail: 'sweeper walks the file groups, commits per group, writes the drift table', model: 'opus' },
    { title: 'Verify', detail: 'the other family verifies every claim against the code; fix loop, max 3 rounds', model: 'opus' },
    { title: 'Gate', detail: 'lint and the test lanes in the worktree', model: 'opus' },
  ],
}

const NAME = 'docs-sweep'

// ── lib: shared rules, byte-identical in every script here (workflow scripts cannot import) ──
const A = args || {}
const ROOT = A.worktree || A.repo
if (!ROOT) throw new Error(NAME + ': args.worktree (or args.repo) is required — the absolute path of the checkout to work in')
const BRANCH = A.branch || ''
const BASE = A.base || 'main'
const SPEC = A.spec || ''
const SCRATCH = A.scratch || '/tmp/taurhaus-workflows'
const MODEL = 'opus'
const GATES =
  A.gates ||
  "'just check-quick' and 'just lint', plus 'cd src-tauri && cargo test <touched module paths>' (check-quick does not run the Rust tests); vitest runs from the checkout root"

// Every agent runs on Opus in this repo's model split; effort is inherited unless args.effort pins one.
function call(o) {
  return A.effort ? { model: MODEL, effort: A.effort, ...o } : { model: MODEL, ...o }
}

function trailers(family) {
  const author =
    family === 'codex'
      ? 'Co-Authored-By: Codex (gpt-5.6) <noreply@openai.com>'
      : 'Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>'
  return 'Commit with these trailer lines:\n' + (A.sessionUrl ? author + '\nClaude-Session: ' + A.sessionUrl : author)
}

const RULES = {
  checkout:
    'CHECKOUT: ' +
    ROOT +
    (BRANCH ? ' (branch ' + BRANCH + ', already checked out; do not switch branches)' : '') +
    ' — work ONLY there, never in another checkout of the same repo. Read ' +
    ROOT +
    '/CLAUDE.md first and follow it.',
  spec: SPEC ? 'SPEC: ' + SPEC + ' — the minimum deliverable only; respect its "not building" list.' : '',
  commits:
    'COMMIT DISCIPLINE: `git add` the files you touched (never `git add -A`) and commit after every green step — a killed run must leave a tree one `git stash` away from clean. Never edit ledger rows in plan documents; the orchestrator fills them at merge.',
  tdd:
    'TDD: write the test first, run it to observe red, then implement and observe green. A regression test carries a "// Regression:" comment naming the commit that broke it.',
  gates: 'GATES: ' + GATES + '.',
  safety:
    'SAFETY: tests never read or write the real ~/.claude*, ~/.codex, ~/.gemini or ~/.grok and never invoke a real CLI; no load or stress runs; kill anything you start (trap/finally) and never kill a process you did not start; never print tokens or secrets.',
  readOnly:
    'READ-ONLY: change no file in any repository and run no git write command; write only under ' +
    SCRATCH +
    '. Report facts you verified (file:line, command output) and mark inferences UNVERIFIED with what would settle them.',
  scope:
    'SCOPE RULE: judge against the spec\'s minimum deliverable and its "not building" list — missing scaffolding (tests or docs for tooling, dry-run niceties, extra configurability) is at most a minor, and majors are reserved for defects a user would hit.',
  evidence:
    'Do NOT modify any file. Report only findings you verified with file:line evidence; severity blocker/major/minor/nit; verdict fix_required only for blocker/major.',
}

// The Codex lane: a thin Opus wrapper drives `codex exec` detached and polls for the EXIT marker,
// because one Bash call is capped at 10 minutes and Codex runs take longer.
function codexWrapper(o) {
  const base = SCRATCH + '/codex-' + o.tag
  const out = base + (o.schema ? '.json' : '.out.md')
  return [
    'You are a thin wrapper around the Codex CLI (gpt-5.6): it does the work, you do not. Do NOT do the task yourself.',
    '1) `mkdir -p ' +
      SCRATCH +
      '`; write the TASK below verbatim to ' +
      base +
      '.prompt.md' +
      (o.schema ? ', and this JSON Schema verbatim to ' + base + '.schema.json:\n' + JSON.stringify(o.schema) : '.'),
    '2) Launch Codex DETACHED: rm -f ' +
      out +
      ' ' +
      base +
      '.log; cd ' +
      ROOT +
      " && (setsid nohup bash -c 'timeout " +
      o.timeout +
      ' codex exec --yolo --skip-git-repo-check -C ' +
      ROOT +
      (o.schema ? ' --output-schema ' + base + '.schema.json' : '') +
      ' -o ' +
      out +
      ' - < ' +
      base +
      '.prompt.md > ' +
      base +
      '.log 2>&1; echo EXIT=$? >> ' +
      base +
      ".log' >/dev/null 2>&1 < /dev/null & disown)",
    '3) Poll with Bash calls of at most 9 minutes each: `until grep -q "^EXIT=" ' +
      base +
      '.log; do sleep 20; done` — repeat the call until the marker appears; wait rather than abandoning a run that is still going.',
    '4) Read ' +
      out +
      ' and the tail of ' +
      base +
      '.log.' +
      (o.resume
        ? ' If Codex left uncommitted work or an unfinished implementation (a run killed by the timeout counts), run up to THREE follow-up turns, each: `cd ' +
          ROOT +
          ' && timeout ' +
          o.timeout +
          ' codex exec resume --last --yolo --skip-git-repo-check -o ' +
          base +
          "-r<N>.md 'Continue from the current tree: commit any green step that is already complete, then proceed step by step, committing after each; run the gates; commit with the trailers.'` (`codex exec resume` does not accept -C — run it from " +
          ROOT +
          '; same detached + poll pattern). Report every turn and its exit code under deviations, and verify the gate claims yourself (`cd src-tauri && cargo check --all-targets`) before returning.'
        : ''),
    '5) Return the result as your structured output' +
      (o.reviewer
        ? ", with reviewer='" +
          o.reviewer +
          "'. If Codex failed or its JSON is invalid, retry steps 2-4 once; if it still fails, return verdict 'approve' with a single 'nit' finding titled 'codex unavailable' carrying the error as evidence."
        : '.'),
    '',
    'TASK FOR CODEX:',
    o.task,
  ].join('\n')
}
// ── end lib ──

const SWEEPER = A.implementer === 'codex' ? 'codex' : 'opus'
const VERIFY_FAMILY = SWEEPER === 'codex' ? 'opus' : 'codex'
const TITLE = A.title || 'documentation drift sweep'
const TAG = (A.tag || BRANCH || NAME).replace(/[^A-Za-z0-9._-]+/g, '-')
const DIFF = 'git diff ' + BASE + '...HEAD'
const TABLE = A.table || SCRATCH + '/' + TAG + '-drift-table.md'
const MAX_ROUNDS = A.maxRounds || 3
const GROUPS =
  (Array.isArray(A.groups) ? A.groups.join('; ') : A.groups) ||
  'README/ARCHITECTURE/CLAUDE.md/CONTRIBUTING; CHANGELOG (the [Unreleased] section only); docs/architecture; docs/design (the plan documents the spec lists); docs/operations plus the testing and visual guides; docs/features, getting-started and team-templates; the e2e READMEs and spec comments; Rust `//!` module docs and the src/lib header comments that name tools'

const COMMON = [RULES.checkout, RULES.spec, RULES.gates, RULES.commits, RULES.safety].filter(Boolean).join('\n')
const EVIDENCE_RULE =
  'Verify every claim against the code in this checkout (grep, `git show`, `just --list`, check that the named files, paths, events, recipes and flags exist and behave as described) and cite file:line evidence. Never rewrite a name mechanically — write what the code does.'

const SWEEP_SCHEMA = {
  type: 'object',
  required: ['summary', 'commits', 'files_changed', 'table_path', 'unresolved'],
  properties: {
    summary: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    files_changed: { type: 'array', items: { type: 'string' } },
    table_path: { type: 'string', description: 'the drift table: before -> after with file:line evidence' },
    unresolved: { type: 'array', items: { type: 'string' }, description: 'drift you could not settle from the code' },
  },
}

const FINDINGS_SCHEMA = {
  type: 'object',
  required: ['findings', 'verdict'],
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        required: ['title', 'severity', 'file', 'evidence', 'fix'],
        properties: {
          title: { type: 'string' },
          severity: { type: 'string', enum: ['blocker', 'major', 'minor', 'nit'] },
          file: { type: 'string' },
          evidence: { type: 'string' },
          fix: { type: 'string' },
        },
      },
    },
    verdict: { type: 'string', enum: ['approve', 'fix_required'] },
    reviewer: { type: 'string' },
  },
}

const reviewers = new Set()
function verifyPrompt(round, prior) {
  return [
    RULES.scope,
    'You are an independent documentation verifier from a different model family than the author. Checkout: ' +
      ROOT +
      (BRANCH ? ', branch ' + BRANCH : '') +
      '; review ONLY `' +
      DIFF +
      '` (docs and comments) plus the drift table at ' +
      TABLE +
      '.',
    'For EVERY changed paragraph and every table row: verify the claim against the code (grep, `git show`, `just --list`, run a cited command when it is cheap). Report as findings the claims that are wrong, stale or unverifiable, the enumerations that miss a case the code has, and the counts that disagree with the code. Severity: major for a wrong or stale claim a reader would act on, minor for imprecision, nit for style.',
    prior
      ? 'This is re-verification round ' +
        round +
        '. Prior findings (JSON): ' +
        prior +
        '. First verify each prior finding is resolved, with file:line evidence, then look for drift introduced by the fix.'
      : '',
    RULES.evidence,
  ]
    .filter(Boolean)
    .join('\n')
}

function verifyAgent(round, prior) {
  const label = 'verify:' + VERIFY_FAMILY + '-r' + round
  if (VERIFY_FAMILY === 'opus') {
    return agent(verifyPrompt(round, prior) + "\nSet reviewer='opus docs'.", call({ label: label, phase: 'Verify', schema: FINDINGS_SCHEMA }))
  }
  return agent(
    codexWrapper({
      tag: TAG + '-verify-r' + round,
      task: verifyPrompt(round, prior) + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: 'codex gpt-5.6 docs',
    }),
    call({ label: label, phase: 'Verify', schema: FINDINGS_SCHEMA })
  )
}

function sweepAgent(task, tag, label, groupPhase) {
  if (SWEEPER === 'codex') {
    return agent(
      codexWrapper({
        tag: tag,
        task: task + '\nWhen done print a final JSON object with keys summary, commits, files_changed, table_path, unresolved.',
        schema: SWEEP_SCHEMA,
        timeout: 3000,
      }),
      call({ label: label + ':codex', phase: groupPhase, schema: SWEEP_SCHEMA })
    )
  }
  return agent(task + '\nReturn the structured summary.', call({ label: label + ':opus', phase: groupPhase, schema: SWEEP_SCHEMA }))
}

phase('Sweep')
const sweep = await sweepAgent(
  [
    COMMON,
    '',
    'You are the documentation sweeper for the ' +
      TITLE +
      '. Work file group by file group — ' +
      GROUPS +
      ' — with one commit per group.',
    EVIDENCE_RULE +
      ' Re-measure every count the docs state (IPC commands, daemon methods, just recipes) instead of trusting the prose.' +
      (A.notes ? ' ' + A.notes : ''),
    'Write the drift table (before -> after, evidence file:line) to ' + TABLE + ' as you go, and return it as table_path.',
    trailers(SWEEPER),
  ].join('\n'),
  TAG + '-sweep',
  'sweep:' + TAG,
  'Sweep'
)

phase('Verify')
let round = 1
let review = await verifyAgent(round, null)
if (review) reviewers.add(review.reviewer || VERIFY_FAMILY)
let findings = review ? (review.findings || []).map((f) => ({ ...f, reviewer: review.reviewer, round: round })) : []
let actionable = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
const allFindings = findings.slice()
log('Verify r1 (' + VERIFY_FAMILY + '): ' + findings.length + ' findings, ' + actionable.length + ' actionable')

const fixes = []
while (actionable.length > 0 && fixes.length < MAX_ROUNDS) {
  fixes.push(
    await sweepAgent(
      [
        COMMON,
        '',
        'You are the fixer for the ' +
          TITLE +
          ', round ' +
          (fixes.length + 1) +
          '. Apply these verified findings — check each against the code first and skip a wrong one with a stated reason — and take the minors where they are trivial:',
        JSON.stringify(findings, null, 1),
        EVIDENCE_RULE,
        'Update the drift table at ' + TABLE + ' accordingly and commit per file group.',
        trailers(SWEEPER),
      ].join('\n'),
      TAG + '-fix-r' + (fixes.length + 1),
      'fix:' + TAG + '-r' + (fixes.length + 1),
      'Verify'
    )
  )
  const prior = JSON.stringify(actionable)
  round += 1
  review = await verifyAgent(round, prior)
  if (review) reviewers.add(review.reviewer || VERIFY_FAMILY)
  findings = review ? (review.findings || []).map((f) => ({ ...f, reviewer: review.reviewer, round: round })) : []
  allFindings.push(...findings)
  actionable = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
  log('Re-verify r' + round + ': ' + findings.length + ' findings, ' + actionable.length + ' actionable')
}
if (actionable.length > 0) {
  log('Stopped short: ' + actionable.length + ' actionable findings left after ' + fixes.length + ' fix rounds')
}

phase('Gate')
const gate = await agent(
  [
    COMMON,
    '',
    'Final gate for the ' +
      TITLE +
      ': run the gates above in this checkout. No code change is expected — if one is needed for a trivial reason, commit it.',
    trailers(SWEEPER),
    'Return {check_quick, lint, tests, diff_stat, commits, status} as JSON text.',
  ].join('\n'),
  call({ label: 'gate:' + TAG, phase: 'Gate' })
)

return {
  ledger: {
    title: TITLE,
    size: A.size || 'docs',
    implementer: SWEEPER,
    reviewers: [...reviewers],
    rounds: round,
    majors: allFindings.filter((f) => f.severity === 'blocker' || f.severity === 'major').length,
    findings: allFindings,
    remaining: actionable,
    table: sweep && sweep.table_path ? sweep.table_path : TABLE,
    unresolved: sweep && sweep.unresolved ? sweep.unresolved : [],
  },
  commits: [sweep, ...fixes].filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
