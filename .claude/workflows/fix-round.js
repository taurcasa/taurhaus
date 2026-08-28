export const meta = {
  name: 'fix-round',
  description: 'Extra fix -> cross-family re-review rounds for a change whose main loop stopped short, then gate',
  phases: [
    { title: 'Fix', detail: 'fixer applies the open findings, the other family re-reviews (conformance lens)', model: 'opus' },
    { title: 'Gate', detail: 'check-quick, lint, targeted tests', model: 'opus' },
  ],
}

const NAME = 'fix-round'

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

const IMPLEMENTER = A.implementer === 'codex' ? 'codex' : 'opus'
const REVIEW_FAMILY = IMPLEMENTER === 'codex' ? 'opus' : 'codex'
const TITLE = A.title || SPEC || BRANCH || NAME
const TAG = (A.tag || BRANCH || NAME).replace(/[^A-Za-z0-9._-]+/g, '-')
const DIFF = 'git diff ' + BASE + '...HEAD'
const OPEN = Array.isArray(A.findings) ? A.findings : []
if (OPEN.length === 0) throw new Error(NAME + ': args.findings must be the open findings from the run that stopped short')
const START_ROUND = A.startRound || 2
const MAX_ROUNDS = A.maxRounds || 2

const COMMON = [RULES.checkout, RULES.spec, RULES.gates, RULES.tdd, RULES.commits, RULES.safety].filter(Boolean).join('\n')

const IMPL_SCHEMA = {
  type: 'object',
  required: ['summary', 'commits', 'files_changed', 'tests_added', 'red_observed', 'gate', 'deviations'],
  properties: {
    summary: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    files_changed: { type: 'array', items: { type: 'string' } },
    tests_added: { type: 'array', items: { type: 'string' } },
    red_observed: { type: 'string', description: 'which tests failed before the fix and how' },
    gate: { type: 'string', description: 'the gate commands run and their outcome' },
    deviations: { type: 'array', items: { type: 'string' }, description: 'findings skipped as wrong, with the reason' },
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
function reviewPrompt(round, prior) {
  return [
    RULES.scope,
    'You are an independent code reviewer from a different model family than the author. Checkout: ' +
      ROOT +
      (BRANCH ? ', branch ' + BRANCH : '') +
      '; review ONLY the changes in `' +
      DIFF +
      '` (run it — if ' +
      BASE +
      ' is missing use origin/' +
      BASE +
      '; read the surrounding code as needed; you may run the test lanes). Context: the change implements ' +
      TITLE +
      (SPEC && SPEC !== TITLE ? ', specified in ' + SPEC + ' — read it first' : '') +
      '.',
    'Lens: conformance and correctness — does the change implement the spec item completely; are the tests genuinely red-before/green-after (inspect them, run them); edge cases and backward compatibility; anything missing or out of scope.',
    'This is re-review round ' +
      round +
      '. Prior findings (JSON): ' +
      prior +
      '. First verify each prior finding is resolved, with file:line evidence, then look for regressions introduced by the fix.',
    RULES.evidence,
  ].join('\n')
}

function reviewAgent(round, prior) {
  const label = 'review:' + REVIEW_FAMILY + '-r' + round
  if (REVIEW_FAMILY === 'opus') {
    return agent(reviewPrompt(round, prior) + "\nSet reviewer='opus conformance'.", call({ label: label, phase: 'Fix', schema: FINDINGS_SCHEMA }))
  }
  return agent(
    codexWrapper({
      tag: TAG + '-review-r' + round,
      task: reviewPrompt(round, prior) + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: 'codex gpt-5.6 conformance',
    }),
    call({ label: label, phase: 'Fix', schema: FINDINGS_SCHEMA })
  )
}

function fixAgent(findings, minors, round) {
  const task = [
    COMMON,
    '',
    'You are the fixer for ' +
      TITLE +
      ', round ' +
      round +
      '. Apply these confirmed findings — verify each against the code first and skip a wrong one with a stated reason. Every blocker and major gets a red-first regression test:',
    JSON.stringify(findings, null, 1),
    'Take the minors too where they are trivial: ' + JSON.stringify(minors, null, 1),
    'Keep the change minimal and local to the files named — no new features, no architecture reshaping, no wire changes.' + (A.fixNotes ? ' ' + A.fixNotes : ''),
    'Re-run the gates.',
    trailers(IMPLEMENTER),
  ].join('\n')
  if (IMPLEMENTER === 'codex') {
    return agent(
      codexWrapper({
        tag: TAG + '-fix-r' + round,
        task: task + '\nWhen done print a final JSON object with keys summary, commits, files_changed, tests_added, red_observed, gate, deviations.',
        schema: IMPL_SCHEMA,
        timeout: 3000,
        resume: true,
      }),
      call({ label: 'fix:' + TAG + '-r' + round + ':codex', phase: 'Fix', schema: IMPL_SCHEMA })
    )
  }
  return agent(task + '\nReturn the structured summary.', call({ label: 'fix:' + TAG + '-r' + round + ':opus', phase: 'Fix', schema: IMPL_SCHEMA }))
}

phase('Fix')
let round = START_ROUND
let actionable = OPEN.filter((f) => f.severity === 'blocker' || f.severity === 'major')
let minors = OPEN.filter((f) => f.severity === 'minor')
const allFindings = OPEN.slice()
const fixes = []
log('Starting at round ' + round + ' with ' + actionable.length + ' actionable findings (max ' + MAX_ROUNDS + ' rounds)')
while (actionable.length > 0 && fixes.length < MAX_ROUNDS) {
  fixes.push(await fixAgent(actionable, minors, round))
  const prior = JSON.stringify(actionable)
  round += 1
  const review = await reviewAgent(round, prior)
  if (review) reviewers.add(review.reviewer || REVIEW_FAMILY)
  const found = review ? (review.findings || []).map((f) => ({ ...f, reviewer: review.reviewer, round: round })) : []
  allFindings.push(...found)
  actionable = found.filter((f) => f.severity === 'blocker' || f.severity === 'major')
  minors = found.filter((f) => f.severity === 'minor')
  log('Re-review r' + round + ': ' + found.length + ' findings, ' + actionable.length + ' actionable')
}
if (actionable.length > 0) {
  log('Still ' + actionable.length + ' actionable findings after ' + fixes.length + ' rounds — escalate rather than looping again')
}

phase('Gate')
const gate = await agent(
  [
    COMMON,
    '',
    'Final gate for ' +
      TITLE +
      ': run the gates above for every module touched in `git diff --name-only ' +
      BASE +
      '...HEAD`. No stress or load runs. Do not modify code unless a gate fails for a trivial reason (formatting, an unused import); if you must, commit it.',
    trailers(IMPLEMENTER),
    'Return {check_quick, lint, tests, diff_stat, commits, status} as JSON text.',
  ].join('\n'),
  call({ label: 'gate:' + TAG, phase: 'Gate' })
)

return {
  ledger: {
    title: TITLE,
    size: A.size || 'fix-round',
    implementer: IMPLEMENTER,
    reviewers: [...reviewers],
    rounds: round,
    majors: allFindings.filter((f) => f.severity === 'blocker' || f.severity === 'major').length,
    findings: allFindings,
    remaining: actionable,
  },
  commits: fixes.filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
