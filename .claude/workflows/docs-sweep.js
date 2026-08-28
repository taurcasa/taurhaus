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

// One single-quoted shell word. Every path these scripts hand to a shell goes through it, because a
// WSL checkout is routinely `/mnt/c/Users/Jane Doe/…` and an apostrophe would close the quote.
function sh(value) {
  return "'" + String(value).replace(/'/g, "'\\''") + "'"
}

// A checkout can arrive as a Windows or a \\wsl$ UNC path; the agents work in WSL, so normalize to
// the POSIX form first (docs/architecture/path-handling-guide.md).
function posixPath(value, what) {
  const raw = String(value == null ? '' : value)
    .trim()
    .replace(/^["']|["']$/g, '')
  const unc = /^\\\\wsl(?:\$|\.localhost)\\[^\\]+((?:\\.*)?)$/.exec(raw)
  let out = (unc ? unc[1] || '\\' : raw).replace(/\\/g, '/')
  const drive = /^([A-Za-z]):\/(.*)$/.exec(out)
  if (drive) out = '/mnt/' + drive[1].toLowerCase() + '/' + drive[2]
  out = out.replace(/\/{2,}/g, '/').replace(/(.)\/+$/, '$1')
  if (!out.startsWith('/')) throw new Error(NAME + ': ' + what + ' must be an absolute path — got ' + JSON.stringify(value))
  return out
}

if (!A.worktree && !A.repo) throw new Error(NAME + ': args.worktree (or args.repo) is required — the absolute path of the checkout to work in')
const ROOT = posixPath(A.worktree || A.repo, 'args.worktree (or args.repo)')
const BRANCH = A.branch || ''
const BASE = A.base || 'main'
const SPEC = A.spec || ''
const SCRATCH = posixPath(A.scratch || '/tmp/taurhaus-workflows', 'args.scratch')
const MODEL = 'opus'
const GATES =
  A.gates ||
  "'just check-quick' and 'just lint', plus 'cd src-tauri && cargo test <touched module paths>' (check-quick does not run the Rust tests); vitest runs from the checkout root"

// The gate commands that must actually run and pass. Everything else the gate reports is optional and
// may come back `skipped` with a reason; a required command reported skipped — or never run at all —
// is a gate that did not happen. A spec naming different gates passes args.requiredGates ([] opts out).
if (A.requiredGates != null && !Array.isArray(A.requiredGates)) throw new Error(NAME + ': args.requiredGates must be an array of command substrings — got ' + JSON.stringify(A.requiredGates))
const REQUIRED_GATES = (A.requiredGates || ['just check-quick', 'just lint']).map(String)

// Every agent runs on Opus in this repo's model split; effort is inherited unless args.effort pins one.
const EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max']
if (A.effort && EFFORTS.indexOf(A.effort) === -1) throw new Error(NAME + ': args.effort must be one of ' + EFFORTS.join(', ') + ' — got ' + JSON.stringify(A.effort))
function call(o) {
  return A.effort ? { model: MODEL, effort: A.effort, ...o } : { model: MODEL, ...o }
}

// Codex takes its model as `-m` and its reasoning effort as `-c model_reasoning_effort` (CLAUDE.md,
// session_scanner/launch.rs). Without args.codexModel nothing is pinned, the CLI's own default runs,
// and the ledger says so rather than claiming a model nobody requested.
const CODEX_MODEL = A.codexModel ? String(A.codexModel) : ''
if (CODEX_MODEL && !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(CODEX_MODEL)) throw new Error(NAME + ': args.codexModel must be a bare model slug — got ' + JSON.stringify(A.codexModel))
const CODEX_FLAGS = (CODEX_MODEL ? ' -m ' + sh(CODEX_MODEL) : '') + (A.effort ? ' -c ' + sh('model_reasoning_effort="' + A.effort + '"') : '')
const CODEX_ID = 'codex ' + (CODEX_MODEL || 'cli default') + (A.effort ? ' at ' + A.effort : '')
const MODELS = { opus: MODEL + (A.effort ? ' at ' + A.effort : ''), codex: CODEX_ID }

// Fail closed: a run that cannot show a real cross-family review and a green gate is not a success.
function fail(why) {
  log('FAILED CLOSED — ' + why)
  throw new Error(NAME + ' failed closed: ' + why)
}

// A lane that returned nothing, or reported itself unavailable, never produced work or a review.
function laneProblem(result, label) {
  if (!result) return label + ' returned no result (the agent was skipped or died)'
  if (result.status && result.status !== 'ok') return label + ' is unavailable: ' + (result.error || 'no error reported')
  return ''
}

function reviewProblem(review, label) {
  const lane = laneProblem(review, label)
  if (lane) return lane
  if (!Array.isArray(review.findings)) return label + ' returned no findings array'
  if (review.verdict !== 'approve' && review.verdict !== 'fix_required') return label + ' returned an invalid verdict: ' + JSON.stringify(review.verdict)
  return ''
}

// A gate is green only when it says pass AND every command it ran passed AND it ran something AND
// every required command is among the ones that passed. A skipped `just check-quick` is not a pass:
// the run would otherwise complete green over a lane nobody executed. Its own vocabulary is pass/fail,
// so it is checked here rather than through laneProblem.
function gateProblem(gate) {
  if (!gate) return 'the gate agent returned no result (it was skipped or died)'
  if (gate.error && gate.status !== 'pass') return 'the gate could not run: ' + gate.error
  const ran = Array.isArray(gate.commands) ? gate.commands.filter(Boolean) : []
  if (ran.length === 0) return 'the gate reported no commands run'
  const failed = ran.filter((c) => c.status !== 'pass' && c.status !== 'skipped')
  if (failed.length > 0) return 'gate commands failed: ' + failed.map((c) => c.command + ' (' + c.status + ')').join(', ')
  const matches = (c, required) => String(c.command == null ? '' : c.command).indexOf(required) !== -1
  const missing = REQUIRED_GATES.filter((required) => !ran.some((c) => c.status === 'pass' && matches(c, required)))
  if (missing.length > 0) {
    return (
      'required gate commands did not run and pass: ' +
      missing
        .map((required) => {
          const seen = ran.filter((c) => matches(c, required))
          return required + ' (' + (seen.length > 0 ? seen.map((c) => c.status).join('/') : 'never run') + ')'
        })
        .join(', ')
    )
  }
  if (gate.status !== 'pass') return 'the gate reported status ' + JSON.stringify(gate.status) + (Array.isArray(gate.failures) && gate.failures.length > 0 ? ': ' + gate.failures.join('; ') : '')
  return ''
}

// A reviewer that demands a fix but files no blocker or major still blocks: take it at its word.
function actionableFrom(findings, verdicts) {
  const hard = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
  if (hard.length > 0) return hard
  return verdicts.indexOf('fix_required') === -1 ? [] : findings.filter((f) => f.severity !== 'nit')
}

function trailers(family) {
  const author =
    family === 'codex'
      ? 'Co-Authored-By: Codex (' + (CODEX_MODEL || 'gpt-5.6') + ') <noreply@openai.com>'
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
  gateResult:
    'Return the structured result: one entry in `commands` for every gate command you ran, each with its exact command line and its pass/fail, and `status` = pass only when every one of them passed.' +
    (REQUIRED_GATES.length > 0
      ? ' These are required and must actually run: ' +
        REQUIRED_GATES.join(', ') +
        ' — a required command reported `skipped` fails the run, so run it, or report it `fail` with the reason it could not run.'
      : '') +
    ' Mark a command `skipped` only when it is optional and did not apply, with the reason in `detail`.',
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
  honest:
    "HONESTY: set status='ok' only for work you actually did and saw succeed. If your lane could not run, return status='unavailable' with the error — never an invented result, an approval you did not reach, or a gate you did not watch pass. The caller fails the run closed on an unavailable lane, and that is the correct outcome.",
}

// The Codex lane: a thin Opus wrapper drives `codex exec` detached and polls for the EXIT marker,
// because one Bash call is capped at 10 minutes and Codex runs take longer. The command lives in a
// runner script so nothing is nested inside quotes, and every path is one single-quoted word.
function codexWrapper(o) {
  const base = SCRATCH + '/codex-' + o.tag
  const out = base + (o.schema ? '.json' : '.out.md')
  const logFile = base + '.log'
  const runner = base + '.run.sh'
  const prompt = base + '.prompt.md'
  const deadline = o.timeout + 300
  const exec =
    'timeout ' +
    o.timeout +
    ' codex exec --yolo --skip-git-repo-check' +
    CODEX_FLAGS +
    (o.schema ? ' --output-schema ' + sh(base + '.schema.json') : '') +
    ' -C ' +
    sh(ROOT) +
    ' -o ' +
    sh(out) +
    ' - < ' +
    sh(prompt)
  const runnerBody = [
    '#!/usr/bin/env bash',
    'set -u',
    'cd ' + sh(ROOT) + ' || { echo "EXIT=97" >> ' + sh(logFile) + '; exit 97; }',
    exec + ' >> ' + sh(logFile) + ' 2>&1',
    'echo "EXIT=$?" >> ' + sh(logFile),
  ].join('\n')
  const resumeCmd =
    'timeout ' +
    o.timeout +
    ' codex exec resume --last --yolo --skip-git-repo-check' +
    CODEX_FLAGS +
    ' -o ' +
    sh(base + '-r<N>.md') +
    ' ' +
    sh(
      'Continue from the current tree: commit any green step that is already complete, then proceed step by step, committing after each; run the gates; commit with the trailers.'
    )
  return [
    'You are a thin wrapper around the Codex CLI (' + CODEX_ID + '): it does the work, you do not. Do NOT do the task yourself.',
    '1) `mkdir -p ' +
      sh(SCRATCH) +
      '`; write the TASK below verbatim to ' +
      sh(prompt) +
      (o.schema ? ', this JSON Schema verbatim to ' + sh(base + '.schema.json') + ':\n' + JSON.stringify(o.schema) + '\n' : ', ') +
      'and this runner verbatim to ' +
      sh(runner) +
      ' — copy it byte for byte, the quoting is what makes a path with a space or an apostrophe work:\n' +
      runnerBody,
    '2) Launch it DETACHED: `rm -f ' +
      sh(out) +
      ' ' +
      sh(logFile) +
      '; chmod +x ' +
      sh(runner) +
      '; setsid nohup bash ' +
      sh(runner) +
      ' >/dev/null 2>&1 < /dev/null & disown`',
    '3) Poll in Bash calls of at most 9 minutes each: `until grep -q "^EXIT=" ' +
      sh(logFile) +
      '; do sleep 20; done` — repeat the call until the marker appears; wait rather than abandoning a run that is still going. Bound the total wait at ' +
      deadline +
      ' seconds (the deadline): if the marker has not appeared by then, kill the run (`pkill -f ' +
      sh(runner) +
      '`) and treat it as a failure in step 5.',
    '4) Read ' +
      sh(out) +
      ' and the tail of ' +
      sh(logFile) +
      ' — the `EXIT=` line is the exit code and the log header names the model Codex actually ran.' +
      (o.resume
        ? ' If Codex left uncommitted work or an unfinished implementation (a run killed by the timeout counts), run up to THREE follow-up turns, each through the same runner + poll pattern with the same flags: `' +
          resumeCmd +
          '` (`codex exec resume` does not accept -C, so the runner\'s `cd` into the checkout is what places it). Report every turn and its exit code under deviations, and verify the gate claims yourself (`cd src-tauri && cargo check --all-targets`) before returning.'
        : ''),
    '5) Return the result as your structured output' +
      (o.reviewer ? ", with reviewer='" + o.reviewer + "'" : '') +
      ", and model_used set to the model named in the log (or 'unknown'). " +
      RULES.honest +
      ' Concretely: a non-zero EXIT, a missing or empty output file, output that does not match the schema, or the step-3 deadline is a failure — retry steps 2-4 once, and if it fails again return status=\'unavailable\' with the exit code and the last 20 log lines in error and no findings.',
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
  required: ['status', 'summary', 'commits', 'files_changed', 'table_path', 'unresolved'],
  properties: {
    status: { type: 'string', enum: ['ok', 'unavailable'], description: "'ok' only for work you did and saw succeed; 'unavailable' when this lane could not run" },
    error: { type: 'string', description: 'why the lane is unavailable: the exit code and the last log lines' },
    model_used: { type: 'string', description: 'the model that actually ran this lane' },

    summary: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    files_changed: { type: 'array', items: { type: 'string' } },
    table_path: { type: 'string', description: 'the drift table: before -> after with file:line evidence' },
    unresolved: { type: 'array', items: { type: 'string' }, description: 'drift you could not settle from the code' },
  },
}

const FINDINGS_SCHEMA = {
  type: 'object',
  required: ['status', 'findings', 'verdict'],
  properties: {
    status: { type: 'string', enum: ['ok', 'unavailable'], description: "'ok' only for work you did and saw succeed; 'unavailable' when this lane could not run" },
    error: { type: 'string', description: 'why the lane is unavailable: the exit code and the last log lines' },
    model_used: { type: 'string', description: 'the model that actually ran this lane' },

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

const GATE_SCHEMA = {
  type: 'object',
  required: ['status', 'commands', 'diff_stat', 'commits'],
  properties: {
    status: { type: 'string', enum: ['pass', 'fail'], description: "'pass' only when every command you ran passed" },
    commands: {
      type: 'array',
      description: 'every gate command you ran, in order',
      items: {
        type: 'object',
        required: ['command', 'status'],
        properties: {
          command: { type: 'string', description: 'the exact command line' },
          status: { type: 'string', enum: ['pass', 'fail', 'skipped'] },
          detail: { type: 'string', description: 'the failure, or why it was skipped' },
        },
      },
    },
    failures: { type: 'array', items: { type: 'string' } },
    diff_stat: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    error: { type: 'string' },
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
    return agent(
      verifyPrompt(round, prior) + "\nSet reviewer='opus docs', status='ok' and model_used='" + MODELS.opus + "'. " + RULES.honest,
      call({ label: label, phase: 'Verify', schema: FINDINGS_SCHEMA })
    )
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
  return agent(
    task + "\nReturn the structured summary, with status='ok' and model_used='" + MODELS.opus + "'. " + RULES.honest,
    call({ label: label + ':opus', phase: groupPhase, schema: SWEEP_SCHEMA })
  )
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

const sweepProblem = laneProblem(sweep, 'the ' + SWEEPER + ' sweeper')
if (sweepProblem) fail(sweepProblem)

phase('Verify')
let round = 1
let review = await verifyAgent(round, null)
// The claim verification is what makes a sweep trustworthy: an unavailable one fails the run.
let problem = reviewProblem(review, 'the ' + VERIFY_FAMILY + ' verification (round ' + round + ')')
if (problem) fail(problem)
reviewers.add((review.reviewer || VERIFY_FAMILY) + (review.model_used ? ' [' + review.model_used + ']' : ''))
let findings = review.findings.map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
let actionable = actionableFrom(findings, [review.verdict])
const allFindings = findings.slice()
log('Verify r1 (' + VERIFY_FAMILY + '): ' + findings.length + ' findings, ' + actionable.length + ' actionable')

const fixes = []
while (actionable.length > 0 && fixes.length < MAX_ROUNDS) {
  const fixed = await sweepAgent(
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
  const fixProblem = laneProblem(fixed, 'the ' + SWEEPER + ' fixer (round ' + (fixes.length + 1) + ')')
  if (fixProblem) fail(fixProblem)
  fixes.push(fixed)
  const prior = JSON.stringify(actionable)
  round += 1
  review = await verifyAgent(round, prior)
  problem = reviewProblem(review, 'the ' + VERIFY_FAMILY + ' re-verification (round ' + round + ')')
  if (problem) fail(problem)
  reviewers.add((review.reviewer || VERIFY_FAMILY) + (review.model_used ? ' [' + review.model_used + ']' : ''))
  findings = review.findings.map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
  allFindings.push(...findings)
  actionable = actionableFrom(findings, [review.verdict])
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
    RULES.gateResult + ' ' + RULES.honest,
  ].join('\n'),
  call({ label: 'gate:' + TAG, phase: 'Gate', schema: GATE_SCHEMA })
)
// A failing gate fails the run: a completed ledger must never sit on top of a red test lane.
const gateFailure = gateProblem(gate)
if (gateFailure) fail(gateFailure)

return {
  ledger: {
    title: TITLE,
    size: A.size || 'docs',
    implementer: SWEEPER,
    models: MODELS,
    effort: A.effort || 'inherited',
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
