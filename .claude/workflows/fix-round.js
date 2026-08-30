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
const DEFAULT_GATES = ['just check-quick', 'just lint']
const RUST_TEST_GATE = 'just test-rust-unit'
const GATE_ARRAY_EXAMPLE = '["just check-quick", "just lint"]'

// Enough shell tokenization to distinguish cargo's options from its one positional test filter.
// The workflow never executes these tokens itself; it only rejects a command cargo cannot parse.
function commandWords(command) {
  const words = []
  let word = ''
  let quote = ''
  let escaped = false
  for (const char of command) {
    if (escaped) {
      word += char
      escaped = false
    } else if (char === '\\' && quote !== "'") {
      escaped = true
    } else if (quote) {
      if (char === quote) quote = ''
      else word += char
    } else if (char === "'" || char === '"') {
      quote = char
    } else if (/\s/.test(char)) {
      if (word) {
        words.push(word)
        word = ''
      }
    } else {
      word += char
    }
  }
  if (escaped) word += '\\'
  if (word) words.push(word)
  return words
}

const CARGO_OPTIONS_WITH_VALUES = [
  '--package',
  '-p',
  '--exclude',
  '--jobs',
  '-j',
  '--profile',
  '--features',
  '-F',
  '--target',
  '--target-dir',
  '--manifest-path',
  '--lockfile-path',
  '--message-format',
  '--color',
  '--config',
  '-Z',
  '--bin',
  '--example',
  '--test',
  '--bench',
]

function cargoTestIndices(words) {
  const indices = []
  for (let i = 0; i < words.length - 1; i += 1) {
    if (words[i] === 'cargo' && words[i + 1] === 'test') indices.push(i + 1)
    if (words[i] === 'cargo' && /^\+\S+/.test(words[i + 1] || '') && words[i + 2] === 'test') indices.push(i + 2)
  }
  return indices
}

function validateGateCommand(command, what) {
  const words = commandWords(command)
  if (words[0] === 'just' && (!words[1] || words[1].startsWith('-') || !/^[A-Za-z0-9][A-Za-z0-9_.:-]*$/.test(words[1]))) {
    throw new Error(NAME + ': ' + what + ' must use the shape `just <recipe>` — got ' + JSON.stringify(command))
  }
  for (const test of cargoTestIndices(words)) {
    let filters = 0
    for (let i = test + 1; i < words.length; i += 1) {
      const word = words[i]
      if (word === '--' || word === '&&' || word === '||' || word === '|' || word === ';') break
      if (word.includes('>') || word.includes('<')) break
      if (word.startsWith('-')) {
        if (CARGO_OPTIONS_WITH_VALUES.indexOf(word) !== -1) i += 1
        continue
      }
      filters += 1
    }
    if (filters > 1) {
      throw new Error(NAME + ': ' + what + ' cargo test command carries ' + filters + ' positional filters before `--`; it allows at most one positional filter — got ' + JSON.stringify(command))
    }
  }
}

function typedGateCommands(value, what, allowString) {
  if (value == null) return []
  if (typeof value === 'string') {
    const words = commandWords(value)
    const hasConnector = words.some((word, index) => ['&&', '||', '|', 'and', 'plus', 'then'].indexOf(word) !== -1 && index > 0 && index < words.length - 1)
    if (!allowString || value.includes(';') || /\[[^\]\n]*\]/.test(value) || /['"]/.test(value) || hasConnector) {
      throw new Error(
        NAME +
          ': ' +
          what +
          ' must be an array of exact command strings, for example ' +
          GATE_ARRAY_EXAMPLE +
          '; move operational prose to args.gateNotes — got ' +
          JSON.stringify(value)
      )
    }
    value = [value]
  }
  if (!Array.isArray(value)) {
    throw new Error(NAME + ': ' + what + ' must be an array of exact command strings, for example ' + GATE_ARRAY_EXAMPLE + ' — got ' + JSON.stringify(value))
  }
  return value.map((entry, index) => {
    if (typeof entry !== 'string') throw new Error(NAME + ': ' + what + '[' + index + '] must be an exact command string — got ' + JSON.stringify(entry))
    if (/\r|\n/.test(entry)) throw new Error(NAME + ': ' + what + '[' + index + '] must not contain a newline — got ' + JSON.stringify(entry))
    if (/\[[^\]\n]*\]/.test(entry)) throw new Error(NAME + ': ' + what + '[' + index + '] must not contain bracketed prose; move operational prose to args.gateNotes — got ' + JSON.stringify(entry))
    const command = entry.trim()
    if (!command) throw new Error(NAME + ': ' + what + '[' + index + '] must be non-empty')
    validateGateCommand(command, what + '[' + index + ']')
    return command
  })
}

