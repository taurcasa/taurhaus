# taureval harness inventory — adding `agy` and `grok`

Read-only inventory of `~/projects/taureval` (bun/TypeScript) plus the
taurhaus role templates it consumes. Every claim below is tagged with `file:line`
or captured command output. Inferences are marked **UNVERIFIED**.

Probe evidence retained at:
`/tmp/claude-1000/-home-mstie-projects-taurhaus/<uuid>/scratchpad/probe/{grok,agy}.json`

---

## 0. Headline correction to the task framing

The task brief assumes the eval runner drives subjects **headless**
(`-p ... --output-format json`). That is **not** how taureval evaluates roles.

- **Subject transport is interactive**: a real CLI is launched inside a **tmux
  split pane**, onboarded over the **mesh inbox**, and its answer is read back
  out of a JSON inbox file. `scoring/spawner.ts:444-447` (`createSplitPane` →
  `tmux split-window`), `scoring/spawner.ts:249-253` (`mesh send`),
  `scoring/spawner.ts:418-431` (`getUnreadMessagesFrom` reads
  `~/.claude/teams/<team>/inboxes/<lead>.json`).
- **Headless print mode is used only for the judge**: `claude -p --output-format
  json` (`scoring/judge.ts:78-89`) and `codex exec --json`
  (`scoring/judge.ts:109-123`).
- This was a deliberate, documented decision, not an oversight —
  `whats-next.md:148-149`: "Print mode (`claude -p`) for role evaluation
  (rejected by user) … User specifically wanted real interactive CLI sessions
  matching production". Same in `program.md:110-112`.

Consequence: the in-band usage/cost envelope that `grok` and `agy` expose in
headless mode is **unreachable through the current subject path**, because the
current subject path never runs headless. See §3 and §8.

---

## 1. Repo shape

```
~/projects/taureval
├── scoring/          runner, spawner, harness bridge, judge, db, cli (+ *.test.ts)
├── evals/            9 eval case sets (fixed ground truth)
├── roles/            STALE local mirror — NOT read by the runner (see §5.3)
├── results/          taureval.db (SQLite, gitignored)
├── program.md        optimizer loop instructions
├── whats-next.md     session handoff / current state
├── handoff-codex-v3-roles.md
└── CLAUDE.md         project background (partly aspirational/outdated, see §5.3)
```

Runtime: Bun/tsx, CommonJS. `package.json:6-13` scripts — `eval`, `eval:all`,
`eval:status`, `eval:failing`, `eval:progress`, `eval:log`, `test`
(`tsx --test scoring/*.test.ts`). Deps: `better-sqlite3`, `yaml`, `tsx`,
`typescript`.

Git: branch has 8e87a9b as HEAD; `scoring/spawner.ts` is modified in the working
tree (per-case **pane restart** replacing the old `/clear` reuse), and
`evals/{adversarial-reviewer,docs-verifier,quick-dev}.yaml`, `whats-next.md`,
`handoff-codex-v3-roles.md`, and 3 `roles/v3-*` files are untracked.

---

## 2. The runner — control flow

Entry: `scoring/cli.ts` → `runEvalMatrixForRole` / `runAllEvals`
(`scoring/runner.ts:591-649`).

```
cli.ts  run <role-id> [--model a,b] [--effort x,y] [--adapter p,q] [--runs n] [--timeout ms] [--desc s]
  └─ runEvalMatrixForRole(roleId, opts)                         runner.ts:591
       ├─ resolveEvalIdentity(roleId)                           runner.ts:246
       │    ├─ loadRoleDefinition  → reads TAURHAUS role yaml   loader.ts:94
       │    └─ getEvalForRole      → EVAL_TO_ROLE_MAP lookup    loader.ts:144
       ├─ resolveMatrixCells(role, opts)  → adapter×model×effort runner.ts:199
       └─ for each cell:
            ├─ resolveCellContext                               runner.ts:260
            │    ├─ adapterCliVersion(adapter)  → `<bin> --version`  harness.ts:159
            │    ├─ subjectFamily(adapter, model)                harness.ts:171
            │    └─ selectJudgeConfig(taskClass, family, seed)   judge-config.ts:48
            ├─ new EvalSession(adapter, role, {model, effort})   spawner.ts:62
            ├─ session.start()   ← tmux pane + mesh join + daemon + onboarding
            ├─ for each case × runsPerCase (default 3):
            │     ├─ session.runCase(prompt, {timeoutMs})        spawner.ts:204
            │     └─ judgeOutput(case, criteria, output, judge)  judge.ts:45
            ├─ majority vote per criterion                       runner.ts:150,156
            ├─ insertEvalRun / insertCaseResult                  db.ts
            └─ session.destroy()  (in `finally`)                 runner.ts:586-588
```

