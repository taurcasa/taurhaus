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

// Two runs of the same procedure on the same branch would otherwise write the same scratch files and
// poll the same EXIT marker. A workflow script cannot read the clock (the lint says why), so a
// concurrent run is told
// apart by args.stamp — any short token the caller passes.
const STAMP = A.stamp ? String(A.stamp).replace(/[^A-Za-z0-9._-]+/g, '-') : ''
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
  // A reviewer that withholds approval and files nothing the fix loop would act on has contradicted
  // itself: the loop has nothing to fix, so the run would complete green over a withheld approval.
  // That is the failure fail-closed exists to prevent, so the review is rejected as malformed.
  if (review.verdict === 'fix_required' && review.findings.filter((f) => f && f.severity !== 'nit').length === 0) {
    return label + ' returned fix_required with nothing to fix (' + review.findings.length + ' findings, none above a nit) — a withheld approval is not an approval'
  }
  return ''
}

// A gate is green only when it says pass AND every command it listed passed AND it ran something AND
// every required command is among them AND it contradicts itself nowhere. A skipped `just check-quick`
// is not a pass, and neither is a skipped `cargo test`: the run would otherwise complete green over a
// lane nobody executed. A command that did not apply is left off the list rather than reported
// `skipped`. Its own vocabulary is pass/fail, so it is checked here rather than through laneProblem.
function gateProblem(gate) {
  if (!gate) return 'the gate agent returned no result (it was skipped or died)'
  if (gate.error) return 'the gate could not run: ' + gate.error
  const ran = Array.isArray(gate.commands) ? gate.commands.filter(Boolean) : []
  if (ran.length === 0) return 'the gate reported no commands run'
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
  const failed = ran.filter((c) => c.status !== 'pass')
  if (failed.length > 0) return 'gate commands did not pass: ' + failed.map((c) => c.command + ' (' + c.status + ')').join(', ')
  const reported = Array.isArray(gate.failures) ? gate.failures.filter(Boolean) : []
  if (reported.length > 0) return 'the gate reported failures while every command it listed passed: ' + reported.join('; ')
  if (gate.status !== 'pass') return 'the gate reported status ' + JSON.stringify(gate.status)
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
      ? 'Co-Authored-By: Codex' + (CODEX_MODEL ? ' (' + CODEX_MODEL + ')' : '') + ' <noreply@openai.com>'
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
    ' Every command you list has to have passed: any entry that is not `pass` fails the run, required or not. A gate command that did not apply because nothing it covers changed is simply left off the list and explained in the summary — do not report it `skipped` and never report a command you did not run as `pass`. `failures` and `error` stay empty under a passing status; a `status` of pass next to either one is a contradiction and fails the run.',
  safety:
    'SAFETY: tests never read or write the real ~/.claude*, ~/.codex, ~/.gemini or ~/.grok and never invoke a real CLI; no load or stress runs; kill anything you start (trap/finally) and never kill a process you did not start; never print tokens or secrets.',
  readOnly:
    'READ-ONLY: change no file in any repository and run no git write command; write only under ' +
    SCRATCH +
    '. Report facts you verified (file:line, command output) and mark inferences UNVERIFIED with what would settle them.',
  scope:
    'SCOPE RULE: judge against the spec\'s minimum deliverable and its "not building" list — missing scaffolding (tests or docs for tooling, dry-run niceties, extra configurability) is at most a minor, and majors are reserved for defects a user would hit.',
  evidence:
    'Do NOT modify any file. Report only findings you verified with file:line evidence; severity blocker/major/minor/nit; verdict fix_required only for blocker/major. A fix_required must carry at least one finding above a nit — a withheld approval with nothing to fix is rejected as malformed and fails the run, so approve or file the finding.',
  honest:
    "HONESTY: set status='ok' only for work you actually did and saw succeed. If your lane could not run, return status='unavailable' with the error — never an invented result, an approval you did not reach, or a gate you did not watch pass. The caller fails the run closed on an unavailable lane, and that is the correct outcome.",
}