function uniqueGates(commands) {
  return commands.filter((command, index) => commands.indexOf(command) === index)
}

// One typed catalog owns both declarations and requirements. requiredGates remains additive, and
// adding one declares it too; the two defaults cannot be opted out of.
function buildGateCatalog(gates, requiredGates) {
  const requested = typedGateCommands(gates, 'args.gates', true)
  const required = typedGateCommands(requiredGates, 'args.requiredGates', false)
  return {
    declared: uniqueGates(DEFAULT_GATES.concat(requested, required)),
    required: uniqueGates(DEFAULT_GATES.concat(required)),
  }
}

const GATE_CATALOG = buildGateCatalog(A.gates, A.requiredGates)
const GATES = GATE_CATALOG.declared
const REQUIRED_GATES = GATE_CATALOG.required
if (A.gateNotes != null && typeof A.gateNotes !== 'string') throw new Error(NAME + ': args.gateNotes must be a string — got ' + JSON.stringify(A.gateNotes))
const GATE_NOTES = A.gateNotes || ''

function isRustTestGate(command) {
  const words = commandWords(command)
  if (cargoTestIndices(words).length > 0) return true
  return words.some((word, index) => word === 'just' && /^test-rust(?:-|$)/.test(words[index + 1] || ''))
}

function normalizedChangedPaths(paths) {
  return Array.isArray(paths) ? paths.map((path) => String(path).trim()).filter(Boolean) : []
}

function isRustPath(path) {
  return /(^|\/)src-tauri\//.test(String(path))
}

function laneChangedPaths(lanes) {
  return lanes.reduce((paths, lane) => paths.concat(normalizedChangedPaths(lane && lane.files_changed)), [])
}

function effectiveGateCatalog(changedPaths, independentPaths) {
  const rustChanged = normalizedChangedPaths(changedPaths)
    .concat(normalizedChangedPaths(independentPaths))
    .some(isRustPath)
  if (!rustChanged) return GATE_CATALOG
  const declaredRustGates = GATES.filter(isRustTestGate)
  if (declaredRustGates.length > 0) {
    return {
      declared: GATES,
      required: uniqueGates(REQUIRED_GATES.concat(declaredRustGates)),
    }
  }
  return {
    declared: GATES.concat([RUST_TEST_GATE]),
    required: REQUIRED_GATES.concat([RUST_TEST_GATE]),
  }
}

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

// Two runs of the same procedure would otherwise write the same scratch files, poll the same EXIT
// marker and — the part that kills — claim the same pidfile. The scratch dir is shared across
// checkouts, and `tag` defaults to the branch, so two worktrees on one branch (the normal shape of
// parallel agent work) collide unless the checkout is part of the name. A workflow script cannot read
// the clock (the lint says why), so two deliberate runs of one procedure in one checkout are told
// apart by args.stamp — any short token the caller passes.
const CHECKOUT = ROOT.slice(ROOT.lastIndexOf('/') + 1).replace(/[^A-Za-z0-9._-]+/g, '-') || 'checkout'
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

// The reviewer's verdict contract, stated to every reviewer and enforced on every result: a
// `fix_required` means at least one blocker or major. A fix_required carrying nothing above a minor is
// a withheld approval the fix loop cannot act on — it either runs a fix round over trivia or runs none
// at all and lets a green gate complete the run over a review that never approved. So it is malformed,
// not a verdict: re-requested once, and then it fails the run.
const REVIEW_CONTRACT =
  'VERDICT CONTRACT: return `fix_required` only when you filed at least one blocker or major — a fix_required carrying nothing above a minor is malformed and is rejected, not read as an approval. Minors and nits are welcome under `approve`: they ride along to the fixer as trivia and come back in the ledger. If a minor is worth blocking the change on, raise it to a major and say why.'

function contractBreach(review) {
  if (!review || review.verdict !== 'fix_required' || !Array.isArray(review.findings)) return false
  return review.findings.filter((f) => f && (f.severity === 'blocker' || f.severity === 'major')).length === 0
}