### 2.1 How claude / codex are actually invoked

The launch command string is **not** built in TypeScript. It is rendered by the
real taurhaus binary so eval bytes match production bytes
(`scoring/harness.ts:61-83`, `invokeTaurhaus`), which shells out to:

```
$TAURHAUS_BIN --launch-command -      # stdin JSON → stdout JSON
$TAURHAUS_BIN --render-onboarding -   # stdin JSON → stdout text
```

`TAURHAUS_BIN` defaults to
`<taureval>/../taurhaus/src-tauri/target/debug/taurhaus` (`harness.ts:13-15`).
Verified present and current: `448569632` bytes, mtime `Aug 28 19:28`.

There is a regression test asserting golden bytes —
`scoring/harness.test.ts:6-56` ("golden launch and onboarding bytes come from the
real taurhaus binary"), documenting commit `7b852ed` as the regression where
TypeScript duplicated `LaunchSpec`/`DeliveryRenderer`.

`ADAPTER_CONFIG` (`harness.ts:17-42`) — the table a new harness must join:

| adapter | binary | rendererTool | sandbox | base |
|---|---|---|---|---|
| `claude-code` | `claude` | `claude` | `bypassPermissions` | — |
| `codex-cli` | `codex` | `codex` | `danger-full-access` | — |
| `pi` | `pi` | `null` | `pi-no-approve` | `pi --no-approve` |

`pi` is the existing precedent for a **locally-rendered** adapter: when
`adapter === "pi"`, `renderLaunchCommand` bypasses the taurhaus binary entirely
and string-builds `<base> --model '<m>' --thinking '<e>'`
(`harness.ts:91-98`).

**Matrix-cell rejection**: after rendering, the runner inspects `notes[]` and
throws if taurhaus reports `launch.model.invalid`, `launch.model.ignored`,
`launch.effort.ignored`, or `launch.effort.invalid` (`harness.ts:124-141`). This
prevents recording a cell as "effort=max" when the CLI silently ran at default.
Tested at `harness.test.ts:58-84`.

### 2.2 Session lifecycle (`spawner.ts`)

Two transports, keyed off `usesMesh()` = `adapter !== "claude-code"`
(`spawner.ts:329-331`):

| step | claude-code | everything else (mesh path) |
|---|---|---|
| team config | write `~/.claude/teams/<team>/config.json` (`spawner.ts:350`) | same |
| launch | bash script in tmux split pane (`spawner.ts:106-107`, `473-488`) | same |
| membership | `mesh join` lead only, fallback = write empty inbox (`spawner.ts:141-150`) | `mesh join` lead + member (`spawner.ts:116-124`) |
| daemon | none | `mesh daemon --pane <id> --team --name --mark-read`, detached (`spawner.ts:127-131`) |
| onboarding | direct inbox file write (`spawner.ts:152`) | `mesh send … --summary operator_notice` (`spawner.ts:320-327`) |
| readiness | fixed `sleepMs(10_000)` (`spawner.ts:154`) | `waitForAck` polls lead inbox up to 60s (`spawner.ts:310-318`) |
| task | inbox write (`spawner.ts:256`) | `mesh send … --summary task_assignment` (`spawner.ts:249-253`) |

Constants: `MESH_BIN = ~/.local/bin/mesh` (`spawner.ts:16`), `TEAMS_DIR =
~/.claude/teams` (`spawner.ts:15`).

Per-case isolation (uncommitted change): every case after the first kills and
re-creates the pane, blanks the member inbox, and restarts the mesh daemon
(`spawner.ts:159-201`, called at `spawner.ts:214-217`).

### 2.3 Result parsing

There is no structured envelope to parse. The "result" is whatever the agent
messages back to the evaluator's inbox:

- Poll loop `waitForAgentResponse` (`spawner.ts:492-562`), interval 3s
  (`POLL_INTERVAL_MS`, `spawner.ts:19`).
- Messages present before task delivery are checkpointed by id and ignored
  (`spawner.ts:240-245`).
- Bare acknowledgements are filtered by regex + a 160-char floor —
  `classifyAgentMessage` (`spawner.ts:564-569`).
- Surviving messages are joined with `\n\n` and returned as `output`
  (`spawner.ts:522,531,559`).
- Pane death mid-wait short-circuits: returns any substantive message, else
  `timedOut` (`spawner.ts:517-525`).

### 2.4 Timeouts

| timeout | value | site |
|---|---|---|
| per case | 180 000 ms (`--timeout` overrides) | `spawner.ts:17`, `cli.ts:102` |
| nudge ("status check" resend) | 90 000 ms | `spawner.ts:19`, `spawner.ts:534-550` |
| onboarding ack wait | 60 000 ms | `spawner.ts:138,228` |
| judge subprocess | 120 000 ms | `judge.ts:87,120` |
| CLI `--version` probe | 10 000 ms | `harness.ts:164` |
| `mesh send` | 30 000 ms | `spawner.ts:253,326` |
| `git rev-parse` | 5 000 ms | `runner.ts:133` |

A timeout with no output raises `EvalTimeoutError` (`spawner.ts:271-276`) and is
recorded as `error_class='timeout'`, **not** as a failed criterion
(`runner.ts:226-228`, `runner.ts:550-568`). `program.md:107-108` states this
explicitly: "A harness, judge, or timeout error is an infrastructure
measurement, never a failed criterion."

### 2.5 Cost / usage capture — currently always NULL

`SpawnResult` carries `costUsd`, `inputTokens`, `outputTokens`
(`spawner.ts:20-26`), the DB has `cost_usd REAL`, `input_tokens INTEGER`,
`output_tokens INTEGER` (`db.ts`, `case_results`), and the runner sums them with
`sumNullable` (`runner.ts:216-219`, `543-545`).

But the spawner hard-codes them to `null` with a comment
(`spawner.ts:281-285`):

> Interactive tmux panes expose no structured subject usage envelope.
> Keep these NULL; judge-process metrics would measure a different model.

So the plumbing is complete end-to-end and only the **producer** is missing.

---

## 3. Judge

`scoring/judge.ts` — headless, and the one place print mode is legitimate.

- **anthropic family** → `claude -p --output-format json --model <m> --effort <e>
  --append-system-prompt <sys> <user>`; parses `JSON.parse(raw).result`
  (`judge.ts:78-94`).
- **openai family** → `codex exec --json --ephemeral --skip-git-repo-check
  --sandbox read-only --model <m> -c model_reasoning_effort="<e>" -`, prompt on
  stdin; filters NDJSON for `type === "item.completed" && item.type ===
  "agent_message"` (`judge.ts:109-131`).
- Output format is line-oriented `CRITERION:` / `VERDICT: PASS|FAIL` / `REASON:`
  (`judge.ts:34-42`), parsed at `judge.ts:192-246`. A judge that **omits** a
  criterion throws `JudgeExecutionError` (`judge.ts:235-243`) → recorded as
  `error_class='judge'`, not a fail.

**Judge independence rotation** (`judge-config.ts`): 4 judges
(`claude-opus-4-6`, `claude-sonnet-4-5`, `gpt-5.6-sol`, `gpt-5.6-terra`), a
per-task-class ordering (`judge-config.ts:30-38`), then
`selectJudgeConfig` **filters out judges sharing the subject's family** and picks
by a stable hash of `roleId:contentHash:adapter:model:effort`
(`judge-config.ts:48-60`, seed built at `runner.ts:276-280`).

`subjectFamily(adapter, model)` (`harness.ts:171-178`): `claude`-in-model or
`claude-code` adapter → `anthropic`; `gpt`/`codex` in model or `codex-cli`
adapter → `openai`; otherwise `other`.

> Note this already generalizes correctly for the new harnesses: `grok-4.6` →
> `other` (all 4 judges stay eligible), and an `agy` cell running
> `claude-opus-4-6-thinking` → `anthropic` (Anthropic judges correctly excluded).
> `gpt-oss-120b-medium` → `openai`. No change required in `judge-config.ts`.

---

## 4. Scoring

- **Binary per criterion**, no scales (`CLAUDE.md:126-127`).
- **3 runs per case, majority vote** — `RUNS_PER_CASE = 3` (`runner.ts:30`),
  `majorityVote` requires strictly `passes > votes.length / 2`
  (`runner.ts:150-154`); ties → 0.
- `scoreCaseObservations` (`runner.ts:156-186`) splits observations into
  successful vs error, votes only over successful runs, and attaches the first
  failing run's `failure_reason`.
- `total_score` = passing criteria; `max_score` = **measured** criteria only —
  criteria with `passed === null` (all runs errored) are excluded from both
  (`runner.ts:511-514`). Score denominators therefore shrink under infra failure
  rather than silently counting as fails.
- **Persistence** (`db.ts`): `role_versions` (role_id, yaml_content, commit_hash)
  → `eval_runs` (total/max, status `keep|discard|baseline`, `run_config` JSON,
  `cli_version`, `content_hash`) → `case_results` (passed nullable,
  `failure_reason`, `error_class`, `duration_ms`, raw subject + judge output,
  `judge_config`, cost/token columns, `task_class`, `split`).
- **Views**: `v_current_scores`, `v_failing_cases`, `v_role_progress`,
  `v_improvement_log`. Currency is keyed on the **matrix cell**
  (`role_id` + `model` + `reasoning_effort` + `adapter`) — deliberately *not* on
  `cli_version` or judge, which are provenance (comment in `db.ts`, and commit
  `8e87a9b` "Key eval currency on the matrix cell, not the run provenance").
- **Cell-level error containment**: a cell that fails to resolve is recorded as a
  synthetic case `__matrix_cell__` / `__harness_error_1` and the sweep continues
  (`recordCellError`, `runner.ts:349-407`; `fallbackCellErrorContext`,
  `runner.ts:312-347`). Regression-tested in `runner.test.ts` around a missing
  adapter CLI.

---

## 5. Role schema and naming

### 5.1 Schema consumed by taureval

`RoleDefinition` (`loader.ts:21-31`) — intentionally partial; the full document
stays owned by taurhaus and is passed through unchanged:

```ts
interface RoleDefinition extends Record<string, unknown> {
  role_id: string
  name: string
  instructions: string
  defaults: {
    cli_tool: "claude" | "codex"        // ← the blocker, see §8
    model: string
    reasoning_effort?: string
    default_name_pattern: string
  }
}
```

The full taurhaus role template (e.g. `grok-developer.yaml`) also carries:
`schema.kind: role_template`/`version: 1`, `version`, `kind: agent`,
`focus_area`, `context_summary`, `behavior_summary`, `communication_style`,
`quality_gates[]`, `definition_of_done[]`, `phase_scope[]`, `mode`,
`required_artifacts[]`, `handoff_expectations[]`,
`behavioral_contract.{communication,execution,escalation}[]`, `capabilities[]`,
`constraints.{min_instances,max_instances,requires_lead_tool,allowed_project_binding}`.
Some roles add `runtime_compact_summary`.

Only `role_id`, `instructions`, and `behavioral_contract` are copied into the
team config members array (`spawner.ts:376-380`); the whole role object is
forwarded to the taurhaus onboarding renderer (`harness.ts:145-153`).

### 5.2 Naming convention

`{version}-{role}-{family}` for the optimization line — `v2-*`/`v3-*` prefix,
`-claude`/`-codex` suffix naming the **model family**, e.g.
`v3-developer-codex`, `v3-product-checker-claude`. The newer holdout roles drop
the version prefix: `adversarial-reviewer-claude`, `docs-verifier-codex`,
`quick-dev-codex`. `LEGACY_ROLE_IDS` (`loader.ts:88-92`) rewrites the `v3-`
forms found inside eval YAML `role:` fields onto the unprefixed ids.

Family suffix is **naming only** — the actual harness binding comes from
`defaults.cli_tool` via `defaultAdapter()` (`runner.ts:188-190`).

### 5.3 Source of truth — `roles/` is stale

`rolesDir()` (`loader.ts:6-11`) resolves to
`$TAURHAUS_REPO/src-tauri/resources/templates/roles`, defaulting to
`../taurhaus/...`. **The local `taureval/roles/` directory is never read by the
runner.** Verified by diffing all 19 files: 16 `DIFFERS`, 3 exist only in
taureval (`v3-adversarial-reviewer-claude`, `v3-docs-verifier-codex`,
`v3-quick-dev-codex` — resolved in taurhaus under their unprefixed names). Zero
files are identical.

`README.md:9-16`, `program.md:13-17`, and `whats-next.md:186-188` all state the
taurhaus-owns-roles model correctly. `CLAUDE.md:36-43` still describes
`roles/` as "Mutable role definitions" and `scoring/` as "(to be built)" — it is
outdated relative to the code.

Other env overrides: `TAUREVAL_EVALS_DIR` (`loader.ts:13-17`),
`TAUREVAL_DB_PATH` (`db.ts:6-10`), `TAURHAUS_BIN` / `TAURHAUS_REPO`
(`harness.ts:10-15`), `TAURHAUS_DATA_DIR` (`harness.ts:67-69`).

---

## 6. The eval matrix

`EVAL_TO_ROLE_MAP` (`loader.ts:64-74`) + `EVAL_METADATA` (`loader.ts:76-86`),
case/criterion counts measured from the YAML:

| eval | role(s) | task_class | split | cases | crit/case | max |
|---|---|---|---|---|---|---|
| product-checker | v3-product-checker-claude | product-review | dev | 13 | 3 | 39 |
| design-lead | v3-design-lead-claude | design-review | dev | 7 | 3 | 21 |
| developer-claude | v3-developer-claude | implementation | dev | 9 | 3 | 27 |
| developer-codex | v3-developer-codex | implementation | dev | 7 | 3 | 21 |
| architect | v3-architect-codex, v3-architect-claude | architecture-review | dev | 9 | 2 | 18 |
| lead | v3-lead-codex, v3-lead-claude | coordination | dev | 14 | 2 | 28 |
| adversarial-reviewer | adversarial-reviewer-claude | code-review | **holdout** | 10 | 3 | 30 |
| docs-verifier | docs-verifier-codex | documentation | **holdout** | 10 | 3 | 30 |
| quick-dev | quick-dev-codex | implementation | **holdout** | 10 | 3 | 30 |

**9 eval sets, 10 role bindings, 89 cases, 244 max criteria.** At the default
3 runs/case that is 267 subject invocations for a full `run-all` on default
cells.

### 6.1 Which roles × which harnesses

There is **no** declared role×harness matrix table. The default cell is derived
per role:

```ts
function defaultAdapter(cliTool: "claude" | "codex"): Adapter {
  return cliTool === "claude" ? "claude-code" : "codex-cli";   // runner.ts:188-190
}
```

`resolveMatrixCells` (`runner.ts:199-214`) then produces the cross product
`adapters × models × efforts`, each dimension falling back to the role's own
`defaults` when the CLI flag is absent. So:

- default sweep: `*-claude` roles → `claude-code`; `*-codex` roles → `codex-cli`.
- `--adapter a,b --model m,n --effort x,y` expands one role across `2×2×2 = 8`
  cells, all scored independently and stored under distinct `run_config`s.
- Legacy `"<model> <effort>"` strings are split by `splitLegacyModel`
  (`runner.ts:192-197`), which accepts `low|medium|high|xhigh|max|ultra`.

`--adapter` is allow-listed in the CLI: `["claude-code", "codex-cli", "pi"]`
(`cli.ts:113`) — a hard gate a new adapter must pass.

### 6.2 Dev / holdout discipline

`program.md:46-47,106-108`: edit against dev results only, never read holdout
case text while editing, run holdout cells only after the role text is frozen,
report the two splits separately. `split` is persisted per `case_result` row.

---

## 7. Verified facts about `agy` and `grok`

### 7.1 Binaries and versions

```
~/.local/bin/agy    → 1.1.22
~/.local/bin/grok   → grok 1.0.5 (5115b46bc9) [stable]
```

Both answer `--version` on stdout, so `adapterCliVersion` (`harness.ts:159-169`)
works unmodified once the binary is registered.

### 7.2 Headless flags — one correction to the brief

| | agy | grok |
|---|---|---|
| prompt flag | `-p` / `--print` / `--prompt` | `-p` / **`--single`** (not `--print`) |
| output formats | `text`, `json`, `stream-json` | `plain`, `json`, `streaming-json`, **`streaming-messages-json`** |
| approval | `--dangerously-skip-permissions` | `--always-approve` |
| effort | `--effort low\|medium\|high` (**no xhigh**) | `--effort` (alias of `--reasoning-effort`) |
| timeout | `--print-timeout` (default 5m0s) | — |
| models | `agy models` → 14 | `grok models` → 2 |

Effort vocabularies confirmed in taurhaus:
`AGY_EFFORTS = ["low","medium","high"]`,
`GROK_EFFORTS_THROUGH_XHIGH = ["low","medium","high","xhigh"]` (grok-4.6),
`GROK_EFFORTS_THROUGH_HIGH` (grok-4.5) —
`taurhaus/src-tauri/src/models/mod.rs:711-715`. So **xhigh is grok-4.6-only**;
requesting it on agy or grok-4.5 should be rejected as an invalid cell.

`agy models` output (14): `gemini-3.7-flash-{high,medium,low}`,
`gemini-3.6-flash-{high,medium,low}`, `gemini-3.5-flash-{high,medium,low}`,
`gemini-3.1-pro-{high,low}`, `claude-sonnet-4-6`, `claude-opus-4-6-thinking`,
`gpt-oss-120b-medium`. Note agy model ids **embed the effort tier**.
`grok models` (2): `grok-4.6` (default), `grok-4.5`.

### 7.3 Headless envelopes — measured

`grok -p "Reply with exactly: OK" --output-format json --model grok-4.6 --effort low --always-approve`:

```json
{ "text": "OK", "stopReason": "end_turn", "sessionId": "...", "requestId": "...",
  "thought": "...",
  "usage": { "input_tokens": 8636, "cache_read_input_tokens": 5760,
             "cache_creation_input_tokens": 0, "output_tokens": 19,
             "reasoning_tokens": 14, "total_tokens": 14415 },
  "num_turns": 1,
  "total_cost_usd": 0.00344522, "total_cost_usd_ticks": 34452200,
  "modelUsage": { "grok-4.6-build": { "inputTokens": 8636, "outputTokens": 19,
                  "cacheReadInputTokens": 5760, "cacheCreationInputTokens": 0,
                  "modelCalls": 1, "costUSD": 0.00344522 } } }
```

`agy -p "Reply with exactly: OK" --output-format json --model gemini-3.7-flash-low --dangerously-skip-permissions`:

```json
{ "conversation_id": "...", "status": "SUCCESS", "response": "OK\n",
  "duration_seconds": 1.61147506, "num_turns": 1,
  "usage": { "input_tokens": 13874, "output_tokens": 1, "thinking_tokens": 0,
             "cache_read_tokens": 0, "total_tokens": 13875 } }
```

**Mapping onto `SpawnResult`:**

| field | grok | agy |
|---|---|---|
| `output` | `.text` | `.response` |
| `inputTokens` | `.usage.input_tokens` | `.usage.input_tokens` |
| `outputTokens` | `.usage.output_tokens` | `.usage.output_tokens` |
| `costUsd` | `.total_cost_usd` | **absent — must stay `null`** |

The brief's claim of in-band cost holds for **grok only**. agy reports tokens but
no dollar figure. Deriving agy cost from tokens would require a price table
taureval does not have — leave `costUsd` NULL rather than fabricate it.

### 7.4 taurhaus already speaks agy and grok

`CliTool` is a 5-variant enum including `Agy` and `Grok`
(`taurhaus/src-tauri/src/session_scanner/cli_tool.rs:17-22`), with catalog,
efforts, account/usage providers, idle resolvers and transcript locators wired
per tool. Both `--launch-command` and `--render-onboarding` take a typed
`CliTool` (`taurhaus/src-tauri/src/lib.rs:326`, `:353`), so `"agy"` / `"grok"`
deserialize natively.

**Live probe of the production renderer** (temp `TAURHAUS_DATA_DIR`, since
removed):

```
claude → {"command":"CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 claude --dangerously-skip-permissions --model 'claude-opus-4-6' --team-name 'taureval-probe' --agent-name 'agent-under-test' --agent-id 'agent-under-test@taureval-probe' --agent-type 'general-purpose' -n 'agent-under-test'","notes":[]}
codex  → {"command":"codex --yolo -m 'gpt-5.4' -c 'model_reasoning_effort=\"high\"'","notes":[{"event":"launch.model.deprecated","found":"gpt-5.4","replacement":"gpt-5.6-terra"}]}
agy    → {"command":"agy --dangerously-skip-permissions --model 'gemini-3.7-flash-high'","notes":[]}
grok   → {"command":"grok --always-approve --model 'grok-4.6' --effort 'high'","notes":[]}
```

`--render-onboarding` with `tool: "agy"` and `tool: "grok"` both emit the
mesh-style `[taurhaus] onboarding` block (identical shape to codex, with `mesh
read` / `mesh send` / `mesh task …` command lines) — **not** the claude-native
`[taurhaus] role_context` block. That is exactly what the spawner's
`usesMesh()` branch expects.

**So the production renderer needs no change at all.** agy and grok can be
first-class `rendererTool` adapters like claude and codex — they do *not* need
the local string-building `pi` treatment.

---

## 8. Gap analysis — what actually blocks agy/grok today

| # | gap | site | severity |
|---|---|---|---|
| 1 | `cli_tool` union is `"claude" \| "codex"` | `loader.ts:24` | type-level; blocks compile of any new branch |
| 2 | `defaultAdapter` is a binary ternary — **`agy` and `grok` silently resolve to `codex-cli`** | `runner.ts:188-190` | **silent wrong harness**, worst failure mode |
| 3 | `Adapter` union lacks the new variants | `harness.ts:7` | type-level |
| 4 | `ADAPTER_CONFIG` has no agy/grok rows (binary, sandbox, rendererTool) | `harness.ts:17-42` | blocks version probe + launch |
| 5 | `rendererTool` typed `"claude" \| "codex" \| null` | `harness.ts:22` | type-level |
| 6 | `renderOnboarding` hard-codes the tool: `adapter === "claude-code" ? "claude" : "codex"` | `harness.ts:148` | **would onboard an agy agent as codex** |
| 7 | CLI `--adapter` allow-list rejects the new names | `cli.ts:113` | hard gate |
| 8 | no effort validation per tool (agy has no `xhigh`) | — | relies on taurhaus `notes[]` rejection (§2.1) — likely already covered, **UNVERIFIED** |
| 9 | subject cost/usage always NULL | `spawner.ts:281-285` | pre-existing, orthogonal |

Gap 2 is the dangerous one: taurhaus **already ships** roles with `cli_tool: agy`
and `cli_tool: grok`, so `bun run eval grok-developer` today would launch a
**codex** session and record it as `adapter: "codex-cli"` — a plausible-looking
but wrong row. Note those roles are not in `EVAL_TO_ROLE_MAP`, so they are not
reachable via `run-all` yet; the risk materializes the moment a mapping is added.

`subjectFamily` (`harness.ts:171-178`), `judge-config.ts`, `db.ts`, and the
`v_*` views need **no** change — `Adapter` is stored as an opaque string and
`other` is already a valid family.

---

## 9. Minimal first step

Ordered smallest-to-largest, stopping at the first genuinely useful milestone.

### Step 1 — runner adapter (the whole change is ~6 edits, all in 4 files)

1. `harness.ts:7` — `export type Adapter = "claude-code" | "codex-cli" | "pi" | "agy-cli" | "grok-cli"`.
2. `harness.ts:22` — widen `rendererTool` to `"claude" | "codex" | "agy" | "grok" | null`.
3. `harness.ts:17-42` — two `ADAPTER_CONFIG` rows:
   - `"agy-cli": { binary: "agy", rendererTool: "agy", sandbox: "dangerously-skip-permissions" }`
   - `"grok-cli": { binary: "grok", rendererTool: "grok", sandbox: "always-approve" }`
   (sandbox strings are free-form provenance labels; pick ones that name the real
   flag, matching how `pi` uses `"pi-no-approve"`.)
4. `harness.ts:145-153` — `renderOnboarding` must use
   `ADAPTER_CONFIG[request.adapter].rendererTool` instead of the
   claude/codex ternary. **This is the one genuinely bug-shaped edit** — without
   it an agy agent gets a codex onboarding.
5. `loader.ts:24` — `cli_tool: "claude" | "codex" | "agy" | "grok"`.
6. `runner.ts:188-190` — replace the ternary with an explicit map:
   `{ claude: "claude-code", codex: "codex-cli", agy: "agy-cli", grok: "grok-cli" }`.
7. `cli.ts:113` — extend the allow-list (and the `--adapter` help line at
   `cli.ts:37`).

No spawner change is required: `usesMesh()` already returns true for anything
that is not `claude-code` (`spawner.ts:329-331`), and §7.4 confirms taurhaus
renders the mesh onboarding block for both new tools.

**Extend `harness.test.ts:6` with golden bytes**, which are already measured:

```
agy  → agy --dangerously-skip-permissions --model 'gemini-3.7-flash-high'
grok → grok --always-approve --model 'grok-4.6' --effort 'high'
```

and add a `resolveMatrixCells` case to `runner.test.ts:37` proving
`cli_tool: grok` → `grok-cli` (guarding gap 2 permanently).

### Step 2 — one role variant each

The cheapest honest variant is a family port of an existing eval, so the new
harness is measured against a known-answer set rather than a new one:

- **grok**: `grok-developer` already exists in taurhaus with
  `cli_tool: grok`, `model: grok-4.6`, `reasoning_effort: high`. Map it into
  `EVAL_TO_ROLE_MAP` under the existing `developer-codex` or `developer-claude`
  eval — it is `task_class: implementation`, so the judge rotation is already
  configured. Zero new YAML required.
- **agy**: no developer-shaped agy role exists. The three agy roles are
  `antigravity-orchestrator`, `antigravity-ui-specialist`, `taurhaus-designer`.
  `antigravity-orchestrator` maps naturally onto the existing `lead` eval
  (`task_class: coordination`); alternatively author `v3-developer-agy.yaml`
  as a `cli_tool: agy` copy of `v3-developer-claude`, matching how the codex
  baselines were created (`whats-next.md:63-71`).

Either way this is a `loader.ts:64-74` map edit plus, at most, one new role
YAML **in taurhaus** (the source of truth — never in `taureval/roles/`).

### Step 3 — smoke, then baseline

```bash
bun run test                                   # golden bytes + matrix cells
bun run eval grok-developer --adapter grok-cli --model grok-4.6 \
  --effort high --runs 1 --desc "grok harness smoke"
bun run eval:status grok-developer
```

`--runs 1` first (cheap, proves transport), then `--runs 3` for a
noise-smoothed baseline. Expect the first run to surface pane-timing issues:
`start()` sleeps a fixed 10s before checking liveness (`spawner.ts:108-112`) and
the mesh path then waits on an ack — a slower-booting CLI will need that constant
raised. `whats-next.md:111-113` records exactly this class of bug for codex.

### Step 4 (optional, separable) — headless cost lane

Only worth doing if per-cell cost/usage is a goal in itself. It is a **second
transport**, not a tweak: add a headless `runCase` path that shells
`grok -p <prompt> --output-format json …` / `agy -p <prompt> --output-format json …`,
parses the envelope per §7.3, and populates the already-existing
`costUsd`/`inputTokens`/`outputTokens` fields and DB columns.

Caveat worth stating up front: this measures a **different subject** than the
interactive lane — no mesh inbox, no tmux pane, no onboarding round-trip, and
`whats-next.md:148-149` records that print-mode evaluation was explicitly
rejected by the user. Treat it as a separate adapter (e.g. `grok-headless`) so
matrix-cell currency keeps the two apart, rather than changing what `grok-cli`
means.

---

## 10. taurhaus role templates mirrored by taureval

`~/projects/taurhaus/src-tauri/resources/templates/roles/` — **39
files**. `cli_tool` / `model` / `reasoning_effort` read from each `defaults:`
block.

### cli_tool: agy (3)

| role | model | effort | name pattern |
|---|---|---|---|
| antigravity-orchestrator | gemini-3.7-flash-high | null | lead-{project} |
| antigravity-ui-specialist | gemini-3.7-flash-high | null | ui-specialist-{n} |
| taurhaus-designer | gemini-3.7-flash-high | null | designer-{n} |

### cli_tool: grok (1)

| role | model | effort | name pattern |
|---|---|---|---|
| **grok-developer** | grok-4.6 | high | dev-{n} |

### cli_tool: claude (18)

adversarial-reviewer-claude · claude-design-lead · claude-orchestrator ·
claude-product-checker · claude-researcher (`claude-sonnet-4-5`) ·
claude-reviewer · frontend-design-skill-developer · taurhaus-lead-claude ·
v2-architect-claude · v2-design-lead-claude · v2-developer-claude ·
v2-lead-claude · v2-product-checker-claude · v3-architect-claude ·
v3-design-lead-claude · v3-developer-claude · v3-lead-claude ·
v3-product-checker-claude

All `claude-opus-4-6`, `reasoning_effort: null`, except `claude-researcher`
(`claude-sonnet-4-5`).

### cli_tool: codex (17)

codex-architect · codex-developer · codex-orchestrator · codex-product-lead ·
codex-qa · codex-vertical-slice-developer · docs-verifier-codex ·
quick-dev-codex · taurhaus-architect · taurhaus-developer · taurhaus-lead-codex ·
v2-architect-codex · v2-developer-codex · v2-lead-codex · v3-architect-codex ·
v3-developer-codex · v3-lead-codex

All `gpt-5.4`; `reasoning_effort: high` except `v3-architect-codex`,
`v3-developer-codex`, `v3-lead-codex` which are `null`.

> `gpt-5.4` is flagged deprecated by the live renderer:
> `{"event":"launch.model.deprecated","found":"gpt-5.4","replacement":"gpt-5.6-terra"}`.
> Deprecation is a **note**, not one of the four blocking events
> (`harness.ts:124-129`), so those cells still run — but they run on a model the
> catalog considers superseded. Worth a separate decision.

### Of these 39, only 11 are wired into taureval

`adversarial-reviewer-claude`, `docs-verifier-codex`, `quick-dev-codex`,
`v3-architect-claude`, `v3-architect-codex`, `v3-design-lead-claude`,
`v3-developer-claude`, `v3-developer-codex`, `v3-lead-claude`,
`v3-lead-codex`, `v3-product-checker-claude` (`loader.ts:64-74`). The `v2-*`,
`taurhaus-*`, `codex-*`, `claude-*`, agy and grok roles have **no eval mapping**
and are unreachable from the CLI (`getAllRoleIds`, `loader.ts:138-142`, derives
its list from the same map).

---

## 11. Caveats and unverified items

- **UNVERIFIED**: that an agy/grok agent actually drives the mesh loop — reads
  its inbox, calls `mesh send`, and reports back within 180s. Only the *rendering*
  was probed, not a live session. Verify with the Step 3 smoke run and watch the
  tmux pane.
- **UNVERIFIED**: that `mesh join --model grok-4.6` / `--model
  gemini-3.7-flash-high` is accepted (`spawner.ts:121-124`). Verify:
  `mesh join --team <tmp> --name x --type general-purpose --model grok-4.6`.
- **UNVERIFIED**: that taurhaus rejects `--effort xhigh` for agy via a blocking
  `launch.effort.invalid` note. Verify by re-running the `--launch-command` probe
  with `"tool":"agy","reasoningEffort":"xhigh"` and checking `notes[]`.
- **UNVERIFIED**: whether `agy --print-timeout` (default 5m) interacts with the
  interactive pane path — likely print-mode-only, so irrelevant to Step 1.
- The two headless probes in §7.3 were real billed calls (grok reported
  `total_cost_usd: 0.00344522`; agy reports no cost). They ran with cwd set to a
  scratchpad subdirectory, and each CLI wrote its own session/transcript state
  under its own home directory as a normal side effect of running. No repo file
  and no CLI config file was modified.
- No process was left running: every command was foreground with an explicit
  `timeout`. The temporary `TAURHAUS_DATA_DIR` used for renderer probes was
  deleted.
