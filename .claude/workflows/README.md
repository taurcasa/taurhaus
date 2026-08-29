# Workflows — the versioned procedures

These are the procedures that produce taurhaus PRs, kept in the repo instead of in one session's
directory. Claude Code resolves a named workflow from `<project>/.claude/workflows/<name>.js`, so
anyone working in this checkout can run them.

A lead triggers one directly:

```
Workflow({name: "small-change", args: {worktree: "/home/you/projects/taurhaus-w1", branch: "feat/w1", spec: "/tmp/w1-spec.md"}})
```

or hands a team member the same call in a mesh notice:

```
ACTION REQUIRED: Invoke Workflow({name:"small-change", args:{worktree:"…", branch:"…", spec:"…"}}) — mark the task complete and report the ledger.
```

The slash form (`/small-change {…}`) works too. A named run is a background task; its return value
lands in the session's task output and the run tree under `<session>/subagents/workflows/<runId>/`.

## The five procedures

| Script | Shape | Use it for |
|---|---|---|
| `feature-pr.js` | implement → two-lens cross-family review → fix ↔ conformance re-review (≤3 rounds) → gate | a feature-sized PR |
| `small-change.js` | implement → one cross-family lens → ≤1 fix round → gate | a small PR: a bug fix, a script, a contained refactor |
| `fix-round.js` | fix ↔ conformance re-review (`startRound`…, ≤`maxRounds`) → gate | a run that stopped short with findings still open |
| `research-sweep.js` | N independent researchers in parallel, read-only, one report each | a question that needs inventories or spikes before a plan |
| `docs-sweep.js` | sweep the doc groups → cross-family claim verification (≤3 rounds) → gate | documentation drift after a release or a subsystem change |

## Args

Every script takes the shared args below; `worktree` (or `repo`) is the only hard requirement.

| Arg | Default | Meaning |
|---|---|---|
| `worktree` / `repo` | — (required) | absolute path of the checkout the agents work in |
| `branch` | current branch | branch name, used in prompts and in the scratch-file tags |
| `base` | `main` | diff base: reviewers read `git diff <base>...HEAD` |
| `spec` | — | absolute path of the spec; agents are told to build its minimum deliverable only |
| `title` | the spec path | what the change is called in prompts and in the ledger |
| `implementer` | `opus` | `opus` or `codex` — the other family reviews |
| `effort` | inherit | `low`/`medium`/`high`/`xhigh`/`max`, applied to every agent call — and to Codex as `-c model_reasoning_effort` |
| `codexModel` | the Codex CLI's own default | model slug passed to `codex exec` as `-m`; the ledger records what actually ran, never a guess |
| `scratch` | `/tmp/taurhaus-workflows` | where the Codex wrapper writes prompts, schemas and logs |
| `stamp` | — | a short token appended to the scratch file names; the names already carry the checkout and the tag, so pass one only to run the same procedure in the same checkout twice in sequence (a workflow script cannot read the clock itself) |
| `sessionUrl` | — | the `Claude-Session:` trailer value; omitted when absent |
| `gates` | check-quick + lint + targeted cargo tests | the gate commands, when a spec names different ones |
| `requiredGates` | `['just check-quick', 'just lint']` | further commands the gate must actually run and pass, on top of `just check-quick` and `just lint`, which cannot be opted out of; matched as substrings of the reported command line |
| `notes` | — | extra instructions appended to the implementer's task |
| `tag` | the branch | prefix for scratch file names |
| `size` | per script | recorded in the ledger |

Per script:

- `feature-pr`: `maxRounds` (default 3).
- `fix-round`: `findings` (required — the open findings from the run that stopped short; every
  severity but a nit is fixed, because a finding another run could not close is already an open one),
  `startRound` (default 2), `maxRounds` (default 2), `fixNotes`.
- `research-sweep`: `question` (required), `researchers` (required — `[{family, prompt, label?, report?}]`),
  `outputs` (report directory, default `scratch`).
- `docs-sweep`: `table` (drift table path), `groups` (file groups to sweep), `maxRounds` (default 3).

Every `*_SCHEMA` literal keeps `additionalProperties: false` on each object and lists every property in `required`: the OpenAI structured-output endpoint behind `codex exec --output-schema` rejects anything else, and `just lint-workflows` checks it.

## What a run returns