function reviewProblem(review, label) {
  const lane = laneProblem(review, label)
  if (lane) return lane
  if (!Array.isArray(review.findings)) return label + ' returned no findings array'
  if (review.verdict !== 'approve' && review.verdict !== 'fix_required') return label + ' returned an invalid verdict: ' + JSON.stringify(review.verdict)
  if (contractBreach(review)) {
    return (
      label +
      ' returned fix_required with no blocker or major (' +
      review.findings.length +
      ' findings) — the contract is that fix_required carries at least one, so this is a withheld approval the fix loop cannot act on'
    )
  }
  return ''
}

// One re-request, then the run fails. A reviewer that withheld approval over a minor usually meant to
// approve, and a real blocker it forgot to file is worth one more call — but only one. `request(note)`
// re-runs the same lane with the note appended; nothing here throws, because a review lane can run
// inside parallel(), where a throw is indistinguishable from a dead agent. The caller validates.
async function reviewOnce(request, label) {
  const first = await request('')
  if (!contractBreach(first)) return first
  const breach = reviewProblem(first, label)
  log('Re-requesting ' + label + ' with the verdict contract restated — ' + breach)
  return await request(
    'RE-REQUEST — your previous review of this same diff was rejected: ' +
      breach +
      '. ' +
      REVIEW_CONTRACT +
      ' Review the same diff again and return a well-formed result: approve, keeping those minors and nits as findings, or file the blocker or major that justifies fix_required. This is the only re-request — a second malformed review fails the run.'
  )
}

// A gate is green only when it says pass, every exact command it listed was declared and passed, every
// required command is among them, and it contradicts itself nowhere. Its first action supplies the
// diff paths that add the Rust test gate when needed.
function gateProblem(gate, independentPaths) {
  if (!gate) return 'the gate agent returned no result (it was skipped or died)'
  if (gate.error) return 'the gate could not run: ' + gate.error
  const ran = Array.isArray(gate.commands) ? gate.commands.filter(Boolean) : []
  if (ran.length === 0) return 'the gate reported no commands run'
  const gatePaths = normalizedChangedPaths(gate.changed_paths)
  const independentRustPaths = normalizedChangedPaths(independentPaths).filter(isRustPath)
  if (independentRustPaths.length > 0 && !gatePaths.some(isRustPath)) {
    return 'the gate did not report the diff it was asked to run; another lane reported Rust paths: ' + independentRustPaths.join(', ')
  }
  const catalog = effectiveGateCatalog(gatePaths, independentPaths)
  const commandOf = (entry) => String(entry.command == null ? '' : entry.command).trim()
  const missing = catalog.required.filter((required) => !ran.some((entry) => entry.status === 'pass' && commandOf(entry) === required))
  if (missing.length > 0) {
    return (
      'required gate commands did not run and pass: ' +
      missing
        .map((required) => {
          const seen = ran.filter((entry) => commandOf(entry) === required)
          return required + ' (' + (seen.length > 0 ? seen.map((entry) => entry.status).join('/') : 'never run') + ')'
        })
        .join(', ')
    )
  }
  const undeclared = ran.filter((entry) => catalog.declared.indexOf(commandOf(entry)) === -1)
  if (undeclared.length > 0) return 'gate commands were not declared: ' + undeclared.map(commandOf).join(', ')
  const failed = ran.filter((c) => c.status !== 'pass')
  if (failed.length > 0) return 'gate commands did not pass: ' + failed.map((c) => c.command + ' (' + c.status + ')').join(', ')
  const reported = Array.isArray(gate.failures) ? gate.failures.filter(Boolean) : []
  if (reported.length > 0) return 'the gate reported failures while every command it listed passed: ' + reported.join('; ')
  if (gate.status !== 'pass') return 'the gate reported status ' + JSON.stringify(gate.status)
  return ''
}

// What another fix round is for: the hard findings. The verdict no longer widens this set — the
// contract makes every fix_required carry a blocker or major, and one filed under `approve` is taken
// at its word anyway. Everything below rides along to the fixer as trivia and comes back in the
// ledger's `remaining` rather than looping.
function actionableFrom(findings) {
  return findings.filter((f) => f && (f.severity === 'blocker' || f.severity === 'major'))
}

