export const meta = {
  name: 'feature-pr',
  description: 'One feature PR: implement (Opus or Codex), cross-family two-lens review, fix loop, gate',
  phases: [
    { title: 'Implement', detail: 'the implementer works the spec red-first and commits per green step', model: 'opus' },
    { title: 'Review', detail: 'the other family reviews: conformance + operational lenses in parallel', model: 'opus' },
    { title: 'Fix', detail: 'fix -> re-review (conformance lens), max 3 rounds', model: 'opus' },
    { title: 'Gate', detail: 'check-quick, lint, targeted tests', model: 'opus' },
  ],
}

const NAME = 'feature-pr'

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
const SIZE = A.size || 'feature'
const MAX_ROUNDS = A.maxRounds || 3
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

const LENSES = [
  {
    key: 'conformance',
    prompt:
      'Lens: spec conformance and correctness — does the change implement the spec item completely and actually fix what it claims; are the tests genuinely red-before/green-after (inspect them, run them); edge cases (timeouts, empty output, partial reads, platform cfgs, backward compatibility on the daemon wire); anything from the spec missing, or anything present that the spec did not ask for.',
  },
  {
    key: 'operational',
    prompt:
      'Lens: operational failure modes, checked as a fixed list before anything else — (1) UPGRADE: persisted data (DB rows, settings blobs, role/preset files, config.json) written by the previous release still loads; renamed or removed enum values never abort a whole record; migrations are idempotent. (2) WIRE: any change to daemon method names, result shapes or serialised enum vocabularies bumps PROTOCOL_VERSION; additive fields carry serde defaults. (3) PLATFORM: Windows app + WSL daemon path mapping (UNC, \\\\wsl$, drvfs), no SQLite from the daemon across drvfs, cfg(unix)/cfg(target_os) hygiene with stubs so every target compiles. (4) USER CONFIG: files under ~/.claude*, ~/.codex, ~/.gemini, ~/.grok are edited only through tempfile+rename, ownership proven before overwrite, symlinks written through, permissions preserved (0600/0700), one writer, and nothing refreshed or rotated on the tool\'s behalf. (5) CONCURRENCY: races between scanner and daemon, repeated degraded states, stale caches, blocking RPCs past the daemon timeout, unbounded retry loops, per-keystroke process leaks. (6) HONEST TESTS: a regression test that would pass without the fix, tests reading the developer\'s real home directories, timing-based assertions. (7) HYGIENE: performance regressions, JSONL log spam, CLAUDE.md violations (over-engineering, legacy Svelte syntax), dead code left by a removal. Report each item as checked-clean or as a finding.',
  },
]