`feature-pr`, `small-change`, `fix-round` and `docs-sweep` return the same ledger shape, so a plan's
ledger row can be filled from the run instead of by hand:

```js
{
  ledger: { title, size, implementer, models, effort, reviewers, rounds, majors, findings, remaining },
  commits: [...],
  gate: { status: 'pass', commands: [{command, status, detail}], diff_stat, commits },
}
```

`remaining` is what the loop could not close — the blockers and majors it ran out of rounds for, plus
the minors and nits nobody picked up. Feed it to `fix-round` as `findings` with `startRound` set to
`rounds + 1`. `research-sweep` returns `{question, outputs, researchers}` with one
structured summary and report path per researcher; the lead synthesizes them.

## Failing closed

A run that cannot show a real cross-family review and a green gate raises instead of returning, because
a completed ledger with no findings reads as an approval:

- **An absent reviewer is not an approval.** Every lane returns `status`; a lane that could not run
  returns `status: 'unavailable'` with the error. An unavailable, empty or malformed review — no
  findings array, a verdict outside `approve`/`fix_required` — fails the run, and a reviewer is
  recorded in the ledger only after its result validates.
- **A `fix_required` means at least one blocker or major.** That is the verdict contract, stated to
  every reviewer and enforced on every result. A `fix_required` carrying nothing above a minor — no
  findings at all, nits only, minors only — is malformed, not a verdict: the fix loop has nothing to
  act on, so the withheld approval would otherwise complete as an approval. The lane is re-requested
  **once** with the contract restated (its own label and scratch tag, `…-recontract`), and a second
  malformed review fails the run — no gate, no ledger. Minors and nits are welcome under `approve`:
  they ride along to the fixer as trivia and come back in `remaining`.
- **A red gate fails the run.** The gate returns one entry per command with its pass/fail; any command
  that did not pass, a `status` other than `pass`, or a gate that ran nothing aborts. So does a gate
  that contradicts itself — `status: 'pass'` arriving with a non-empty `failures` or `error`.
- **A skipped gate command is not a pass.** `just check-quick` and `just lint` (or whatever
  `requiredGates` names) must appear among the commands that passed; one reported `skipped`, or never
  run at all, fails the run. So does any other listed command that did not pass — the targeted
  `cargo test` included. A gate command that did not apply is left off the list and explained in the
  summary, never reported `skipped` to get past the gate.
- **What stays open does not fail it.** Findings the loop could not close come back as `remaining` —
  that is what `fix-round` is for.
- **A reviewer is named by the model that ran it.** The lane must report `model_used`, the reviewer
  label carries `codexModel` (or "cli default" when none is pinned), and a commit trailer names a
  Codex model only when the run pinned one. No script claims a model nobody requested.

## The model split

- **Opus implements, fixes and sweeps.** Every `agent()` call in these scripts runs on Opus.
- **The other family reviews.** Whoever implements never reviews: `implementer: "opus"` gets Codex
  reviews, `implementer: "codex"` gets Opus reviews.
- **Codex runs behind a thin wrapper.** An Opus agent writes the prompt, the output schema and a
  runner script to `scratch`, launches the runner detached (`codex exec --yolo … -o`), polls for the
  `EXIT=` marker (one Bash call is capped at 10 minutes) up to a deadline, and returns Codex's JSON.
  Every path is one single-quoted shell word and the command lives in the runner rather than inside a
  nested `bash -c '…'`, so a checkout under `/mnt/c/Users/Jane Doe/…` works; a Windows or `\\wsl$` path
  in `worktree` is normalized first. `-m` carries `codexModel` and `-c model_reasoning_effort` carries
  `effort`, on the resumed turns too. The implementer lane may take up to three `codex exec resume`
  turns — `resume` does not accept `-C`, so the runner's `cd` is what places it.
- **An abandoned run kills itself.** Every give-up path above is an instruction, and an instruction
  cannot run once the lane that owned it is gone — an abort after the launch used to leave the group
  writing to the checkout until its own 1700-3300s timeout. So the runner holds an ownership lease:
  it creates `<scratch>/codex-<tag>.lease`, the wrapper `touch`es it in every poll loop, and a
  watchdog inside the runner kills its own process group (TERM, then KILL) when the lease goes
  unrefreshed for 300 seconds. `TAURHAUS_WORKFLOW_LEASE_TTL` and `_POLL` override the timings, which
  is how the regression test proves a real dummy group dies in seconds. The runner's own TERM trap
  takes the group down with it too.