function trivialFrom(findings) {
  return findings.filter((f) => f && f.severity !== 'blocker' && f.severity !== 'major')
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
  gates:
    'GATES (exact commands; run from the checkout root):\n' +
    GATES.map((command) => '- ' + command).join('\n') +
    '\nRUST DIFF RULE: if your diff touches `src-tauri/`, also run `just test-rust-unit` — `just check-quick` compiles the Rust tests but does not execute them.',
  gateNotes: GATE_NOTES ? 'GATE NOTES (operational instructions, not commands):\n' + GATE_NOTES : '',
  gateResult: (base) =>
    'As your first step, run `git diff --name-only ' +
    base +
    '...HEAD` and return its output lines verbatim in `changed_paths`. If any path begins `src-tauri/`, append and run the exact required command `' +
    RUST_TEST_GATE +
    '` unless the declared gates already contain a `cargo test` or `just test-rust-*` command; when they do, run every declared Rust-test command as the required Rust run. Return one entry in `commands` for every effective declared gate command you ran, with the exact command after trimming and its pass/fail; do not report discovery commands or any command outside that catalog. These commands are always required and must actually run: ' +
    REQUIRED_GATES.join(', ') +
    '. Before running a `just <recipe>` gate, use `just --summary` to confirm its recipe exists; do not list that discovery query as a gate command, and report the declared gate as fail with `unknown recipe` when absent. A required command reported `skipped` fails the run, so run it or report it `fail` with the reason it could not run. Set `status` = pass only when every command passed. A gate command that did not apply is left off the list and explained in the summary — never report it `skipped` or report a command you did not run as `pass`. `failures` and `error` stay empty under a passing status; pass next to either one is a contradiction and fails the run.',
  safety:
    'SAFETY: tests never read or write the real ~/.claude*, ~/.codex, ~/.gemini or ~/.grok and never invoke a real CLI; no load or stress runs; kill anything you start (trap/finally) and never kill a process you did not start; never print tokens or secrets.',
  readOnly:
    'READ-ONLY: change no file in any repository and run no git write command; write only under ' +
    SCRATCH +
    '. Report facts you verified (file:line, command output) and mark inferences UNVERIFIED with what would settle them.',
  scope:
    'SCOPE RULE: judge against the spec\'s minimum deliverable and its "not building" list — missing scaffolding (tests or docs for tooling, dry-run niceties, extra configurability) is at most a minor, and majors are reserved for defects a user would hit.',
  evidence:
    'Do NOT modify any file. Report only findings you verified with file:line evidence; severity blocker/major/minor/nit. ' + REVIEW_CONTRACT,
  honest:
    "HONESTY: set status='ok' only for work you actually did and saw succeed. If your lane could not run, return status='unavailable' with the error — never an invented result, an approval you did not reach, or a gate you did not watch pass. The caller fails the run closed on an unavailable lane, and that is the correct outcome.",
}

