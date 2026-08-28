# Model evidence — internal record

Read-only audit, 2026-08-28. Nothing was edited in either repo; no git write ran; no
credential value was read or printed. Anything not read directly from a file, a command
output, or a cited page is marked **INFERRED**.

Sources: `~/projects/taureval` (scoring/*), `~/projects/taurhaus`
(`docs/design/*.md`, `docs/design/research/*.md`, `docs/archive/design-workflow.md`,
`src-tauri/src/models/mod.rs`, `src-tauri/resources/templates/roles/*.yaml`,
`scripts/load-test-keys.sh`, `CHANGELOG.md`), and
<https://platform.claude.com/docs/en/about-claude/model-deprecations> (fetched 2026-08-28).

---

# JOB 1 — taureval judges

## 1.1 Where judge models are chosen

One file, no CLI override.

- `~/projects/taureval/scoring/judge-config.ts` — the only place a judge model
  string exists. Four `JudgeConfig` constants and a `JUDGES_BY_TASK_CLASS` table keyed by
  the seven task classes (`product-review`, `design-review`, `implementation`,
  `architecture-review`, `coordination`, `code-review`, `documentation`). Each class lists
  all four judges in a different order.
- `~/projects/taureval/scoring/runner.ts:295-299` — the single call site:
  `selectJudgeConfig(evalSet.task_class, subjectFamily(adapter, model), "<roleId>:<contentHash>:<adapter>:<model>:<effort>")`.
- `~/projects/taureval/scoring/cli.ts` — parses `--model`, `--effort`,
  `--adapter`, `--runs`, `--timeout`, `--desc`. **There is no `--judge` flag.** The judge is
  never operator-selectable; it is derived.
- `~/projects/taureval/scoring/judge.ts` — executes the chosen judge.

## 1.2 The four judges as configured today

| Const | Family | Model id | Effort | Invoked as |
|---|---|---|---|---|
| `ANTHROPIC_OPUS` | `anthropic` | `claude-opus-4-6` | `high` | `claude -p --output-format json --model claude-opus-4-6 --effort high --append-system-prompt <sys> <userPrompt>` (`judge.ts:71-95`) |
| `ANTHROPIC_SONNET` | `anthropic` | `claude-sonnet-4-5` | `high` | same shape, `--model claude-sonnet-4-5` |
| `OPENAI_SOL` | `openai` | `gpt-5.6-sol` | `high` | `codex exec --json --ephemeral --skip-git-repo-check --sandbox read-only --model gpt-5.6-sol -c model_reasoning_effort="high" -` with the prompt on stdin (`judge.ts:105-140`) |
| `OPENAI_TERRA` | `openai` | `gpt-5.6-terra` | `high` | same shape, `--model gpt-5.6-terra` |

Both paths are `execFileSync`, `timeout: 120_000` ms, `maxBuffer: 10 MiB`. The Anthropic
path reads `envelope.result` from the JSON envelope; the Codex path filters the NDJSON
stream for `type === "item.completed" && item.type === "agent_message"`.

## 1.3 How they authenticate — NOT via `scripts/load-test-keys.sh`

**Judges are CLI-invoked and inherit the ambient environment. No API key is set, read, or
passed by taureval.** `judge.ts` passes only `encoding`, `timeout`, `maxBuffer` (and
`input` for Codex) to `execFileSync` — there is no `env:` option on either call, so both
children inherit `process.env` unchanged.

Consequence: judges run on whatever auth the `claude` and `codex` CLIs already hold on the
host. The research reports record that as subscription auth on this machine:
`codex login status` → `Logged in using ChatGPT` with `auth_mode: chatgpt`
(`docs/design/research/codex-accounts-usage-report.md`), and the Claude side is a
claude.ai/max subscription (`docs/design/workflows-and-multi-model-orchestration.md:39`).

`scripts/load-test-keys.sh` lives in **taurhaus**, not taureval. Grep over both repos finds
exactly one reference to it — its own usage comment. It is a manual Mac-side helper: it
reads `~/.maccreds` (`key=value`) and exports `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
`GEMINI_API_KEY` (Antigravity/Vertex path), `XAI_API_KEY` into the current shell only, then
tells the user to `rm ~/.maccreds`. It is wired to nothing in the judge path.

**Hazard worth flagging (INFERRED):** if that script is sourced in the same shell that then
runs `bun run scoring/cli.ts`, the exported `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` would be
inherited by the `claude` and `codex` judge children. The claude-api reference states
`ANTHROPIC_API_KEY` outranks an OAuth profile in the SDK credential chain; that the Claude
Code **CLI** follows the same precedence is INFERRED, not probed here. If it does, judging
would silently move from subscription to metered API billing with no signal in the run
record — `run_config` stores `judge_model`/`judge_effort` but nothing about auth mode.

## 1.4 Deprecation status of each judge — the brief's premise is only half right

Primary source: Anthropic model deprecations page, fetched **2026-08-28**
(<https://platform.claude.com/docs/en/about-claude/model-deprecations>). Its lifecycle
vocabulary: *Active* (fully supported), *Legacy*, *Deprecated* (replacement + retirement
date assigned), *Retired*.

| Judge model | Official state (2026-08-28) | Tentative retirement | taurhaus `ModelCatalog` `deprecated` flag |
|---|---|---|---|
| `claude-opus-4-6` | **Active** | not sooner than 2027-02-05 | `false` (`models/mod.rs:506-513`) |
| `claude-sonnet-4-5` (`-20250929`) | **Active** | **not sooner than 2026-09-29 — 32 days from today** | `false` (`models/mod.rs:514-521`) |
| `gpt-5.6-sol` | n/a (OpenAI) | — | `false` (`models/mod.rs:524-531`) |
| `gpt-5.6-terra` | n/a (OpenAI) | — | `false` (`models/mod.rs:532-539`) |

So, precisely:

- **Neither Anthropic judge is deprecated.** Both are Active. `claude-opus-4-6` is three
  Opus generations behind (4.7, 4.8, Opus 5 have shipped since) — stale, not deprecated.
- **`claude-sonnet-4-5` is the one that must move on a clock.** Its tentative retirement is
  2026-09-29, the nearest of any model in the judge table, roughly one month out. When it
  retires, `design-review`, `architecture-review`, `coordination` and `documentation`
  classes lose half their Anthropic rotation and every judged run of an OpenAI-family
  subject in those classes starts failing at the `execFileSync` boundary.
- **`gpt-5.4` is deprecated but is not a judge.** taurhaus's own catalog marks it
  `deprecated: true, replacement: "gpt-5.6-terra"` (`models/mod.rs:556-563`) and
  `gpt-5.4-mini` `deprecated: true, replacement: "gpt-5.6-luna"` (`:564-571`). Neither
  appears in `judge-config.ts`. They appear elsewhere — see 1.6.
- The two OpenAI judges are the current generation per taurhaus's catalog and need no change.

## 1.5 A current replacement set

Constraint that shapes this: judges are driven through the **CLIs under subscription**, so
the model string has to be one `claude --model` / `codex --model` accepts, not just a valid
API id. Spike S9 (`harness-realignment-plan.md`, executed 2026-08-21) probed only
`claude --model claude-opus-4-6` and `claude --model claude-sonnet-4-5` — both returned
`OK`, exit 0, under subscription. That the CLI accepts `claude-opus-5` / `claude-sonnet-5` /
`claude-fable-5` as literal `--model` values is **INFERRED** and should be probed the same
way before the swap.

Recommended set, minimal-change:

| Slot | From | To | Rationale |
|---|---|---|---|
| `ANTHROPIC_OPUS` | `claude-opus-4-6` | `claude-opus-5` (or the alias `opus`) | Active, retirement not sooner than 2027-07-24; taurhaus's catalog already labels the alias `opus` as "Opus 5" (`models/mod.rs:502`), so `--model opus` is the probe-free route |
| `ANTHROPIC_SONNET` | `claude-sonnet-4-5` | `claude-sonnet-5` (or the alias `sonnet`) | Active, retirement not sooner than 2027-06-30; removes the 2026-09-29 cliff |
| `OPENAI_SOL` | `gpt-5.6-sol` | unchanged | current in taurhaus's catalog |
| `OPENAI_TERRA` | `gpt-5.6-terra` | unchanged | current in taurhaus's catalog |

Two optional widenings, both INFERRED as improvements rather than measured:

- Add `claude-fable-5` as a third Anthropic judge for the two hardest classes
  (`architecture-review`, `code-review`). It is Anthropic's most capable widely released
  model. Cost caveat: $10/$50 per MTok vs Opus 5's $5/$25 on the first-party API — but the
  judges run on subscription, so per-token pricing may not be the operative cost here.
- Add `gpt-5.6-luna` as a third OpenAI judge for rotation width. Note its effort ladder
  excludes `ultra` (`CODEX_EFFORTS_WITH_MAX`, `models/mod.rs:540-547`); `high` — what
  `judge-config.ts` hard-codes — is supported.

Also worth reconsidering: **every judge is pinned to `effort: "high"`.** `gpt-5.6-sol`'s
catalog default is `low` and it supports `ultra`; `gpt-5.6-terra`'s default is `medium`.
For strict binary adjudication, `xhigh` on the Anthropic side is plausibly the better
setting. INFERRED — there is no measurement in the repo either way (see 1.7).

## 1.6 Stale model strings outside the judge table

The same generation of ids the brief flags is spread wide, and none of it is judge code:

- **taurhaus role templates** — 39 files in `src-tauri/resources/templates/roles/`:
  17 × `model: gpt-5.4` (deprecated in taurhaus's own catalog), 17 × `model: claude-opus-4-6`,
  1 × `model: claude-sonnet-4-5` (`claude-researcher.yaml:12`), 3 × `gemini-3.7-flash-high`,
  1 × `grok-4.6`. taureval reads these role files in place
  (`program.md`; `loader.ts` resolves against `../taurhaus/src-tauri/resources/templates/roles`),
  so a role's *subject* model is `gpt-5.4` for 17 of 39 roles.
- **taureval's own copies** — `roles/v3-quick-dev-codex.yaml:12` and
  `roles/v3-docs-verifier-codex.yaml:12` both `model: "gpt-5.4 high"`;
  `roles/v3-adversarial-reviewer-claude.yaml:12` `model: claude-opus-4-6`.
- **taureval hard-codes** — `scoring/spawner.ts:117,143,313` pin `--model claude-opus-4-6`
  for the orchestrator/lead spawn.
- **taureval tests** — `harness.test.ts`, `runner.test.ts`, `db.test.ts` use `gpt-5.4` and
  `claude-opus-4-6` as fixtures (fixtures, so lower priority).
- **taureval docs** — `CLAUDE.md:150-153` "Model Notes" still describes the team as
  "Claude (Opus 4.6)" and "GPT 5.4 (Codex)"; `whats-next.md:14,65,202` still shows
  `codex --yolo -m gpt-5.4`.
- **taurhaus experiment plans** — `workflows-and-multi-model-orchestration.md:93,100-102`
  budgets Experiments 3–5 on `gpt-5.4-mini` (deprecated → `gpt-5.6-luna`).

## 1.7 The judge rotation has never produced a row

`results/taureval.db` (read-only query, 2026-08-28) holds **17 `eval_runs`, all dated
2026-03-21/22**, and its `eval_runs` table is the **pre-migration schema** — no
`run_config`, no `cli_version`, no `content_hash`. `case_results` has 467 rows on the old
shape. So `judge_config`, `judge_model` and `judge_effort` — the columns
`initializeSchema` adds at `db.ts:72-96` — carry no data yet. The rotation in
`judge-config.ts` is untested against real subjects; the only coverage is the one unit test
in `judge-config.test.ts`.

## 1.8 How `subjectFamily` excludes same-family judges

Two functions, `harness.ts:187-195` and `judge-config.ts:47-62`.

**Classification** (`subjectFamily(adapter, model)`, `harness.ts:187`), first match wins on
the lowercased model string:

1. model contains `"claude"` **or** adapter is `"claude-code"` → `"anthropic"`
2. model contains `"gpt"` **or** `"codex"` **or** adapter is `"codex-cli"` → `"openai"`
3. otherwise → `"other"`

**Exclusion** (`selectJudgeConfig(taskClass, subjectFamily, rotationSeed)`):

```ts
const configured = JUDGES_BY_TASK_CLASS[taskClass];        // throws on unknown class
const independent = configured.filter(j => j.family !== subjectFamily);
const candidates  = independent.length > 0 ? independent : [...configured];
return candidates[stableIndex(rotationSeed, candidates.length)];
```

`stableIndex` is a 32-bit rolling hash (`hash = (hash*31 + charCode) >>> 0`) mod the
candidate count — deterministic, not random.

Properties that follow, and the gaps in them:

- **The fallback branch is dead for the current table.** `judge-config.test.ts` asserts
  every class lists both families, so `independent` always has ≥ 1 member; for an
  anthropic or openai subject it always has exactly 2. The `[...configured]` fallback only
  fires if someone writes a single-family class list.
- **`other` excludes nothing.** An `agy-cli` subject on `gemini-3.7-flash-high`, or a
  `grok-cli` subject on `grok-4.6`, classifies as `"other"`, so all four judges stay
  candidates. That is correct behaviour — neither is an Anthropic or OpenAI model.
- **Two real misclassifications hide in taurhaus's `agy` catalog.** It carries
  `claude-sonnet-4-6` and `claude-opus-4-6-thinking` (`models/mod.rs:662-678`) — those
  correctly land as `"anthropic"` and correctly exclude the Anthropic judges. But it also
  carries `gpt-oss-120b-medium` (`:679-686`), an open-weights model served by Google, which
  the `"gpt"` substring rule sends to `"openai"` and which therefore **wrongly excludes both
  OpenAI judges**. Substring matching on the model string is the root cause.
- **The judge is keyed to the role text.** `rotationSeed` is
  `${roleId}:${contentHash}:${adapter}:${model}:${reasoningEffort ?? ""}`
  (`runner.ts:298`), and `contentHash` is the SHA-256 of the role YAML (`runner.ts:294`).
  So editing role text can re-roll which judge scores it. Combined with `db.ts:122-134`,
  where currency is keyed on **role + model + effort + adapter only** and "a CLI bump or
  judge rotation supersedes the older run rather than standing beside it", a keep/discard
  decision between two role versions can carry a judge change inside it. That is a real
  methodological confound for the optimization loop in `program.md`, and it is not recorded
  as a caveat anywhere in the repo.
- **Provenance is recorded per row**: `judge_config` JSON on every `case_results` row and
  `judge_model`/`judge_effort` inside `run_config` on `eval_runs` (`db.ts:37-39, 76, 96`).
  Zero rows carry it today (see 1.7).

---

# JOB 2 — per-family observations from our own record

## 2.1 The dataset

Two ledgers, both in `docs/design/`, both described in-repo as
"the first real dataset for 'which model is good at what' on production work"
(`harness-realignment-plan.md:176`).

- `harness-realignment-plan.md:178-198` — 19 merged PRs (#9–#33), 2026-08-21 → 2026-08-27.
- `accounts-and-usage-plan.md:96-104` — 6 rows (#34–#41), 2026-08-27 → 2026-08-28, one cancelled.

The process they record (`docs/architecture/harness-model.md:71`): each PR is implemented
by one model family and reviewed by the other (Opus ↔ Codex) through two lenses —
conformance to spec, and an operational checklist (persisted-data upgrade, protocol bumps,
Windows/WSL paths, user-config edit discipline, concurrency, honest tests, hygiene) — with
fix → re-review repeated until no majors remain. The orchestrator (Fable 5) writes the
spec, fills the ledger, and merges on the check's conclusion. Each new CLI starts with two
**independent** research reports.

Aggregates computed from the two ledger tables:

| Implementer | PRs | Review rounds | Majors raised against it | Majors/PR | Rounds/PR |
|---|---|---|---|---|---|
| Codex gpt-5.6 | 15 | 47 | 95 | 6.3 | 3.1 |
| Opus 5 | 9 | 44 | 101 | 11.2 | 4.9 |

**Confound, stated up front (INFERRED):** the split is not random. Opus drew the
user-facing, multi-surface features — PR 8 (hub-owned focus + protocol bump), 5c (frontend
catalog), 16 (account selection), 16b (status-line bridge), 17a (popup + menus), 18b (whole
new harness) — while Codex drew more single-surface backend work. Larger surface plausibly
explains part of the majors/PR gap. The *kinds* of findings below are the more reliable
signal than the counts.

## 2.2 Observation table

### Fable 5 — orchestrator

**Role in the record:** writes the spec, arbitrates, fills the ledger, makes the merge call
(`harness-model.md:71`). Wrote the narrative half of the PR 19 docs sweep
(`accounts-and-usage-plan.md:104`).

**Strengths shown**
- Terminates review loops that would otherwise keep spinning. Explicit ledger entries:
  "orchestrator cut" (PR 5a/5b), "orchestrator default flip" (PR 9), "the last fix was
  verified by the orchestrator instead of a tenth Codex round" (PR 16b), "the last major …
  fixed by the orchestrator's pass" (17a), "the orchestrator settled the last seven
  findings" (19).
- Absorbs residue the reviewer loop leaves: last minor on PR 15, two minors on 17c, the
  orchestrator fix on 18b.
- Owns spec-writing for cross-family work, including the escalation rule that a spec
  touching user config or persisted formats is reviewed by the *other* family first.

**Failure modes shown**
- **Approval without adversarial depth.** On PRs 2 and 3 the reviewer column reads
  `Fable ×2 (approve), Codex gpt-5.6` — Fable approved twice, then Codex found 3 majors on
  PR 2 (degraded scans still mutated state; Windows daemon branch lacked the flag) and 2 on
  PR 3 (macOS ignored per-process `CLAUDE_CONFIG_DIR`; unknown status discarded identity).
  Fable-as-reviewer is not a substitute for a cross-family adversarial pass.
- **Merge-gate error.** PR 17c "merged with a red lint gate by orchestrator error — unused
  import fixed forward on `main`" (`accounts-and-usage-plan.md:100`).

**Cost / duration signals**
- Appears in ~8 of 25 ledger rows as the terminating actor, i.e. roughly a third of PRs
  need an orchestrator pass to close.

**Implication for role wording (INFERRED)**
- Word it as **decision authority and loop termination**, not as review. State explicitly
  that its approval does not satisfy the cross-family review requirement — the ledger
  already proves it does not.
- Give it an explicit **merge checklist** with the gate result named as a required input;
  the one recorded merge error was a gate that was red and not read.
- Give it the **spec-escalation rule** as a hard clause: any spec touching user config or
  persisted formats goes to the other family before implementation starts.

---

### Opus 5 — implementer and reviewer

**Strengths shown**
- **As reviewer against Codex, it is the operational-hazard finder.** Its findings cluster
  on unbounded and destructive behaviour: retry flood and liveness-blind fallback (PR 9),
  unbounded extractor maps and two owners after daemon recovery (PR 6), "failed probe
  deleting the installed hook" (PR 13), "skip path re-journalled forever and bypassed
  escalation" (PR 14), "app-side compaction owner unreachable on Linux/macOS" (PR 6,
  blocker).
- **Catches abstraction leakage in the PR that builds the abstraction.** PR 15 (capability
  registry) — Opus found the blocker that Gemini lost all runtime session binding behind a
  hard-coded idle floor, plus `config_dir_env` used as the "is Claude" predicate, tool
  identity laundered through the `accent` token, and `catalog: none` panicking.
- **Volume research.** Its harness reports are the longer and denser of each pair:
  `grok-report-opus.md` 8,438 words / 1,284 lines vs Codex's 6,530 / 654;
  `agy-report-opus.md` 6,413 / 811 vs Codex's 5,879 / 601. The grok Opus report caught the
  framing that mattered most operationally — "grok is already executing taurhaus's Claude
  hooks on this machine" — which became the dedupe requirement shipped in PR 18b.
- Handles wide mechanical sweeps: PR 19 drift sweep across 35 files.

**Failure modes shown (with PR/round evidence)**
- **Concurrency, atomicity and ownership under real hosts.** PR 16b, 9 rounds, 25 findings
  — app/daemon double owner with cross-namespace paths, non-CAS commit, lock-timeout
  partial reads, in-place rewrite of a live script, stale removal deleting the script,
  pipe-subshell payload loss for compound commands, substring ownership, symlinked settings
  severed, process-wide version probe never re-read. PR 16 — chooser races, stale state,
  non-atomic save. PR 8 — seed races, startup-fallback race.
- **A finding class re-raised across rounds instead of converging in one fix.** PR 5c
  "preset override semantics ×3 rounds"; PR 10 "degraded state invisible to the frontend
  ×2 rounds"; PR 18b "compaction event value mismatch across two rounds". Opus fixes the
  named instance, not the class.
- **Vacuous or dishonest tests.** PR 8 "E2E vacuity" (found by Codex).
- **Numeric claims asserted without re-measuring.** PR 19: Codex's four verification rounds
  raised 60+ findings against the Opus drift sweep, including wrong counts — 89→90 IPC
  commands, protocol 10/11→13, 27→28 daemon methods.
- **A security-shaped miss:** "world-readable copies of the user's command" (PR 16b).

**Cost / duration signals**
- 9 PRs, 44 rounds, 101 majors. Worst case PR 16b: **9 rounds**, findings converging
  8→4→3→2→2→2→3→3→1 — note the *non-monotonic* tail, three rounds where the count went back
  up. PR 18b: 7 rounds including 2 fix-only rounds plus an orchestrator fix.
- Reverse-primitive cost measured: `claude -p` answers in 1.4 s but carries a
  ~25k-token fixed context per call (10 in + 2,695 cache-create + 22,307 cache-read)
  (`workflows-and-multi-model-orchestration.md:39`).

**Implication for role wording (INFERRED)**
- For an **Opus implementer** role: make concurrency and ownership an explicit, enumerated
  pre-submit checklist — single owner named, compare-and-swap on every commit to a shared
  file, no in-place rewrite of a file another process may be executing, lock-timeout paths
  return *unknown* rather than partial, file modes stated. Every one of those is a finding
  the record already produced.
- Add a **"fix the class, not the instance"** clause: when a reviewer names a defect, state
  in the reply which *other* call sites of the same shape you checked. Three PRs re-raised
  the same class across rounds.
- Add a **"re-measure every number you write"** clause for docs work, with the command that
  produced it. PR 19 is the direct evidence.
- For an **Opus reviewer** role: keep it aimed at unbounded loops, retry floods, destructive
  failure paths, blast radius, and abstraction leakage — that is where its findings
  actually land. Do not word it as a style or conformance reviewer.
- Its research output is long. If length is a cost concern, cap it by *section contract*
  (one section per capability slice), not by word count — the extra length in the grok
  report is where the load-bearing finding was.

---

### Codex gpt-5.6 — implementer, reviewer, researcher

**Strengths shown**
- **As reviewer against Opus, it is the state-machine and race finder.** PR 8 — per-pane
  focus, v7 acceptance on three reconnect paths, seed races, E2E vacuity, bridge auth,
  startup-fallback race (14 majors). PR 16 — logged-out misclassification, resume tied to
  live sessions, global default ignored, Restart bypass, root-override escapes, daemon
  outage masked as absence (17 majors). PR 10 — dropped final wall-clock interval, fallback
  accrual, protocol-9 break.
- **Claim verification at scale.** PR 19: Codex ×4 verification rounds over an Opus docs
  sweep of 35 files, 60+ findings, catching every wrong count. This is its single clearest
  comparative advantage in the record.
- **Throughput.** PR 18a — an entire harness integration (Antigravity registry, every
  capability slice, Gemini CLI removal everywhere) in **3 turns**, 4 rounds, 10 majors.
- Lowest-friction rows in the whole ledger are Codex's: PR 0 (1 round, 0 majors), PR 17c
  (3 rounds, 2 majors).

**Failure modes shown (with PR/round evidence)**
- **Fails hard where the spec requires fail-soft.** PR 7 "save aborted on unparseable
  config", "lead join fatal after commit"; PR 7b blocker "vanished pane as hard error",
  plus false-foreign detection and team-wide quarantine — blast radius from an over-strict
  guard.
- **Unbounded work.** PR 9 retry flood; PR 6 unbounded extractor maps; PR 13 per-tick sink
  parsing; PR 17b SQLite in the scanner hot path, fire-and-forget usage, usage-sync retry
  flood, plus a **second TLS stack** pulled into the graph.
- **Persisted-data and protocol discipline.** PR 18a blocker — persisted `gemini` values
  aborting whole records on upgrade — plus "protocol vocabulary without a bump" in the same
  PR. The upgrade path for data already on disk is the recurring blind spot.
- **Cross-platform reachability.** PR 6 "app-side compaction owner unreachable on
  Linux/macOS"; PR 13 "macOS-unreachable edge" and a probe bypassing the login shell/distro
  (×2 rounds); PR 18a "Windows hooks path".
- **Test honesty.** PR 1 "deleted regression guard"; PR 4 "tautological parity test" and a
  stale integration test left after a quoting change.
- **Destructive-on-failure.** PR 13 "failed probe deleting the installed hook".
- **Over-permissive parsing.** PR 17c — `identify()` invented a logged-in "API key" account
  from any parseable `auth.json`; duplicate ids from the `id_token` workspace claim crashed
  keyed lists.
- **Abstraction leakage inside the abstraction PR.** PR 15 — tool identity laundered through
  the `accent` token and `config_dir_env` used as the "is Claude" predicate, in the PR whose
  whole point was that nothing outside the registry may know a tool's name.
- **Round-trip data loss on the model/effort work.** PR 5a/5b — editor round-trip dropping
  effort, closed-catalog effort drop, catalog default effort overriding global (8 majors,
  4 rounds, orchestrator cut).

**Cost / duration signals**
- 15 PRs, 47 rounds, 95 majors — fewer rounds and fewer majors per PR than Opus, on a
  smaller average surface.
- Transport probes: whole probe set ~82k input tokens (~62k cached) / ~410 output on
  `gpt-5.4-mini`; `codex exec --json` "reply with OK" 8.9 s; `exec resume` 5.0 s; App Server
  ~9 s to first token (~5 s of it MCP startup)
  (`workflows-and-multi-model-orchestration.md:33-36`).
- **Deafness is a structural property, not a model trait:** `codex exec` has no stdin/IPC
  mid-turn, must run `< /dev/null` or it blocks on "Reading additional input from stdin…",
  and cannot ask a blocking question in non-interactive mode. `--ephemeral` threads cannot
  be resumed. Judge invocations in `judge.ts` use `--ephemeral` and feed the prompt on
  stdin, which is the correct shape for that constraint.
- Its research reports are the shorter, more heavily hedged of each pair: 18 UNVERIFIED /
  INFERRED markers in `agy-report-codex.md` vs 10 in the Opus one; 13 vs 12 for grok.

**Implication for role wording (INFERRED)**
- For a **Codex implementer** role: put the failure-handling contract in the role text
  itself — "a malformed or unreadable input degrades this record, never the operation";
  "every retry is bounded and every map has a cap"; "a failed probe never deletes installed
  state". Those three sentences map to five separate ledger findings.
- Add a standing clause: **"any change to a persisted format or a wire vocabulary requires
  a stated upgrade path for data already on disk and a protocol bump."** PR 18a's blocker
  and its missing bump are both this.
- Add: **"never delete or weaken an existing test; if a test must change, say which
  regression it guarded and how the new one still guards it."** PR 1 and PR 4 are the
  evidence.
- Word the platform clause concretely — Windows/WSL path forms, macOS login-shell
  invocation — rather than "cross-platform"; the misses are always one specific host.
- For a **Codex reviewer / researcher** role: aim it at **claim verification** — re-run
  every count, re-read every cited line, and report the delta. That is its measured
  strength (PR 19). Word its research role to require an explicit
  verified-vs-inferred marker per claim; it already does this more heavily than Opus and
  the habit is worth locking in.

---

### agy (Antigravity CLI, Gemini family)

**Evidence base is thin and must be stated as such.** `agy` landed on 2026-08-28 (PR 18a,
#39, CHANGELOG 0.8.0). It has **no ledger row as an implementer or reviewer** of any PR in
either plan. It appears only as a *subject harness* and as two role templates.

**Strengths shown**
- The one direct performance observation in the repo, from the archived design workflow
  (`docs/archive/design-workflow.md:9`), about the UI-specialist lane: *"It's a strong
  implementer"* — reliable at executing a specified change.
- As a harness it is genuinely coordinator-friendly (from `agy-report-opus.md`): a
  bidirectional NDJSON stream that keeps one conversation open across turns, first-party
  lifecycle hooks whose `Stop` carries `fullyIdle`, an flock-based presence registry.
- It is the only harness with a **command-backed usage provider** —
  `agy -p /usage --output-format json` — rather than an HTTP endpoint.

**Failure modes shown**
- **Goes visually generic under an over-specified brief.** `design-workflow.md:9`: when the
  UI specialist receives implementation-heavy specs ("build this component with these 7
  fields, these props, this layout") *"it produces functionally correct but visually generic
  output … won't spontaneously add the depth, micro-interactions, and visual grouping that
  make taurhaus feel premium. The fix is process, not tooling — give it design ownership,
  not just coding tasks."* The anti-pattern table (`:89`) names it: *"Over-specified brief →
  UI specialist becomes a code monkey, no design input → Give functional requirements, let
  it design."*
  **Caveat: that observation was written about the Gemini CLI UI specialist, before the
  Antigravity switch.** Carrying it to `agy` is **INFERRED**.
- **Capability gaps that constrain any role built on it**, from PR 18a: no compaction hook
  at all (`compaction_hook: false`, `cli_tool.rs:386-390`) — a long `agy` lane loses context
  at compaction with no reinjection; no account selector, one implicit account under
  `~/.gemini`; `session_dir` returns `None`, so a resume derives no account.

**Cost / duration signals**
- None measured. No token, round, or duration figure for `agy` as an actor exists in either
  ledger.

**Implication for role wording (INFERRED)**
- Word `agy` roles as **ownership grants, not specifications**. The one measured failure is
  caused by over-specification. `antigravity-ui-specialist.yaml` already does this well
  ("You are NOT a code-spec executor", "Never jump straight to implementation without an
  approved design") — that wording is the mitigation, and it should be preserved verbatim
  if the role is re-derived.
- Require **reviewable artifacts** in the role's definition of done — actual token values,
  wireframes, dark *and* light values — because the same anti-pattern table names "vague
  proposal" as the failure on the other side.
- Because it has **no compaction reinjection**, word `agy` roles for **bounded sessions**:
  a task scoped to complete inside one context, with its state written to a file rather than
  carried in the conversation. Do not word an `agy` role as a long-running resident lane.
- Do not word it as a reviewer of another family's work. There is zero evidence either way,
  and the cross-family review contract in `harness-model.md:71` is defined as Opus ↔ Codex.

---

### grok (Grok CLI, xAI)

**Evidence base is thinner still.** `grok` landed 2026-08-28 (PR 18b, #40). **No ledger row
as implementer or reviewer.** One role template, `grok-developer.yaml`
(`model: grok-4.6`, `reasoning_effort: high`), plus a "Grok Pair" preset.

**Strengths shown**
- Authoritative activity signal: busy/idle from the session's own `events.jsonl` turn
  lifecycle, and identity from `active_sessions.json` — better observability than the
  process heuristics the floor provides.
- `GROK_HOME` isolates an entire account — credentials, config, sessions, live registry,
  leader socket — which makes per-account isolation clean.
- Deliberately Claude-Code-shaped: reads `~/.claude/settings.json` hooks and
  `~/.claude/skills/`, emits Anthropic Messages wire-format NDJSON
  (`grok-report-opus.md`).

**Failure modes shown** — all harness-level, none behavioural:
- **Duplicate hook registration.** Because it also loads `~/.claude/settings.json`, one
  compaction can reach the bridge through two registrations; the registry declares
  `compaction_hook_compat_import` and the bridge deduplicates (`CHANGELOG.md:25`). Found by
  the Opus research report before implementation.
- **Passive-hook stdout is discarded**, and its start source never reports `compact` — so
  compaction cards must travel via the mesh inbox on `PostCompact`, unlike Claude/Codex's
  `hookSpecificOutput.additionalContext`.
- **No usage provider at all.** grok 1.0.5 publishes no quota endpoint (`/usage` is
  TUI-only, `grok usage` does not exist); cost and tokens arrive in-band per turn. Registry
  carries `usage: false` plus the sentence the UI shows in a meter's place.
- **Release debt, explicitly accepted:** "Grok compaction has no scripted end-to-end lane" —
  `just test-compaction` accepts `claude` and `codex` only; grok's unique path is verified by
  hand (`accounts-and-usage-plan.md:110`).
- A `GROK_HOME` store holding several records is read as **no** account, because grok's own
  selection rule is unverified.

**Cost / duration signals**
- None as an actor. One operational figure from PR 18b: grok is given a **15-second stop
  timeout** so its documented ten-second exit budget can run before the pane is killed.

**Implication for role wording (INFERRED)**
- Word `grok` roles as **short, self-contained implementation tasks with explicit
  verification steps** — which is exactly what `grok-developer.yaml` already does
  ("stay tightly scoped", "report exact verification steps and outcomes"). Its compaction
  path works but is manually verified only, so keep sessions inside one context where
  possible.
- Because there is **no usage meter**, any role wording that assumes budget awareness must
  not rely on the UI. If budget matters, put it in the role text as a turn/task cap.
- No evidence supports wording it as a reviewer or a lead. Keep it an implementer lane until
  a ledger row exists.

## 2.3 Two cross-cutting conclusions

1. **The cross-family review contract is load-bearing and is measurable.** Fable approving
   twice on PRs 2 and 3 did not prevent 5 majors that the other family then found. Every
   ledger row where the reviewer is the *other* family produced findings; the two rows where
   Fable reviewed produced none of its own. Any role wording that lets one family both
   implement and sign off contradicts the record.

2. **The two families fail in complementary, stable directions**, and role text should be
   written against the family's own failure direction rather than generically:
   Opus misses **concurrency, atomicity, ownership and unmeasured numbers**; Codex misses
   **degradation paths, bounds, persisted-data upgrades and one specific host**. Each is
   reliably caught by the other — which is precisely why the pairing works, and why a
   single-family loop would ship both classes.