- **The wrapper owns what it launches.** The runner is started with `setsid`, so it leads its own
  process group, and its first act is to write that pid to its pidfile. Every give-up path — the poll
  deadline, the one retry, and the return itself — kills the group (`kill -TERM -"$PGID"`, then
  `-KILL`), not just the runner shell: killing the shell alone left `timeout` and Codex alive to keep
  writing to the checkout while the retry ran. The resumed turns name the session id read from the log
  rather than `--last`, which resolves to the newest session on the machine and can belong to somebody
  else's run in the same checkout.
- **Ownership artifacts never collide, and a kill is never aimed at a stranger.** Every scratch file
  is named `codex-<checkout>-<tag>[-<stamp>]` — the basename of `worktree`, then the tag, then
  `args.stamp`. `tag` defaults to the branch and the scratch dir is shared, so without the checkout in
  the name two worktrees on one branch wrote the same pidfile and each run's kill paths then aimed at
  the other's pid. Before launching, the wrapper reads the pidfile: if that pid is alive **and** its
  `/proc/<pid>/cmdline` names this runner, it refuses to launch and returns `unavailable` with the pid
  rather than overwriting the file. And every kill — the deadline, the retry, the return — re-reads
  the pid's own command line first and kills only a group whose leader still names the runner it
  started; a pidfile outlives a crashed run, and the pid it names can already be somebody else.
  Running one procedure twice in the same worktree at once is **not supported**: the two runs would
  fight over the git index anyway. Run them in different worktrees, or in sequence with a `stamp`.
- **Effort is inherited** unless `args.effort` pins one, and then it applies to every call in the run.

## Sizing policy

- **One-off** — a typo, a one-line fix, anything you can verify by reading it: do it inline in the
  session. A workflow costs more than it returns.
- **Small** — a contained change with a spec: `small-change`. One implementer, one review lens, one fix
  round. If findings are still open at the end, that is what `fix-round` is for.
- **Feature** — a PR that touches several modules or a wire contract: `feature-pr`. Two review lenses
  (conformance and the operational checklist) and up to three fix rounds.

## The rules the scripts encode

The shared `lib` section is **byte-identical in every script** — workflow scripts cannot import, so it
is copied. Change it in one file and copy it to the other four; `just lint` will not catch drift, a
reviewer will. It carries:

- the checkout rule (work only there, read `CLAUDE.md` first, never `git add -A`);
- commit discipline (commit after every green step; never edit ledger rows — the orchestrator fills
  them at merge);
- TDD (red first, `// Regression:` comments naming the breaking commit);
- safety (tests never touch the real `~/.claude*`, `~/.codex`, `~/.gemini`, `~/.grok`; no stress runs;
  kill what you start, never what you did not; never print secrets);
- the read-only rule for research;
- the scope rule for reviewers (judge against the spec's minimum; missing scaffolding is at most a
  minor; majors are defects a user would hit) and the verdict contract (`fix_required` carries at
  least one blocker or major), with the one re-request that enforces it;
- the Codex wrapper builder.

## The lint

`just lint` runs `bun scripts/check-workflow-scripts.mjs`, which parses every script here without
running it and fails on: a syntax error, a missing or misplaced `export const meta`, a `meta.name` that
does not match the file name, a missing description, an `import`/`require` (impossible in a workflow
script), and `Date.now()` / argless `new Date()` / `Math.random()` (they throw at runtime — pass a
timestamp through `args`, and vary a prompt by index instead of randomising).

Run it directly with `bun scripts/check-workflow-scripts.mjs` (or `node …`, which adds the line
number of a syntax error — bun's parser reports the file only).

`scripts/workflow-procedures.test.mjs` goes further: it runs each script against a stubbed Workflow API
and pins the control flow — the fail-closed rules above, the Codex launcher's quoting and flags, the
path normalization — without spawning a single agent.

## Not here yet

User-scope installation is a follow-up: these procedures resolve from this checkout's
`.claude/workflows/`, and copying them to `<CLAUDE_CONFIG_DIR>/workflows/` so a lead can run them from
any project is not built. Nor is the run scanner that turns a run tree into the mesh canvas and a
ledger export (W2), or generated agent definitions (W3) — see
[`docs/design/workflows-integration-plan.md`](../../docs/design/workflows-integration-plan.md).