// The Codex lane: a thin Opus wrapper drives `codex exec` detached and polls for the EXIT marker,
// because one Bash call is capped at 10 minutes and Codex runs take longer. The command lives in a
// runner script so nothing is nested inside quotes, and every path is one single-quoted word.
// Ownership: the runner is its own process-group leader (setsid) and records that pid, so every
// give-up path kills the whole group — the runner, its `timeout` and codex itself. Killing the runner
// shell alone would leave an agent writing to the checkout while the retry started. Every artifact is
// named for the checkout, the tag and the stamp so two runs cannot claim one pidfile; the launch
// refuses over a live owner rather than overwriting it, and every kill first proves the pid still
// names this runner — a stale pidfile points at a recycled pid, and killing that is killing a
// stranger. Resumes name the session this run created rather than `--last`, which is whatever ran
// most recently on the machine.
function codexWrapper(o) {
  const base = SCRATCH + '/codex-' + CHECKOUT + '-' + o.tag + (STAMP ? '-' + STAMP : '')
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
  // Never kill a pid this run cannot prove is its own: a pidfile outlives a crashed runner, and the
  // pid it names can already belong to something else. `ps` is the fallback where /proc is not mounted.
  const ownsPid = (name) =>
    '{ tr "\\0" " " < /proc/"$' + name + '"/cmdline 2>/dev/null || ps -o args= -p "$' + name + '" 2>/dev/null; } | grep -qF ' + sh(runner)
  const killRun =
    'kill the whole group, not just the runner shell, and only while that pid is still the run you started: `PGID=$(cat ' +
    sh(pidFile) +
    ' 2>/dev/null); if [ -n "$PGID" ] && ' +
    ownsPid('PGID') +
    '; then kill -TERM -"$PGID" 2>/dev/null; sleep 5; kill -KILL -"$PGID" 2>/dev/null; fi; rm -f ' +
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
    '2) FIRST make sure no live run already owns these files: `OWNER=$(cat ' +
      sh(pidFile) +
      ' 2>/dev/null); if [ -n "$OWNER" ] && kill -0 "$OWNER" 2>/dev/null && ' +
      ownsPid('OWNER') +
      '; then echo "BUSY $OWNER"; fi` — if that prints BUSY, another run of this procedure is live in this checkout. Do NOT launch, and do NOT remove or overwrite ' +
      sh(pidFile) +
      ': it belongs to that run, and overwriting it would point its kill paths at whatever the pid becomes. Return status=\'unavailable\' immediately, with the pid and the pidfile path in error (two runs of one procedure in one checkout are unsupported — they would fight over the git index anyway). Otherwise launch it DETACHED, in its own process group: `rm -f ' +
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
  additionalProperties: false,
  required: ['status', 'error', 'model_used', 'summary', 'commits', 'files_changed', 'tests_added', 'red_observed', 'gate', 'deviations'],
  properties: {
    status: { type: 'string', enum: ['ok', 'unavailable'], description: "'ok' only for work you did and saw succeed; 'unavailable' when this lane could not run" },
    error: { type: 'string', description: 'why the lane is unavailable: the exit code and the last log lines' },
    model_used: { type: 'string', description: 'the model that actually ran this lane' },

    summary: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    files_changed: { type: 'array', items: { type: 'string' }, description: 'repo-relative paths, exactly as git reports them' },
    tests_added: { type: 'array', items: { type: 'string' } },
    red_observed: { type: 'string', description: 'which tests failed before the fix and how' },
    gate: { type: 'string', description: 'the gate commands run and their outcome' },
    deviations: { type: 'array', items: { type: 'string' }, description: 'findings skipped as wrong, with the reason' },
  },
}

const FINDINGS_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['status', 'error', 'model_used', 'findings', 'verdict', 'reviewer'],
  properties: {
    status: { type: 'string', enum: ['ok', 'unavailable'], description: "'ok' only for work you did and saw succeed; 'unavailable' when this lane could not run" },
    error: { type: 'string', description: 'why the lane is unavailable: the exit code and the last log lines' },
    model_used: { type: 'string', description: 'the model that actually ran this lane' },

    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
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
    verdict: { type: 'string', enum: ['approve', 'fix_required'], description: 'fix_required requires at least one blocker or major; one carrying nothing above a minor is rejected as malformed' },
    reviewer: { type: 'string' },
  },
}

const GATE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['status', 'changed_paths', 'commands', 'failures', 'diff_stat', 'commits', 'error'],
  properties: {
    status: { type: 'string', enum: ['pass', 'fail'], description: "'pass' only when every command you ran passed and `failures` and `error` are empty" },
    changed_paths: { type: 'array', items: { type: 'string' }, description: 'verbatim lines from the required git diff --name-only command' },
    commands: {
      type: 'array',
      description: 'every effective declared gate command you ran, in order; commands outside the catalog are rejected',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['command', 'status', 'detail'],
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
    'Lens: conformance and correctness — does the change implement the spec item completely; are the tests genuinely red-before/green-after (inspect them, run them); edge cases and backward compatibility; anything missing or out of scope. Does the change re-derive a rule another layer owns (frontend vs backend, app vs daemon), or add a view that bypasses the existing authority? Name the authority and cite the duplicate.',
    'This is re-review round ' +
      round +
      '. Prior findings (JSON): ' +
      prior +
      '. First verify each prior finding is resolved, with file:line evidence, then look for regressions introduced by the fix.',
    RULES.evidence,
  ].join('\n')
}

