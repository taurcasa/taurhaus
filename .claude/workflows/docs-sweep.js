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
  if (result.status === 'blocked') return label + ' is blocked: ' + (result.reason || 'no reason reported')
  if (result.status === 'timeout') return label + ' timed out'
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
  // A lane's Rust path may legitimately be gone from the final diff (a reverted file); what cannot
  // be legitimate is a gate that reports no diff at all while a lane changed Rust. The Rust rule
  // itself still takes the union, so the reverted case costs one extra test run, never a false green.
  if (independentRustPaths.length > 0 && gatePaths.length === 0) {
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
  // A command the gate ran beyond the catalog is extra evidence, not a breach — as long as it
  // passed; a failing one is caught below like any other.
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

const STAGE_BLOCKED_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['status', 'reason'],
  properties: {
    status: { type: 'string', enum: ['blocked'] },
    reason: { type: 'string' },
  },
}

const STAGE_TIMEOUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['status'],
  properties: {
    status: { type: 'string', enum: ['timeout'] },
  },
}

const STAGE_UNAVAILABLE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['status', 'error'],
  properties: {
    status: { type: 'string', enum: ['unavailable'] },
    error: { type: 'string' },
  },
}

function stageOutputSchema(schema) {
  return { type: 'object', anyOf: [schema, STAGE_BLOCKED_SCHEMA, STAGE_TIMEOUT_SCHEMA, STAGE_UNAVAILABLE_SCHEMA] }
}