// The Codex lane: a thin Opus wrapper drives `codex exec` detached and polls for the EXIT marker,
// because one Bash call is capped at 10 minutes and Codex runs take longer. The command lives in a
// runner script so nothing is nested inside quotes, and every path is one single-quoted word.
// Ownership: the runner is its own process-group leader (setsid) and records that pid, so every
// give-up path kills the whole group — the runner, its `timeout` and codex itself. Killing the runner
// shell alone would leave an agent writing to the checkout while the retry started. Resumes name the
// session this run created rather than `--last`, which is whatever ran most recently on the machine.
function codexWrapper(o) {
  const base = SCRATCH + '/codex-' + o.tag + (STAMP ? '-' + STAMP : '')
  const out = base + (o.schema ? '.json' : '.out.md')
  const logFile = base + '.log'
  const runner = base + '.run.sh'
  const prompt = base + '.prompt.md'
  const pidFile = base + '.pid'
  const leaseFile = base + '.lease'
  const deadline = o.timeout + 300
  // The ownership lease: how long the runner keeps going without a heartbeat from the wrapper, and how
  // often its watchdog looks. Both are overridable so a test can prove the mechanism in seconds.
  const LEASE_TTL = 300
  const LEASE_POLL = 15
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
  // One runner shape for the first turn and for every resume: record the pid, take the ownership
  // lease, then run one command. The watchdog is what makes the ownership executable — an instruction
  // to the wrapper cannot run once the wrapper is gone, and an aborted lane used to leave this group
  // writing to the checkout until its own timeout expired.
  function runnerBody(command) {
    return [
      '#!/usr/bin/env bash',
      'set -u',
      'PIDFILE=' + sh(pidFile),
      'LOG=' + sh(logFile),
      'LEASE=' + sh(leaseFile),
      '# setsid makes this shell the process-group leader, so $$ is the pgid that kills the whole run.',
      'PGID=$$',
      'echo $$ > "$PIDFILE"',
      ': > "$LEASE"',
      'trap \'rm -f "$PIDFILE"\' EXIT',
      '# A TERM aimed at this shell alone still takes the group down with it.',
      'trap \'trap "" INT TERM; rm -f "$PIDFILE"; kill -TERM -"$PGID" 2>/dev/null; exit 143\' INT TERM',
      '# Ownership lease: the wrapper touches "$LEASE" while it polls. If it stops - the lane was',
      '# aborted, the session died - nobody is left to kill this run, so the watchdog kills the group.',
      'LEASE_TTL="${TAURHAUS_WORKFLOW_LEASE_TTL:-' + LEASE_TTL + '}"',
      'LEASE_POLL="${TAURHAUS_WORKFLOW_LEASE_POLL:-' + LEASE_POLL + '}"',
      '(',
      '  trap "" TERM',
      '  while [ -f "$PIDFILE" ]; do',
      '    sleep "$LEASE_POLL"',
      '    if [ -e "$LEASE" ]; then',
      '      NOW=$(date +%s 2>/dev/null) || continue',
      '      MTIME=$(stat -c %Y "$LEASE" 2>/dev/null || stat -f %m "$LEASE" 2>/dev/null) || continue',
      '      [ -n "$MTIME" ] || continue',
      '      [ "$((NOW - MTIME))" -lt "$LEASE_TTL" ] && continue',
      '    fi',
      '    echo "EXIT=98 ownership lease expired after ${LEASE_TTL}s (nothing refreshed $LEASE)" >> "$LOG"',
      '    kill -TERM -"$PGID" 2>/dev/null',
      '    sleep 5',
      '    kill -KILL -"$PGID" 2>/dev/null',
      '    exit 0',
      '  done',
      ') &',
      'cd ' + sh(ROOT) + ' || { echo "EXIT=97" >> "$LOG"; exit 97; }',
      command + ' >> "$LOG" 2>&1',
      'echo "EXIT=$?" >> "$LOG"',
    ].join('\n')
  }
  const killRun =
    'kill the whole group, not just the runner shell: `PGID=$(cat ' +
    sh(pidFile) +
    ' 2>/dev/null); if [ -n "$PGID" ]; then kill -TERM -"$PGID" 2>/dev/null; sleep 5; kill -KILL -"$PGID" 2>/dev/null; fi; rm -f ' +
    sh(pidFile) +
    ' ' +
    sh(leaseFile) +
    '`'
  const resumeCmd =
    'timeout ' +
    o.timeout +
    ' codex exec resume <SESSION_ID> --yolo --skip-git-repo-check' +
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
      ' — copy it byte for byte, the quoting is what makes a path with a space or an apostrophe work, the PIDFILE lines are what let you kill the run, and the LEASE watchdog is what kills it when you cannot:\n' +
      runnerBody(exec),
    '2) Launch it DETACHED, in its own process group: `rm -f ' +
      sh(out) +
      ' ' +
      sh(logFile) +
      ' ' +
      sh(pidFile) +
      ' ' +
      sh(leaseFile) +
      '; chmod +x ' +
      sh(runner) +
      '; setsid nohup bash ' +
      sh(runner) +
      ' >/dev/null 2>&1 < /dev/null & disown` — the runner writes its own pid to ' +
      sh(pidFile) +
      ', and because it was started with setsid that pid is the process-group id of everything it launches. You own that group until this lane returns, and the runner holds you to it: if ' +
      sh(leaseFile) +
      ' goes unrefreshed for ' +
      LEASE_TTL +
      ' seconds it kills its own group, so an abandoned lane cannot leave Codex writing to the checkout.',
    '3) Poll in Bash calls of at most 9 minutes each: `until grep -q "^EXIT=" ' +
      sh(logFile) +
      '; do touch ' +
      sh(leaseFile) +
      '; sleep 20; done` — the `touch` is your heartbeat on the lease from step 2: keep it in every wait loop and refresh it at least once a minute, or the runner will take itself down mid-run. Repeat the call until the marker appears; wait rather than abandoning a run that is still going. Bound the total wait at ' +
      deadline +
      ' seconds (the deadline): if the marker has not appeared by then, ' +
      killRun +
      ', and treat it as a failure in step 5.',
    '4) Read ' +
      sh(out) +
      ' and the tail of ' +
      sh(logFile) +
      ' — the `EXIT=` line is the exit code, and the log header names the model Codex actually ran and the session id of the session it created (`grep -iEm1 "session[ _-]?id" ' +
      sh(logFile) +
      '`).' +
      (o.resume
        ? ' If Codex left uncommitted work or an unfinished implementation (a run killed by the timeout counts), run up to THREE follow-up turns. Each turn is a fresh runner written exactly like the one above — the same PIDFILE, LEASE, watchdog, trap and `cd` lines — with this command in place of the exec, launched and polled (heartbeat included) the same way: `' +
          resumeCmd +
          '`. Substitute the session id you read in this step; use `--last` instead ONLY if the log names no id, and say so under deviations — `--last` resumes the newest session on the machine, which may be another run in this checkout rather than yours. `codex exec resume` does not accept -C, so the runner\'s `cd` into the checkout is what places it. Before each new turn, make sure the previous one is gone (' +
          killRun +
          '). Report every turn and its exit code under deviations, and verify the gate claims yourself (`cd src-tauri && cargo check --all-targets`) before returning.'
        : ''),
    '5) Return the result as your structured output' +
      (o.reviewer ? ", with reviewer='" + o.reviewer + "'" : '') +
      ", and model_used set to the model named in the log (or 'unknown'). " +
      RULES.honest +
      ' Concretely: a non-zero EXIT, a missing or empty output file, output that does not match the schema, or the step-3 deadline is a failure — ' +
      killRun +
      ' first, so nothing you launched outlives the attempt, then retry steps 2-4 once, and if it fails again return status=\'unavailable\' with the exit code and the last 20 log lines in error and no findings. Whatever the outcome, before you return: if ' +
      sh(pidFile) +
      ' still exists, the run is still running — ' +
      killRun +
      ' and never leave it orphaned. Kill only the group you started; never another process.',
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
  required: ['status', 'summary', 'commits', 'files_changed', 'tests_added', 'red_observed', 'gate', 'deviations'],
  properties: {
    status: { type: 'string', enum: ['ok', 'unavailable'], description: "'ok' only for work you did and saw succeed; 'unavailable' when this lane could not run" },
    error: { type: 'string', description: 'why the lane is unavailable: the exit code and the last log lines' },
    model_used: { type: 'string', description: 'the model that actually ran this lane' },

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
  required: ['status', 'findings', 'verdict', 'model_used'],
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
    verdict: { type: 'string', enum: ['approve', 'fix_required'], description: 'fix_required requires at least one finding above a nit; a fix_required with nothing to fix is rejected as malformed' },
    reviewer: { type: 'string' },
  },
}