function reviewPrompt(lens, round, prior) {
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
    lens.prompt,
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

function reviewAgent(lens, round, prior) {
  const label = 'review:' + REVIEW_FAMILY + '-' + lens.key + '-r' + round
  const groupPhase = round === 1 ? 'Review' : 'Fix'
  if (REVIEW_FAMILY === 'opus') {
    return agent(
      reviewPrompt(lens, round, prior) + "\nSet reviewer='opus " + lens.key + "'.",
      call({ label: label, phase: groupPhase, schema: FINDINGS_SCHEMA })
    )
  }
  return agent(
    codexWrapper({
      tag: TAG + '-' + lens.key + '-r' + round,
      task: reviewPrompt(lens, round, prior) + '\nRespond with JSON matching the provided schema only.',
      schema: FINDINGS_SCHEMA,
      timeout: 1700,
      reviewer: 'codex gpt-5.6 ' + lens.key,
    }),
    call({ label: label, phase: groupPhase, schema: FINDINGS_SCHEMA })
  )
}

const reviewers = new Set()
function flatten(reviews, round) {
  const done = reviews.filter(Boolean)
  done.forEach((r) => reviewers.add(r.reviewer || REVIEW_FAMILY))
  return done.flatMap((r) => (r.findings || []).map((f) => ({ ...f, reviewer: r.reviewer, round: round })))
}

phase('Implement')
const IMPL_TASK = [
  COMMON,
  '',
  'You are the implementer for ' +
    TITLE +
    ' (size: ' +
    SIZE +
    '). Work the spec item completely — every file, every test and every acceptance signal it names — and keep the change minimal and production-quality. Run the gates when green.' +
    (A.notes ? ' ' + A.notes : ''),
  trailers(IMPLEMENTER),
].join('\n')

const impl =
  IMPLEMENTER === 'codex'
    ? await agent(
        codexWrapper({
          tag: TAG + '-impl',
          task:
            IMPL_TASK +
            '\nWhen completely done, print a final JSON object with keys summary, commits (`git log --oneline ' +
            BASE +
            '..HEAD`), files_changed, tests_added, red_observed, gate, deviations.',
          schema: IMPL_SCHEMA,
          timeout: 3300,
          resume: true,
        }),
        call({ label: 'impl:' + TAG + ':codex', phase: 'Implement', schema: IMPL_SCHEMA })
      )
    : await agent(IMPL_TASK + '\nReturn the structured summary.', call({ label: 'impl:' + TAG + ':opus', phase: 'Implement', schema: IMPL_SCHEMA }))

phase('Review')
// Cross-family rule: whoever implements never reviews.
let round = 1
let reviews = await parallel(LENSES.map((lens) => () => reviewAgent(lens, round, null)))
let findings = flatten(reviews, round)
let actionable = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
const allFindings = findings.slice()
log('Review r1 (' + REVIEW_FAMILY + ', ' + LENSES.length + ' lenses): ' + findings.length + ' findings, ' + actionable.length + ' actionable')

phase('Fix')
const fixes = []
while (actionable.length > 0 && fixes.length < MAX_ROUNDS) {
  const FIX_TASK = [
    COMMON,
    '',
    'You are the fixer for ' +
      TITLE +
      ', round ' +
      (fixes.length + 1) +
      '. Apply these findings from the independent reviewers — verify each against the code first and skip a wrong one with a stated reason. Every blocker and major gets a red-first regression test:',
    JSON.stringify(actionable, null, 1),
    'Take the minors too where they are trivial: ' + JSON.stringify(findings.filter((f) => f.severity === 'minor'), null, 1),
    'Keep the change local to the files named — no new features, no architecture reshaping. Re-run the gates.',
    trailers(IMPLEMENTER),
  ].join('\n')
  const fix =
    IMPLEMENTER === 'codex'
      ? await agent(
          codexWrapper({
            tag: TAG + '-fix-r' + (fixes.length + 1),
            task: FIX_TASK + '\nWhen done print a final JSON object with keys summary, commits, files_changed, tests_added, red_observed, gate, deviations.',
            schema: IMPL_SCHEMA,
            timeout: 3000,
            resume: true,
          }),
          call({ label: 'fix:' + TAG + '-r' + (fixes.length + 1) + ':codex', phase: 'Fix', schema: IMPL_SCHEMA })
        )
      : await agent(
          FIX_TASK + '\nReturn the structured summary.',
          call({ label: 'fix:' + TAG + '-r' + (fixes.length + 1) + ':opus', phase: 'Fix', schema: IMPL_SCHEMA })
        )
  fixes.push(fix)
  const prior = JSON.stringify(actionable)
  round += 1
  // Re-review runs the conformance lens only: it verifies the prior findings and looks for regressions.
  reviews = [await reviewAgent(LENSES[0], round, prior)]
  findings = flatten(reviews, round)
  allFindings.push(...findings)
  actionable = findings.filter((f) => f.severity === 'blocker' || f.severity === 'major')
  log('Re-review r' + round + ': ' + findings.length + ' findings, ' + actionable.length + ' actionable')
}
if (actionable.length > 0) {
  log('Stopped short: ' + actionable.length + ' actionable findings left after ' + fixes.length + ' fix rounds — hand them to the fix-round workflow')
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
      '...HEAD` (map files to module paths), plus the full frontend unit tests if any src/ file changed. No stress or load runs. Do not modify code unless a gate fails for a trivial reason (formatting, an unused import); if you must, commit it.',
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
  commits: [impl, ...fixes].filter(Boolean).flatMap((r) => r.commits || []),
  gate: gate,
}
