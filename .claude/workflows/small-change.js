export const meta = {
  name: 'small-change',
  description: 'One small change: implement, one cross-family review lens, at most one fix round, gate',
  phases: [
    { title: 'Implement', detail: 'the implementer works the spec red-first and commits per green step', model: 'opus' },
    { title: 'Review', detail: 'the other family reviews once; one fix round if it finds a blocker or major', model: 'opus' },
    { title: 'Gate', detail: 'check-quick, lint, targeted tests', model: 'opus' },
  ],
}

const NAME = 'small-change'

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
const SIZE = A.size || 'small'
const TITLE = A.title || SPEC || BRANCH || NAME
const TAG = (A.tag || BRANCH || NAME).replace(/[^A-Za-z0-9._-]+/g, '-')
const DIFF = 'git diff ' + BASE + '...HEAD'

const COMMON = [RULES.checkout, RULES.spec, RULES.gates, RULES.tdd, RULES.commits, RULES.safety].filter(Boolean).join('\n')

const IMPL_SCHEMA = {
  type: 'object',
  required: ['summary', 'commits', 'files_changed', 'tests_added', 'red_observed', 'gate', 'deviations'],
  properties: {
    summary: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    files_changed: { type: 'array', items: { type: 'string' } },
    tests_added: { type: 'array', items: { type: 'string' } },
    red_observed: { type: 'string', description: 'which tests failed before the change and how' },
    gate: { type: 'string', description: 'the gate commands run and their outcome' },
    deviations: { type: 'array', items: { type: 'string' } },
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

// One lens for a small change: correctness and the operational checklist in a single pass.
const LENS =
  'Lens: correctness and operational failure modes in one pass — does the change do what the spec asks and nothing beyond it; are the tests honest (they fail without the change and touch only tempdirs, never the real ~/.claude*, ~/.codex, ~/.gemini or ~/.grok); does data written by the previous release still load; does a change to the daemon wire vocabulary bump PROTOCOL_VERSION; Windows/WSL path handling; user-config files edited only through tempfile+rename with ownership and permissions preserved; concurrency, unbounded retries and processes left running; hygiene (log spam, dead code, CLAUDE.md violations); and for scripts, that they parse and match the API they call.'

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
    LENS,
    prior
      ? 'This is re-review round ' +
        round +
        '. Prior findings (JSON): ' +
        prior +
        '. First verify each prior finding is resolved, with file:line evidence, then look for regressions introduced by the fix.'
      : '',
    RULES.evidence,
  ]
    .filter(Boolean)
    .join('\n')
}

function reviewAgent(round, prior) {
  const label = 'review:' + REVIEW_FAMILY + '-r' + round
  if (REVIEW_FAMILY === 'opus') {
    return agent(reviewPrompt(round, prior) + "\nSet reviewer='opus'.", call({ label: label, phase: 'Review', schema: FINDINGS_SCHEMA }))
  }
  return agent(
    codexWrapper({
      tag: TAG + '-review-r' + round,
      task: reviewPrompt(round, prior) + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: 'codex gpt-5.6',
    }),
    call({ label: label, phase: 'Review', schema: FINDINGS_SCHEMA })
  )
}

function flatten(review, round) {
  if (!review) return []
  reviewers.add(review.reviewer || REVIEW_FAMILY)
  return (review.findings || []).map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
}

function implementer(task, tag, label, resume, groupPhase) {
  if (IMPLEMENTER === 'codex') {
    return agent(
      codexWrapper({
        tag: tag,
        task:
          task +
          '\nWhen completely done, print a final JSON object with keys summary, commits (`git log --oneline ' +
          BASE +
          '..HEAD`), files_changed, tests_added, red_observed, gate, deviations.',
        schema: IMPL_SCHEMA,
        timeout: 3000,
        resume: resume,
      }),
      call({ label: label + ':codex', phase: groupPhase, schema: IMPL_SCHEMA })
    )
  }
  return agent(task + '\nReturn the structured summary.', call({ label: label + ':opus', phase: groupPhase, schema: IMPL_SCHEMA }))
}

phase('Implement')
const impl = await implementer(
  [
    COMMON,
    '',
    'You implement ' +
      TITLE +
      ' (size: ' +
      SIZE +
      '). Follow the spec red-first, one commit per numbered item, and run the gates it lists.' +
      (A.notes ? ' ' + A.notes : ''),
    trailers(IMPLEMENTER),
  ].join('\n'),
  TAG + '-impl',
  'impl:' + TAG,
  true,
  'Implement'
)

phase('Review')
let round = 1
let review = await reviewAgent(round, null)
let findings = flatten(review, round)
let actionable = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
const allFindings = findings.slice()
log('Review r1 (' + REVIEW_FAMILY + '): ' + findings.length + ' findings, ' + actionable.length + ' actionable')

// A small change gets exactly one fix round; anything still open goes to the fix-round workflow.
let fix = null
if (actionable.length > 0) {
  fix = await implementer(
    [
      COMMON,
      '',
      'Fix these verified findings red-first — check each against the code and skip a wrong one with a stated reason:',
      JSON.stringify(findings, null, 1),
      'Keep the change local to the files named. Re-run the gates.',
      trailers(IMPLEMENTER),
    ].join('\n'),
    TAG + '-fix',
    'fix:' + TAG,
    false,
    'Review'
  )
  round += 1
  review = await reviewAgent(round, JSON.stringify(actionable))
  findings = flatten(review, round)
  allFindings.push(...findings)
  actionable = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
  log('Re-review r2: ' + findings.length + ' findings, ' + actionable.length + ' actionable')
}
if (actionable.length > 0) {
  log('Stopped short: ' + actionable.length + ' actionable findings left after one fix round — hand them to the fix-round workflow')
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
    size: SIZE,
    implementer: IMPLEMENTER,
    reviewers: [...reviewers],
    rounds: round,
    majors: allFindings.filter((f) => f.severity === 'blocker' || f.severity === 'major').length,
    findings: allFindings,
    remaining: actionable,
  },
  commits: [impl, fix].filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