// A managed stage is still one Workflow agent call, but that agent is only the courier: it creates
// and assigns a durable mesh task, then waits on mesh's canonical task record. The requested harness
// does the repository work in its already-running taurhaus session. Keeping the shell interaction in
// the courier is intentional — Workflow scripts have no process API, and teaching every procedure a
// second transport would duplicate the babysitter this path replaces.
async function stage(task, options) {
  const value = task && typeof task === 'object' && !Array.isArray(task) ? task : null
  if (!value) throw new Error(NAME + ': stage(task) requires a task object')
  const team = String(A.team || '').trim()
  if (!team) throw new Error(NAME + ': stage(task) requires args.team')
  const harness = String(value.harness || '').trim().toLowerCase()
  if (['codex', 'agy', 'grok'].indexOf(harness) === -1) {
    throw new Error(NAME + ': stage task harness must be codex, agy, or grok — got ' + JSON.stringify(value.harness))
  }
  const requestedEffort = String(value.effort || '').trim().toLowerCase()
  const effort = requestedEffort === 'max' ? 'xhigh' : requestedEffort
  if (['low', 'medium', 'high', 'xhigh'].indexOf(effort) === -1) {
    throw new Error(NAME + ': stage task effort must be low, medium, high, or xhigh — got ' + JSON.stringify(value.effort))
  }
  if (requestedEffort === 'max') log(NAME + ': managed stage maps workflow effort max to mesh xhigh')
  const deadline = Number(value.deadline)
  if (!Number.isInteger(deadline) || deadline <= 0) {
    throw new Error(NAME + ': stage task deadline must be a positive whole number of minutes — got ' + JSON.stringify(value.deadline))
  }
  const fallbackPollLimit = (deadline + 10) * 12
  const worktree = posixPath(value.worktree, 'stage task worktree')
  const requiredStrings = ['why', 'firstStep', 'deliverable', 'title']
  for (const field of requiredStrings) {
    if (!String(value[field] || '').trim()) throw new Error(NAME + ': stage task ' + field + ' must be a non-empty string')
  }
  if (!value.schema || typeof value.schema !== 'object' || Array.isArray(value.schema)) {
    throw new Error(NAME + ': stage task schema must be a JSON Schema object')
  }
  const model = String(value.model || '').trim()
  const resume = options && options.resume != null ? String(options.resume).trim() : ''
  if (resume && !/^[A-Za-z0-9_-]+$/.test(resume)) {
    throw new Error(NAME + ': stage resume task id must contain only letters, numbers, `_`, or `-` — got ' + JSON.stringify(options.resume))
  }
  const slug = String(value.title)
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'task'
  const schemaText = JSON.stringify(value.schema)
  const taskRef = resume ? sh(resume) : '<created-task-id>'
  const createStep = resume
    ? '1) Resume task ' +
      resume +
      ': run `mesh task get ' +
      sh(resume) +
      ' --json --team ' +
      sh(team) +
      '`. Read its owner and reuse its existing owner and same managed session. Do not create a replacement task.'
    : '1) Create the durable task and parse its `id`: `mesh task create --subject ' +
      sh(String(value.title).trim()) +
      ' --description ' +
      sh(String(value.why).trim()) +
      ' --effort ' +
      sh(effort) +
      ' --why ' +
      sh(String(value.why).trim()) +
      ' --deadline ' +
      sh(String(deadline)) +
      ' --first-step ' +
      sh(String(value.firstStep).trim()) +
      ' --deliverable ' +
      sh(String(value.deliverable).trim()) +
      ' --json --team ' +
      sh(team) +
      '`. Call that id `<created-task-id>` below.'
  const assignment = resume
    ? 'Reassign task ' + resume + ' to the owner read in step 1'
    : 'Assign `<created-task-id>` to the selected member'

  return await agent(
    [
      'You are a thin courier for a taurhaus-managed ' + harness + ' stage. The managed member does the task; you do not edit the checkout, implement it, review it, or invoke ' + harness + ' yourself.',
      'Work only with team ' + JSON.stringify(team) + ' and checkout ' + JSON.stringify(worktree) + '. Do not start, stop, restart, or kill a member or daemon. Do not create an unmanaged CLI session.',
      'Use the current mesh actor identity (`MESH_NAME`) and always pass `--team ' + sh(team) + '`. If mesh cannot identify the current actor, return `{status: \'unavailable\', error: <reason>}`.',
      '',
      'MEMBER PRECONDITION:',
      '- Run `mesh who --json --team ' + sh(team) + '` and inspect only this team\'s config plus its runtime records. Resolve the team root from `TAURHAUS_CLAUDE_DIR`, then `CLAUDE_DIR`; use the normal Claude root only if neither is set.',
      '- A fresh stage selects one online non-lead member whose config `cli_tool` is exactly ' + JSON.stringify(harness) + ', whose `cwd`/project path is exactly ' + JSON.stringify(worktree) + ', and whose runtime record has a non-empty `session_id`. ' +
        (model ? 'Its configured model must be ' + JSON.stringify(model) + '. ' : '') +
        'A member without a recorded session cannot have assignment effort applied and is not eligible. If there is not exactly one eligible member, return unavailable with the candidates and the violated precondition; never prompt or launch one to manufacture a session.',
      '- A resumed stage uses the task\'s existing owner, and verifies that same member is online, still uses harness ' + JSON.stringify(harness) + ', still belongs to this worktree, and still has its runtime `session_id`. This is what reuses the member\'s session.',
      '',
      createStep,
      '2) Build the completion signal after the id is known. It must tell the member: on success, send the lead one message whose first line is `RESULT <created-task-id>` and whose only remaining content is a fenced `json` block matching this schema, then run `mesh task complete ' + taskRef + ' --summary "<brief result>" --team ' + sh(team) + '`; on a real blocker, send `BLOCKED <created-task-id> <reason>`. Send RESULT before completing the task. Inline JSON is not a mesh completion block. Schema: ' + schemaText,
      '3) ' +
        assignment +
        ' with `mesh task assign ' +
        taskRef +
        ' --owner <selected-member> --status ' +
        sh('in_progress') +
        ' --effort ' +
        sh(effort) +
        ' --why ' +
        sh(String(value.why).trim()) +
        ' --deadline ' +
        sh(String(deadline)) +
        ' --first-step ' +
        sh(String(value.firstStep).trim()) +
        ' --deliverable ' +
        sh(String(value.deliverable).trim()) +
        ' --completion-signal <the-signal-from-step-2> --team ' +
        sh(team) +
        '`.' +
        (resume ? ' Because this is an intentional reassignment to the same owner, pass `--admin-reason ' + sh('stage resume requested for task ' + resume) + '`.' : ''),
      '4) Poll with `mesh task get ' + taskRef + ' --json --team ' + sh(team) + '` about every five seconds for no more than ' + fallbackPollLimit + ' polls. The task record remains the canonical completion and timeout authority; do not read the inbox separately.',
      '5) On every poll, compare `completion.at` with the current `metadata.assigned_at` as parsed timestamps. Ignore the entire completion when either timestamp is missing/invalid or when `completion.at` is older than the current metadata.assigned_at; this prevents a previous attempt\'s RESULT or BLOCKED from completing a reassigned stage.',
      "6) If the task status is `stale`, return exactly `{status: 'timeout'}`. Otherwise, if the current completion.kind is `blocked`, return `{status: 'blocked', reason: completion.reason}`. If it is `result`, extract and parse the fenced JSON object in `completion.result`, verify it against the schema above, and return exactly that object as your structured output. After " + fallbackPollLimit + ' polls without a current completion or stale status, run one final `mesh task get ' + taskRef + ' --json --team ' + sh(team) + "`; handle a terminal record normally, but if it is still non-terminal return exactly `{status: 'timeout'}`. Do not retry, create another task, or abandon the member's session.",
      '7) If create/assign/get fails, the member precondition is not met, or a current completion is malformed or violates the schema, return `{status: \'unavailable\', error: <specific reason>}`. ' + RULES.honest,
    ].join('\n'),
    call({ label: 'stage:' + harness + ':' + slug, phase: 'Managed stage', schema: stageOutputSchema(value.schema) })
  )
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
  additionalProperties: false,
  required: ['status', 'error', 'model_used', 'summary', 'commits', 'files_changed', 'table_path', 'unresolved'],
  properties: {
    status: { type: 'string', enum: ['ok', 'unavailable'], description: "'ok' only for work you did and saw succeed; 'unavailable' when this lane could not run" },
    error: { type: 'string', description: 'why the lane is unavailable: the exit code and the last log lines' },
    model_used: { type: 'string', description: 'the model that actually ran this lane' },

    summary: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' } },
    files_changed: { type: 'array', items: { type: 'string' }, description: 'repo-relative paths, exactly as git reports them' },
    table_path: { type: 'string', description: 'the drift table: before -> after with file:line evidence' },
    unresolved: { type: 'array', items: { type: 'string' }, description: 'drift you could not settle from the code' },
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

// `note` is the one contract re-request: a lane that came back malformed is re-run with the contract
// restated, under its own label and its own scratch tag so the run tree shows both attempts.
function verifyAgent(round, prior, note) {
  const again = note ? '-recontract' : ''
  const label = 'verify:' + VERIFY_FAMILY + '-r' + round + again
  const task = verifyPrompt(round, prior) + (note ? '\n' + note : '')
  if (VERIFY_FAMILY === 'opus') {
    return agent(
      task + "\nSet reviewer='opus docs', status='ok' and model_used='" + MODELS.opus + "'. " + RULES.honest,
      call({ label: label, phase: 'Verify', schema: FINDINGS_SCHEMA })
    )
  }
  return agent(
    codexWrapper({
      tag: TAG + '-verify-r' + round + again,
      task: task + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: CODEX_ID + ' docs',
    }),
    call({ label: label, phase: 'Verify', schema: FINDINGS_SCHEMA })
  )
}

function verifyLane(round, prior, label) {
  return reviewOnce((note) => verifyAgent(round, prior, note), label)
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
let label = 'the ' + VERIFY_FAMILY + ' verification (round ' + round + ')'
let review = await verifyLane(round, null, label)
// The claim verification is what makes a sweep trustworthy: an unavailable or malformed one fails the run.
let problem = reviewProblem(review, label)
if (problem) fail(problem)
reviewers.add((review.reviewer || VERIFY_FAMILY) + (review.model_used ? ' [' + review.model_used + ']' : ''))
let findings = review.findings.map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
let actionable = actionableFrom(findings)
let trivial = trivialFrom(findings)
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
        '. Apply these verified findings — check each against the code first and skip a wrong one with a stated reason:',
      JSON.stringify(actionable, null, 1),
      'Take the minors and nits too where they are trivial: ' + JSON.stringify(trivial, null, 1),
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
  label = 'the ' + VERIFY_FAMILY + ' re-verification (round ' + round + ')'
  review = await verifyLane(round, prior, label)
  problem = reviewProblem(review, label)
  if (problem) fail(problem)
  reviewers.add((review.reviewer || VERIFY_FAMILY) + (review.model_used ? ' [' + review.model_used + ']' : ''))
  findings = review.findings.map((f) => ({ ...f, reviewer: review.reviewer, round: round }))
  allFindings.push(...findings)
  actionable = actionableFrom(findings)
  trivial = trivialFrom(findings)
  log('Re-verify r' + round + ': ' + findings.length + ' findings, ' + actionable.length + ' actionable')
}
if (actionable.length > 0) {
  log('Stopped short: ' + actionable.length + ' actionable findings left after ' + fixes.length + ' fix rounds')
}

phase('Gate')
const gate = await agent(
  [
    COMMON,
    RULES.gateNotes,
    '',
    'Final gate for the ' +
      TITLE +
      ': run the exact gates above, applying the Rust-diff rule below. No code change is expected — if one is needed for a trivial reason, commit it.',
    trailers(SWEEPER),
    RULES.gateResult(BASE) + ' ' + RULES.honest,
  ].filter(Boolean).join('\n'),
  call({ label: 'gate:' + TAG, phase: 'Gate', schema: GATE_SCHEMA })
)
// A failing gate fails the run: a completed ledger must never sit on top of a red test lane.
const gateFailure = gateProblem(gate, laneChangedPaths([sweep].concat(fixes)))
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
    size: A.size || 'docs',
    implementer: SWEEPER,
    models: MODELS,
    effort: A.effort || 'inherited',
    reviewers: [...reviewers],
    rounds: round,
    majors: allFindings.filter((f) => f.severity === 'blocker' || f.severity === 'major').length,
    findings: allFindings,
    // What the loop could not close: the hard findings it ran out of rounds for, plus the trivia
    // nobody picked up. Both are what `fix-round` takes as `findings`.
    remaining: remaining,
    table: sweep && sweep.table_path ? sweep.table_path : TABLE,
    unresolved: sweep && sweep.unresolved ? sweep.unresolved : [],
  },
  commits: [sweep, ...fixes].filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