const GATE_SCHEMA = {
  type: 'object',
  required: ['status', 'commands', 'diff_stat', 'commits'],
  properties: {
    status: { type: 'string', enum: ['pass', 'fail'], description: "'pass' only when every command you ran passed and `failures` and `error` are empty" },
    commands: {
      type: 'array',
      description: 'every gate command you ran, in order; a command that did not apply is left off the list',
      items: {
        type: 'object',
        required: ['command', 'status'],
        properties: {
          command: { type: 'string', description: 'the exact command line' },
          status: { type: 'string', enum: ['pass', 'fail', 'skipped'], description: 'anything but `pass` fails the run, required command or not' },
          detail: { type: 'string', description: 'the failure, or why it was skipped' },
        },
      },
    },
    failures: { type: 'array', items: { type: 'string' }, description: 'must be empty when status is pass' },
    diff_stat: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    error: { type: 'string' },
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
    return agent(
      reviewPrompt(round, prior) + "\nSet reviewer='opus conformance', status='ok' and model_used='" + MODELS.opus + "'. " + RULES.honest,
      call({ label: label, phase: 'Fix', schema: FINDINGS_SCHEMA })
    )
  }
  return agent(
    codexWrapper({
      tag: TAG + '-review-r' + round,
      task: reviewPrompt(round, prior) + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: CODEX_ID + ' conformance',
    }),
    call({ label: label, phase: 'Fix', schema: FINDINGS_SCHEMA })
  )
}