// `note` is the one contract re-request: a lane that came back malformed is re-run with the contract
// restated, under its own label and its own scratch tag so the run tree shows both attempts.
function reviewAgent(round, prior, note) {
  const again = note ? '-recontract' : ''
  const label = 'review:' + REVIEW_FAMILY + '-r' + round + again
  const task = reviewPrompt(round, prior) + (note ? '\n' + note : '')
  if (REVIEW_FAMILY === 'opus') {
    return agent(
      task + "\nSet reviewer='opus conformance', status='ok' and model_used='" + MODELS.opus + "'. " + RULES.honest,
      call({ label: label, phase: 'Fix', schema: FINDINGS_SCHEMA })
    )
  }
  return agent(
    codexWrapper({
      tag: TAG + '-review-r' + round + again,
      task: task + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: CODEX_ID + ' conformance',
    }),
    call({ label: label, phase: 'Fix', schema: FINDINGS_SCHEMA })
  )
}

function reviewLane(round, prior, label) {
  return reviewOnce((note) => reviewAgent(round, prior, note), label)
}

function fixAgent(findings, trivial, round) {
  const task = [
    COMMON,
    '',
    'You are the fixer for ' +
      TITLE +
      ', round ' +
      round +
      '. Apply these confirmed findings — verify each against the code first and skip a wrong one with a stated reason. Every blocker and major gets a red-first regression test:',
    JSON.stringify(findings, null, 1),
    'Take the minors and nits too where they are trivial: ' + JSON.stringify(trivial, null, 1),
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
let trivial = OPEN.filter((f) => f.severity === 'nit')
const allFindings = OPEN.slice()
const fixes = []
log('Starting at round ' + round + ' with ' + actionable.length + ' actionable findings and ' + trivial.length + ' nits (max ' + MAX_ROUNDS + ' rounds)')
if (actionable.length === 0) log('Nothing to fix: every finding handed over is a nit — running the gate only')
while (actionable.length > 0 && fixes.length < MAX_ROUNDS) {
  const fixed = await fixAgent(actionable, trivial, round)
  const fixProblem = laneProblem(fixed, 'the ' + IMPLEMENTER + ' fixer (round ' + round + ')')
  if (fixProblem) fail(fixProblem)
  fixes.push(fixed)
  const prior = JSON.stringify(actionable)
  round += 1
  const label = 'the ' + REVIEW_FAMILY + ' re-review (round ' + round + ')'
  const review = await reviewLane(round, prior, label)
  // The re-review is the whole point of an extra round: an unavailable or malformed one fails the run.
  const problem = reviewProblem(review, label)
  if (problem) fail(problem)
  reviewers.add((review.reviewer || REVIEW_FAMILY) + (review.model_used ? ' [' + review.model_used + ']' : ''))
  const found = review.findings.map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
  allFindings.push(...found)
  // The handover contract above is wider than a review's: everything another run left open, nits
  // aside, is fixed here. A finding this run's own re-review files follows the review contract.
  actionable = actionableFrom(found)
  trivial = trivialFrom(found)
  log('Re-review r' + round + ': ' + found.length + ' findings, ' + actionable.length + ' actionable')
}
if (actionable.length > 0) {
  log('Still ' + actionable.length + ' actionable findings after ' + fixes.length + ' rounds — escalate rather than looping again')
}

phase('Gate')
const gate = await agent(
  [
    COMMON,
    RULES.gateNotes,
    '',
    'Final gate for ' +
      TITLE +
      ': run the exact gates above, applying the Rust-diff rule below. No stress or load runs. Do not modify code unless a gate fails for a trivial reason (formatting, an unused import); if you must, commit it.',
    trailers(IMPLEMENTER),
    RULES.gateResult(BASE) + ' ' + RULES.honest,
  ].join('\n'),
  call({ label: 'gate:' + TAG, phase: 'Gate', schema: GATE_SCHEMA })
)
// A failing gate fails the run: a completed ledger must never sit on top of a red test lane.
const gateFailure = gateProblem(gate, laneChangedPaths(fixes))
if (gateFailure) fail(gateFailure)

const remaining = actionable.concat(trivial)
const outcome = actionableFrom(remaining).length > 0 ? 'followup_required' : 'complete'
return {
  outcome: outcome,
  ...(outcome === 'followup_required'
    ? { followup: { name: 'fix-round', args: { worktree: ROOT, branch: BRANCH, base: BASE, spec: SPEC, title: TITLE, findings: remaining, startRound: round + 1 } } }
    : {}),
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
    // What this run could not close: the hard findings it ran out of rounds for, plus the trivia
    // nobody picked up. Both come straight back as `findings` for another fix-round.
    remaining: remaining,
  },
  commits: fixes.filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
