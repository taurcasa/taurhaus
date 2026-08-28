export const meta = {
  name: 'research-sweep',
  description: 'N independent researchers (Opus or Codex) answer one question read-only, each writing a report',
  phases: [{ title: 'Research', detail: 'every researcher runs in parallel, read-only, and returns a structured summary', model: 'opus' }],
}

const NAME = 'research-sweep'

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

const QUESTION = A.question || ''
if (!QUESTION) throw new Error(NAME + ': args.question is required — the one question the sweep answers')
const RESEARCHERS = Array.isArray(A.researchers) ? A.researchers : []
if (RESEARCHERS.length === 0) throw new Error(NAME + ': args.researchers must be a non-empty array of {family, prompt, label?, report?}')
const OUTPUTS = A.outputs || SCRATCH

const COMMON = [RULES.checkout, RULES.readOnly, RULES.safety].join('\n')

const RESEARCH_SCHEMA = {
  type: 'object',
  required: ['summary', 'report_path', 'key_facts', 'unverified'],
  properties: {
    summary: { type: 'string', description: 'the answer in a few sentences' },
    report_path: { type: 'string', description: 'the report this researcher wrote' },
    key_facts: { type: 'array', items: { type: 'string' }, description: 'verified facts, each with its file:line or command evidence' },
    unverified: { type: 'array', items: { type: 'string' }, description: 'open questions and what would settle them' },
    recommendation: { type: 'string' },
  },
}

function slug(value, index) {
  const base = String(value || 'researcher-' + (index + 1)).replace(/[^A-Za-z0-9._-]+/g, '-')
  return base.replace(/^-+|-+$/g, '') || 'researcher-' + (index + 1)
}

function reportPath(researcher, index) {
  const named = researcher.report
  if (named) return named.startsWith('/') ? named : OUTPUTS + '/' + named
  return OUTPUTS + '/' + slug(researcher.label, index) + '.md'
}

function researchPrompt(researcher, index) {
  return [
    COMMON,
    '',
    'RESEARCH QUESTION: ' + QUESTION,
    'Your assignment (you are researcher ' + (index + 1) + ' of ' + RESEARCHERS.length + '; the others work in parallel and you do not coordinate with them):',
    researcher.prompt,
    'Write your report to ' +
      reportPath(researcher, index) +
      ' — sections Result / Evidence / Recommendation, every claim carrying its file:line or command output — then return the structured summary with that path as report_path.',
  ].join('\n')
}

phase('Research')
log('Sweeping ' + RESEARCHERS.length + ' researcher(s) over: ' + QUESTION)
const results = await parallel(
  RESEARCHERS.map((researcher, index) => () => {
    const label = 'research:' + slug(researcher.label, index)
    if (researcher.family === 'codex') {
      return agent(
        codexWrapper({
          tag: slug(researcher.label, index) + '-research',
          task: researchPrompt(researcher, index) + '\nRespond with JSON matching the provided schema only.',
          schema: RESEARCH_SCHEMA,
          timeout: 1700,
        }),
        call({ label: label + ':codex', phase: 'Research', schema: RESEARCH_SCHEMA })
      )
    }
    return agent(researchPrompt(researcher, index) + '\nReturn the structured summary.', call({ label: label + ':opus', phase: 'Research', schema: RESEARCH_SCHEMA }))
  })
)

const summaries = results.map((result, index) => ({
  label: RESEARCHERS[index].label || 'researcher-' + (index + 1),
  family: RESEARCHERS[index].family === 'codex' ? 'codex' : 'opus',
  expected_report: reportPath(RESEARCHERS[index], index),
  result: result,
}))
const missing = summaries.filter((s) => !s.result)
if (missing.length > 0) log('No result from: ' + missing.map((s) => s.label).join(', '))

return { question: QUESTION, outputs: OUTPUTS, researchers: summaries }