function fixAgent(findings, nits, round) {
  const task = [
    COMMON,
    '',
    'You are the fixer for ' +
      TITLE +
      ', round ' +
      round +
      '. Apply these confirmed findings — verify each against the code first and skip a wrong one with a stated reason. Every blocker and major gets a red-first regression test:',
    JSON.stringify(findings, null, 1),
    'Take the nits too where they are trivial: ' + JSON.stringify(nits, null, 1),
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
  return agent(
    task + "\nReturn the structured summary, with status='ok' and model_used='" + MODELS.opus + "'. " + RULES.honest,
    call({ label: 'fix:' + TAG + '-r' + round + ':opus', phase: 'Fix', schema: IMPL_SCHEMA })
  )
}

phase('Fix')
let round = START_ROUND
// Everything handed to this workflow is already an open finding another run could not close, so
// every severity but a nit is actionable here: the other procedures emit a minor as `remaining`
// whenever a reviewer returned fix_required, and filtering to blocker/major dropped it silently.
let actionable = OPEN.filter((f) => f.severity !== 'nit')
let nits = OPEN.filter((f) => f.severity === 'nit')
const allFindings = OPEN.slice()
const fixes = []
log('Starting at round ' + round + ' with ' + actionable.length + ' actionable findings and ' + nits.length + ' nits (max ' + MAX_ROUNDS + ' rounds)')
if (actionable.length === 0) log('Nothing to fix: every finding handed over is a nit — running the gate only')
while (actionable.length > 0 && fixes.length < MAX_ROUNDS) {
  const fixed = await fixAgent(actionable, nits, round)
  const fixProblem = laneProblem(fixed, 'the ' + IMPLEMENTER + ' fixer (round ' + round + ')')
  if (fixProblem) fail(fixProblem)
  fixes.push(fixed)
  const prior = JSON.stringify(actionable)
  round += 1
  const review = await reviewAgent(round, prior)
  // The re-review is the whole point of an extra round: an unavailable one fails the run.
  const problem = reviewProblem(review, 'the ' + REVIEW_FAMILY + ' re-review (round ' + round + ')')
  if (problem) fail(problem)
  reviewers.add((review.reviewer || REVIEW_FAMILY) + (review.model_used ? ' [' + review.model_used + ']' : ''))
  const found = review.findings.map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
  allFindings.push(...found)
  actionable = actionableFrom(found, [review.verdict])
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
    size: A.size || 'fix-round',
    implementer: IMPLEMENTER,
    models: MODELS,
    effort: A.effort || 'inherited',
    reviewers: [...reviewers],
    rounds: round,
    majors: allFindings.filter((f) => f.severity === 'blocker' || f.severity === 'major').length,
    findings: allFindings,
    remaining: actionable,
  },
  commits: fixes.filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
