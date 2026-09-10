# Harness Model

How taurhaus relates to the AI CLIs it runs (Claude Code, Codex CLI, Antigravity CLI, Grok CLI), what it owns itself, and the rules that keep the two in step. This is the architecture that landed in 0.6.4–0.8.0 (2026-08); the execution records are [`harness-realignment-plan.md`](../design/harness-realignment-plan.md) (slices, stability) and [`accounts-and-usage-plan.md`](../design/accounts-and-usage-plan.md) (accounts, usage, the Antigravity and Grok integrations), with the per-CLI research reports under [`research/`](../design/research/).

## The constraint that shapes everything

Claude models on a subscription are only reachable through the Claude Code CLI. Codex and Antigravity have their own CLIs. So there is no single harness for all models: **Claude Code is the Claude harness, the other CLIs are theirs, and taurhaus coordinates them from outside** — tmux panes for processes, the mesh bridge for team messaging between CLIs. The rule that follows, and that every subsystem applies:

> Use what the harness does natively where it exists; tmux and mesh where it does not.

tmux + mesh is the *floor*: as long as a model has any CLI, taurhaus can reach it. Native capabilities (a sessions registry, hooks, a turn-complete notification, a peer message queue) are per-tool *upgrades* over that floor, never replacements for it.

## Capability slices, not tool adapters

Adding a CLI must touch only the slices where that tool differs; the rest of the system consumes capabilities and never branches on tool identity. Each slice has a declared default when a tool provides nothing:

| Slice | Claude Code | Codex CLI | Antigravity CLI (`agy`) | Grok CLI (`grok`) | Default (floor) |
|---|---|---|---|---|---|
| Registry entry | data | data | aliases `agy`/`antigravity`; Google-blue accent | alias `grok`; graphite accent | required |
| Process signature | argv pattern | argv pattern | interactive `agy` argv; print/subcommands excluded | interactive `grok` argv; `-p`/`--single`/`--prompt-file`/`--prompt-json`, `agent` services and management subcommands excluded | tool invisible, logged once |
| Launch rendering | `--model`, `--effort`, `-n <agent>`, team flags | `-m` (`--model` alias), reasoning config, notify + managed-hook trust flags | `--model`, `--effort`, `--dangerously-skip-permissions`; conversation resume | `--model` (`-m` alias), `--effort`, `--always-approve`; `--continue` / `--resume <uuid>` | user's base command verbatim; every accepted flag spelling is registry data (`model_flag` + `model_flag_aliases`, `effort_flag` + `effort_flag_aliases`, `deprecated_flags`) |
| Session identity | sessions registry | fd-verified rollout binding | `last_conversations.json` + presence lock | `active_sessions.json` row bound by pid and cwd | `/proc` + tmux pane, no session id |
| Busy / idle | registry status (authoritative) | turn-complete notify (authoritative) | hooks sink, default on, needs workspace trust and agy 1.1.10; process-IO floor otherwise | `events.jsonl` turn lifecycle (authoritative) | rchar-rate hysteresis + pane liveness |
| Message delivery + wake | native inbox poller | inbox + tmux wake | inbox + tmux wake | inbox + tmux wake (plain Enter queues, Ctrl+Enter interjects) | inbox + tmux wake |
| Compaction signal | `SessionStart(source=compact)` hook | hosted: completed `contextCompaction` on the owned connection, deduplicated with hooks; pane: managed `SessionStart(source=compact)` hook by default on Codex >= 0.147; older versions log `compaction.codex_hook.unsupported` once and receive no reinjection | unavailable, `compaction.unsupported` logged once | own file in the always-trusted `~/.grok/hooks`; `PostCompact` fires the bridge (grok's start source is never `compact`), `SessionStart(compact)` catches the registration grok imports from Claude and is deduped; hooks are registered under PascalCase names but grok's envelope spells the `hookEventName` **value** in snake_case (`post_compact`), so the bridge matches either spelling; the card is queued in the mesh inbox because grok discards passive-hook stdout | none, logged once |
| Transcript parser | JSONL | rollout JSONL | none | none | none |
| Account selection | config-dir identities + selector | `auth.json` identities + selector | one implicit OAuth account under the shared Google tooling root | `auth.json` identities + `GROK_HOME` | implicit single account |
| Usage | OAuth usage windows | native 5-hour + weekly windows | native `/usage` command through an isolated provider process | unavailable; no quota endpoint, per-turn cost is in-band | unavailable |
| Stop / teardown | `/exit` | interrupt | `/exit`, wait for presence lock, then kill floor | `/quit`, wait for the registry row to clear, then kill floor | tmux kill + mesh daemon stop |

Codex TUI identity resolves against the matched runtime's
`recovery.harness_account_root`, with pane-process ancestry and start ticks
checking the attachment. Standalone resolution reads the process's `CODEX_HOME`
(or its own `HOME/.codex`) before the daemon default. All registered team roots
contribute `appServer.threadId` exclusions, including hosted seats in other teams;
those IDs cannot be reused by a TUI's cached binding or rollout selection.
An open per-thread writer-lock descriptor can identify a fresh 0.153.4 TUI before
its first rollout exists. This is identity evidence; readiness is checked separately.
Rollout descriptor ownership is preferred; multiple indistinguishable candidates
retain no session identity and log the named `codex_identity_ambiguous` source.
A newest-file guess never resolves a multi-candidate TUI.

For team-owned delivery, an attributed TUI at its visible `›` prompt becomes `idle` / `launch_ready` (heuristic, medium confidence) after the existing 10-second quiet window with reads below the calibrated process-I/O activity rate and no validated completion. A missing rollout or metadata-only rollout counts as no turn; unreadable/ambiguous evidence does not. The recorded pane PID/start, ancestry, account and explicit tmux socket bind the probe. Each scan refreshes `last_observed_at`; notify completion takes over as the authoritative source only when a bound transcript validates its boundary, including after pane reattachment. Legacy Codex notify emits completion only; working detection stays with calibrated process-I/O hysteresis and transcript activity. A writer lock alone cannot validate an old completion. Claude registry and hosted activity paths retain their behavior. This checkout has no harness-prompt launch helper to reuse (launch waits for tmux session availability); the bounded prompt probe recognizes the visible Codex glyph. Pane readiness currently requires Linux process start ticks (including the WSL daemon). macOS has no start-tick provider, so it retains transcript/notify classification without this pre-turn readiness signal.

The registry (`src-tauri/src/session_scanner/cli_tool.rs`) is the one place tool identity may fan out; slices with two real implementations are traits (`SessionSource`, `ActivitySource`, `CompactionSignalSource`, `TranscriptParser`), everything else is data. A conformance suite runs every registry entry through every slice, so a new tool is proven by the same tests as the existing ones. The tracked metric is the number of `CliTool::…` branches outside the registry and slice files; it is meant to go down.

## Model and reasoning effort are first-class

`model` and `reasoning_effort` are separate fields everywhere: role templates and presets (`model:` + `reasoning_effort:`; legacy `"gpt-5.4 high"` spellings still load), persisted per team member, hydrated on resume (member → role default → catalog default), validated before managed activation, and rendered per CLI by `LaunchSpec` (`src-tauri/src/session_scanner/launch.rs`). `LaunchSpec` resolves the registry's primary model flag and its `model_flag_aliases` once for every harness (Codex also answers to `--model`, Grok to `-m`); a model pinned in the user's base command is the effective model, otherwise the requested model is. The effort flag is read the same way — the primary spelling plus `effort_flag_aliases`, which is how Grok's `--reasoning-effort` is recognised in a base — and a base carrying a `deprecated_flags` entry (Codex's `--full-auto`) gets a `launch.flag.deprecated` note without any change to the render. Model override and deprecation notes are emitted by that shared path, and effort is validated against the effective model before the tool-specific flag spelling is rendered. An unknown or unsupported effort is logged (`launch.effort.invalid`) and dropped — never silently. A backend `ModelCatalog` on the terminal contract feeds one effort-aware `ModelSelect` in the UI; the catalog is a suggestion list, so user-added models keep their declared effort. The Claude arm lists `fable` (Fable 5) and `opus` (Opus 5) as the models roles run on; `sonnet`, `haiku` and the 4.x ids stay in the list so persisted roles still resolve, marked deprecated with `opus` as the replacement, and the retired Codex ids point at `gpt-5.6-sol` the same way. A deprecated model still launches — the hint is shown in `ModelSelect` and logged as `launch.model.deprecated`, never substituted. Every launch logs the rendered command (`launch.command.rendered`).

Managed team creation, roster addition, and resume validate the seat before runtime
side effects: cwd must be an existing directory, the effective model must be
non-empty and valid for the tool, and a resolved role's `defaults.cli_tool` must
match the member. Failures are typed `CoordinationError::Validation` errors naming
the member and field with a correction. Missing model declarations use the
existing member → role → catalog default resolution; custom model IDs remain
supported. Explicit `external` placeholders and models known to belong to another
tool are rejected. The roster-only API (which has no model field) persists the
tool's catalog default. Unreadable role metadata keeps the existing fail-soft
behavior. This managed-seat check addresses Wave-1 F2/F13 while preserving
resolved defaults and custom models.

Model catalog entries also carry optional `capabilityTier` and `tierRank` fields, while role templates can carry an optional `capability_policy` describing fixed or adaptive selection, a minimum tier, allowed model ids and an effort band. These Stage-0 fields are persisted and transported but deliberately unenforced: they do not change selection, launch rendering, effort application or UI choices. The staged enforcement and routing roadmap lives in [`role-first-model-routing.md`](../design/role-first-model-routing.md).

The bundled presets run the **v4 developer roles**: Dev Team, Full Team and Research Team staff `v4-developer-codex`, Grok Pair staffs `v4-developer-grok`, and neither pins model or effort in the preset — both are inherited from the role, so a preset follows the role default (medium) wherever it moves. The v3 developer roles stay in the catalog for one more release so the comparison can be re-run against them; on the current eval cases the two are indistinguishable at medium, so v4 is the default for its wording contract, not for a score ([Phase C results](../design/research/phase-c-v4-results.md)).

## Task-level effort

The launch effort is a property of the *member*: it is what the session was started with and it holds for the session's lifetime. The effort a piece of work deserves is a property of the *assignment*, and it changes from task to task. Those are two different numbers, and both are visible: the node and its detail show the launch effort, and beside it the level the current assignment carries, with the lead's reason on hover.

Effort travels with the assignment, in mesh, because mesh is the only component that owns both the assignment record and the pane submission for every CLI. `mesh task assign` requires `--effort` and `--why`; both are persisted on the task record and on the inbox message the assignee receives, and mesh applies the level before it delivers the notice wherever the harness takes it in its own prompt. taurhaus reads the pair back — off the **task record**, never off the inbox, because an inbox keeps every assignment ever delivered and its newest effort-bearing message outlives the task it was asked for — for the operational footer, the post-compaction card, the task card and the mesh canvas, and owns the one thing mesh cannot do.

**How a running session changes effort** is a registry declaration (`CliCapabilities::runtime_effort`), because the two paths have different owners:

| Path | Harnesses | Who applies it |
|---|---|---|
| `SlashCommand` | Claude Code, Antigravity, Grok | **mesh alone**, which types the harness's own command into the pane before it delivers the assignment notice, and only when the level differs from the one already in force. taurhaus never sends it: a second writer into the same pane would double the level change, and taurhaus can only act after the assignment is already readable |
| `ResumeWithFlag` | Codex | taurhaus, which stops the session and resumes the member's *own* conversation with the effort flag (`codex resume <session> -c model_reasoning_effort="<level>"`), on the operator's own launch settings and account — Codex 0.150.1 changes effort only through its interactive `/model` picker, which has no one-line grammar to type |
| `None` | — | nothing; the launch effort stands |

The paths are ordered differently, but bundled mesh 0.2.28 gates both on the same runtime fact. Where mesh submits the command it writes `appliedEffort` before delivering the notice. For Codex, mesh holds a mismatched notice while taurhaus resumes the member and commits the effort the rendered command actually carries; the task scan is the earliest trigger and the 30 s daemon sweep starts a switch when that app-side edge was missed. A member already at the requested level passes immediately. A failed switch remains bounded by taurhaus's three-attempt budget and mesh's own bounded fail-open wait rather than holding delivery forever.

Both owners read the same fact — `appliedEffort` on the member's runtime record, seeded by the launch — so neither acts on an assignment the other has already handled, and neither restates a level the member is already at. Because both processes write that file, the runtime store takes the record's own advisory lock — the one mesh takes — across its read and its atomic replacement, under the team lock, so neither writer's snapshot lands on top of the other's. A pass that loaded its snapshot earlier re-reads `appliedEffort` inside that same critical section and carries it forward. The one liveness exception is adopting a changed session id that no taurhaus launch committed: its applied level is unknown, so liveness clears the field in the same compared commit and the next sweep re-applies the assignment. Launch commits derive the field from rendered reality, including an operator base's effort pin; an ignored or invalid request is never asserted as applied, and only a launch that did reach a level clears the attempt budget that bounds the switch. Where the platform refuses an advisory lock outright (Windows answers `ERROR_INVALID_FUNCTION` for the redirected paths a WSL-resolved teams directory lives behind) the store still writes and reports `coordination.store.lock_unsupported` once per path: the re-read keeps the exposed window down to the rename itself, but the exclusion is genuinely absent and an operator has to be able to see that.

A relaunch takes a session down, so taurhaus's side is bounded on every end.

- **The level and task identity travel together.** Pending and in-progress mesh task records form the open candidate set; blocked-status records cannot anchor the level through that ranking. Within the set, mesh's attention projection names a notice still held for the member (`attentionState` of `assigned_pending_delivery` or `delivery_failed`); otherwise an open task the member is already at the applied level for keeps the target; otherwise the highest requested level wins. The operational snapshot is only the compatibility fallback when its task has no readable mesh-owned record; a matching blocked or otherwise non-resumable mesh record suppresses the fallback and cannot start a switch. This keeps two open assignments deterministic without reading stale inbox text or pairing one task with another task's number; a member whose work is finished still owes no level.
- **Only a live member with a recorded session id is switched.** A member the operator stopped stays stopped; a missing session id aborts with `effort.resume.failed` rather than resuming, because the resume pipeline would render a *fresh* launch and an effort switch may never throw away the conversation the assignment builds on.
- **A stop that did not land aborts the switch.** The teardown is best-effort and reports a refused ownership check or a failed kill in its diagnostics alone, so the failed `kill_pane` step is read: resuming on top of a live session would render a second one beside it.
- **The rendered command has to carry the level.** The pass renders the relaunch's own command and reads the level back *before* it stops anything, against the same authority the launch itself uses. Where the operator's resume command pins `model_reasoning_effort` the renderer would keep that value and drop the requested one, so the assignment's level is written into the base first; a pin no rewrite can reach — one naming no value token — is refused. So is a level the member's model does not accept, which the renderer drops outright (`launch.effort.invalid`). Either way the member keeps running, the refusal is one bounded `effort.resume.failed` attempt, and the level is never recorded as applied: a relaunch that committed an unknown level would also clear the attempt budget, and the next sweep would stop the member again for the same level, forever.
- **A member being started takes the level at launch** instead of being stopped again a moment later: an operator's own resume renders with the active task's effort.
- **A relaunch that fails leaves `appliedEffort` at the level the session was actually running** — recording the requested one would report a success that never happened — and is retried at most three times per task and level, cleared by any launch that commits. The next pass records `reason: budget_exhausted` on that failure and emits one `effort.resume.failed` event with the task id and three spent attempts; later self-heal passes stay silent.

**Claude Code's `/effort` has a side effect**: it also saves the level as the user's default for that model, in `modelSettings.<model>.effortLevel` under the account's `settings.json`, so a team run leaves the operator's own default rewritten after the team stops. Putting that value back automatically is **deferred to a follow-up**: a correct restore needs a single writer and proof of ownership — which account's file, which level the harness actually wrote, and no second process editing it — and the version that shipped in the first cut of this work had neither. Until then, `/effort` moves the operator's saved default and taurhaus does not touch it. What the registry does keep is `runtime_effort_frozen_env`: `CLAUDE_CODE_EFFORT_LEVEL` outranks the saved default and is frozen per process, so a managed launch must not set it — the team launch renderer drops a leading `NAME=value` prefix that assigns it and refuses any spelling it cannot rewrite safely, rather than letting a frozen level reach the pane and discard every assignment's level for the session's life.

**Choosing the level is the lead's job**, not an algorithm's. The proportionality rule, in the order a lead should weigh it:

- **Stakes** — what breaks, and for whom, if the work is wrong.
- **Reversibility** — a migration, a release or a user-visible write earns more than something a revert undoes.
- **Uncertainty** — an unfamiliar subsystem or a diagnosis with no reproduction earns more than a mechanical change.
- **Scope** — the number of files, layers and contracts the change crosses.
- **Budget** — a higher level costs tokens and wall-clock for every turn of the task, not just the hard one.

Phase B evidence: **medium is the default for developer roles**, and **high is a deliberate exception with a stated reason**. `--why` exists to make that reason survive into the assignment, the footer and the post-compaction card — the lead states it once and every surface that shows the level shows why.

## Accounts and usage

Account selection is a capability slice, not a Claude-only path. A provider discovers tool-owned config directories and identities; the generic core remembers `pinned` and `last_used` choices per project and tool, resolves explicit → session → pin → last-used → global default → base-command selector → default-dir precedence, and renders the registry's selector in `LaunchSpec`. Resumes derive their account from the provider's transcript layout. Tools without a provider stay on the logged single-account floor. The chooser is not only for a project that has decided nothing: a launch whose account *is* settled is judged against the usage reading taurhaus already holds — the one the chip and the account menus show — and stops to ask when that subscription is spent or unreadable, with a fresher reading requested in the background rather than waited for. A limit whose reset has passed, and a detection that came back degraded, both let the launch through. The user can also call the dialog up on any launch surface to compare what is left of each one.

The base command is read the way the pane's own interactive shell reads it, not literally: `session_scanner/launch_base.rs` expands a head word the shell reports as an alias (at most three levels, cycle-guarded, quoted heads left alone) through the additive `resolve_launch_base` daemon method — in-process on Linux and macOS, in the WSL daemon on Windows — and a head that is still not the tool's own executable is reported as `launch.base.opaque`. The commands layer performs that same cached resolution for managed team launches and carries the `ResolvedBase` inward beside detection-backed account directories; coordination never probes the shell, and background passes request a resolution only after selecting a member to relaunch. If the probe cannot answer, the member still launches from the configured literal and emits one `launch.base.unresolved` event for that render. App launches rewrite a base-owned selector only when a higher-precedence account choice exists. Managed Codex and Grok members persist a stable `account_id`; each launch resolves it to a machine-local directory at render time and pins `CODEX_HOME` or `GROK_HOME`. An unavailable requested account falls back to the current default home with `launch.account.fallback`, `accountApplied: false`, and both old/new labels preserved for the Mesh surface. Two details follow from the shell doing the reading: a `~` in the selector is expanded where that shell lives — the WSL home when the app itself is on Windows, which the app cannot name from its own side — and, because a shell obeys the last assignment of a name, every selector assignment an expansion left behind collapses into the single one the chosen account owns. An opaque team wrapper remains launchable but persists `accountApplied: false` with the stable `opaque_base_command` note, which the member node surfaces instead of implying that account selection was guaranteed.

Claude is different because its managed inbox and team state share one `CLAUDE_CONFIG_DIR`: account selection is team-root-scoped, never per member. The daemon-owned registry maps each team to one teams root, so mixed-account Claude teams remain impossible while whole-team Claude switches can move that authority safely. The shipped boundary is recorded in [`coordination-daemon-routing.md`](../design/coordination-daemon-routing.md#per-team-root-authority-audit-protocol-24).

Team account switching is a daemon-owned accept-then-poll operation. For Codex and Grok it validates a detected signed-in target, records an accumulating per-member handoff manifest (previous label, session id, transcript path, last activity), tears the team down, rewrites matching member ids, and resumes through the canonical pipeline with switch onboarding. Transcripts are pointers, not migration payloads. The Accounts hub offers the operation deliberately from team relationships, while an exhausted account offers it as a proposed recovery action rather than forcing a switch.

Usage is a second provider slice attached to each detected account as an in-memory snapshot. Providers read native state at request time; taurhaus never logs, persists, refreshes, or otherwise owns a credential. Claude uses `CLAUDE_CONFIG_DIR` and its OAuth usage endpoint. Codex uses `CODEX_HOME`, display-only decoding of the `id_token`, and its native usage windows; API-key accounts remain selectable but explicitly report usage as unavailable. Antigravity exposes one implicit account and obtains its native windows by running `agy -p /usage --output-format json` through the injectable command boundary. Grok uses `GROK_HOME` and reads only the display names in its `auth.json`; it reports usage as unavailable because grok 1.0.5 publishes no quota endpoint, and the registry carries the sentence the UI shows in a meter's place. The retired Claude status-line bridge is uninstalled once without disturbing foreign status-line commands.

## App and daemon move together

The daemon (WSL2 on Windows, native elsewhere) owns process inventory, session identity, activity, tmux focus and the JSONL log sink. Native harness hook processes call the same taurhaus compaction bridge on every platform; hosted Codex also uses completed compaction notifications on the daemon-owned connection. The app and daemon speak a versioned JSON-line protocol; **the app validates the exact protocol version on every connect path** (startup, health, inline reconnects, the focus bridge's own socket) and refuses a mismatched daemon rather than half-working. Consequences:

- New methods and fields are additive (`#[serde(default)]`) and do not bump the version; a changed contract does.
- A version bump means the app release and `just install-daemon` ship together. Reinstalling only the daemon under an older app leaves the app daemon-less.
- The app auto-updates its bundled daemon when the installed one is older (semver); mesh is bundled, lock-pinned and installed the same way (`CONTRIBUTING.md` release checklist).

## Stability rules learned the hard way

- A scan whose inventory cannot be read is **degraded** and inert: nothing is pruned, no state changes, no exports, the last good snapshot stands; degradation is visible to the UI within a poll.
- Processes without a controlling terminal are not sessions.
- Activity events fire on real transitions only; first sight of an idle process is not a transition.
- One writer per shared file, with mesh's lock discipline (flock + inode re-check, tmp+rename under the lock); unknown fields written by other tools survive every save.
- A tmux pane is identified by pid + start time, not by pane id; a foreign pane is quarantined, never typed into.
- Compaction has exactly one detection path per harness. Claude and supported managed Codex sessions receive context on native hook stdout; grok's hook queues the card in its mesh inbox.
- A harness that imports another vendor's hook registrations (grok reads `~/.claude/settings.json` by default) can invoke one bridge twice for one event; the registry declares that and the bridge deduplicates, so one compaction is one reinjection.
- The hook that observes a compaction is not always the channel that can deliver the card: the registry names the delivery per harness (`additionalContext` on the hook's stdout for Claude Code and Codex, the member's mesh inbox for grok, whose passive-hook stdout is documented as ignored), and the delivery is recorded only once it has actually happened.

## Retired

- **Codex transcript compaction pipeline** (0.8.2): Codex 0.147 established a reliable native `SessionStart(source=compact)` hook, so the transcript extractor, signal log/watcher/processor, daemon/app owner selection and `harness.codex_compaction` setting are gone. Supported managed Codex installs and reconciles the hook by default; older versions get one unsupported event and no reinjection.
- **Gemini CLI** (0.8.0): Gemini Code Assist for individuals refuses the client ("migrate to the Antigravity suite"), so the registry entry, launch arm, TCP idle heuristic, task scanner, catalog entries and role templates are gone. Persisted `gemini` tool values load as an unknown tool instead of aborting the record that carries them — a role catalog or team config from 0.7.x still opens; re-pick Antigravity where a member needs it. There is deliberately no alias: `agy` is a different binary with different flags and directories.
- **Claude status-line bridge** (0.7.0): the 0.6.8 wrapper around `settings.json`'s `statusLine` could never carry the per-model buckets and edited user config; the OAuth usage endpoint replaced it, and the bridge is uninstalled once, restoring the original status line byte-for-byte.
- **Gemini CLI account/usage provider** (never shipped): the fixture-driven `retrieveUserQuota` provider planned as 17d was cancelled with the CLI.
- The eight architecture infographics under `docs/images/` were regenerated on 2026-08-28 from their prompts in `infographics.manifest.yaml` (`just infographics`, see `docs/operations/infographics.md`).

## Native boundary delivery bridge

`coordination/compact_hook/drain.rs` implements the separate, disabled-by-default
`mesh-hook-drain/1` delivery seam. Mesh publishes per-build/host/event/source
capabilities; compaction support never enables ordinary delivery. The native hook
requires the managed runtime's exact session, launch-root/incarnation, attachment,
pane/socket/PID/start, tmux session and context facts, plus a canonical delivery-owner
team and hook adapter selection. Claude/Codex return a bounded batch through
`hookSpecificOutput.additionalContext`; only the compaction owner prepares the
recovery card, and an allowed compact drain appends a labelled section after it.
No-card/duplicate-card decisions do not suppress independently eligible mail.

Managed ordinary-event scripts and registrations are reconciled separately from
compaction, preserve foreign hooks/trust, use live account homes, and install switch
targets before removing unused previous homes. Unknown/downgraded Codex compaction
reconciliation is unchanged. A flushed response is only `hook_response_offered`;
uncertain output remains `outcome_unknown`, with no automatic retry or alternate
transport. agy/Grok retain their existing activity/compaction behavior and remain
descriptor-only for drains. Stop continuation is unsupported. Production pins stay
disabled pending separately commissioned, scoped harness uptake; see
[the bridge telemetry contract](../operations/telemetry-contracts.md#native-hook-bridge--software-conformance-only).

Drain reconciliation stands down when native Mesh or capability evidence is unavailable,
including the Windows app, preserving WSL-daemon-owned registrations. Runtime
`contextGeneration` string publication requires daemon protocol 26 so older apps
cannot accept an incompatible persisted runtime record.

## How changes are made

Each change is a small PR with red-first regression tests naming the breaking commit, implemented by one model family and reviewed by the other (Opus ↔ Codex) through two lenses — conformance to the spec, and an operational checklist (upgrade of persisted data, protocol bumps on wire vocabulary, Windows/WSL paths, user-config edit discipline, concurrency, honest tests, hygiene) — with the fix → re-review loop repeated until no majors remain. Implementers commit after every green step and never edit the ledger; the orchestrator writes the spec (reviewed by the other family first when it edits user config or persisted formats), fills the ledger at merge, and merges only on the check's conclusion. Each new CLI starts with two independent research reports (`docs/design/research/`), verified live on a host that has it; the plans' facts tables cite them.

## Owned Codex hosting (stage 5b, descriptor-gated)

At creation, initialize and add-agent accept optional seat `delivery`: omitted
or `"tmux"` keeps the existing pane path; `"app_server"` writes
`adapter_mode: "app_server"` on the member and launches from a fresh record.
Only Codex on the Linux/WSL daemon supports hosting. The builder and add-agent
form show **Delivery** only when the backend tool descriptor reports
`hostingSupported: true`; fallback descriptors never enable it. Protocol 27
(unreleased) makes this choice binding alongside canonical team messaging. The existing launch/resume machinery resolves aliases, model, effort and explicit `CODEX_HOME`. The hosted render preserves account/permission/sandbox policy and refuses opaque wrappers, unsupported arguments and unnamed resume. Non-Linux opt-ins fail explicitly. The hosted member render suppresses the runtime-derived managed TUI hook-trust flag; explicit unsupported arguments in an operator base still refuse by name. Strict config continues to enforce model/effort/sandbox/approval.

The builder defaults unchosen Codex seats to **app-server (native; TUI attached in tmux)** when canonical messaging is on, the tool advertises hosting, and the backend's `hostedDeliverySupported` IPC fact is true (normalized as `hosted_delivery_supported`). The initialize payload carries the resolved delivery explicitly. Runtime add-agent defaults unchosen seats to tmux because its snapshot does not attest the target team’s messaging mode; explicit choices still travel in the payload, and the backend rejects hosted adds unless the target config has `messaging_format: 2`. **tmux pane (fallback)** remains available; disabling canonical messaging or losing admissibility returns unchosen seats to tmux. Explicit operator choices are preserved, and the Claude lead is unchanged.

The fact uses the installed Mesh contract, or the bundled contract when no readable install exists, and requires Mesh >= 0.3.0 plus an enabled matching Codex build from that binary's `mesh delivery capabilities` JSON `native_descriptors`. Host, configuration, trust and transport must match the daemon-owned attachment identities; unknown versions, failed queries and disabled descriptors fail closed. `mesh version --json` does not expose these descriptors. The inspected Mesh source still disables 0.153.4 pending pairing; it becomes a default only once the queried descriptor admits it. Mesh's `AppServer::new` remains the final admission authority. This is an additive app-local status field, not a daemon wire change; protocol 27 and initialize semantics are unchanged.

Hosted roster seeding preserves the incarnation minted by team creation. Adding an app-server seat to a canonical team without an incarnation assigns one to that team: co-resident tmux seats use terminal contract 1 and versioned recovery keys on their next activation. A hosted add that fails before publishing its attachment rolls back the roster and Mesh join, allowing a same-name retry. Once an attachment is published, failures retain the recoverable seat and any uncertain input.

The daemon owns the child, private socket and persistent thread. A failed launch names the child's exit status and stderr tail. A child exit before transport readiness gets one retry after 1.5 seconds, with the same strict config and a fresh socket after reaping the first child; live-child readiness timeouts are not retried, and runtime/thread publication occurs only after a successful launch. `appServer` publishes contract, socket/thread/member/account, PID/start ticks, incarnation, build/host/configuration/trust/transport and lifecycle state with a new attachment generation in one compared commit. Startup becomes ready only after a recovery receipt. Stop publishes before killing; restart never adopts a PID or launches automatically. Controlled resume names the saved thread and account. After a daemon crash, a surviving child is published as `orphaned` and the conversation names its PID. The daemon never kills a recorded PID it does not own. For manual cleanup, verify that PID’s `/proc/<pid>/stat` start ticks still match `appServer.processStart`, terminate only that confirmed orphan, then stop and resume the member. A gone process becomes `unavailable`; named recovery remains possible.

After the thread is ready, the daemon opens a real TUI in the member pane using the resolved executable and private view home: `env -u TMUX CODEX_HOME=<private-launch-home> <codex> --remote unix://<socket> resume <exact-thread-id> --no-alt-screen --strict-config`. The host owns model/permission policy; the TUI attaches to that existing thread. `appServer.attachArgv` records the exact CLI argv alongside `threadId`. Pane PID/start ticks, tmux socket/session and terminal exclusion retain their existing meaning. Host RPC, terminal I/O and the compared runtime publication use separate lock scopes.

Hosted activity is authoritative from the host client's tracked thread state, published into the daemon's versioned `SessionActivityHub`: a running turn is **working**, non-empty `activeFlags` mean **active** (waiting for input/approval), and idle is **idle**, all with high confidence and `source: host`. The attached TUI's remote/resume argv supplies identity, never activity evidence: neither its rchar rate nor the ordinary Codex notify edge applies. Its transcript is resolved under the app-server account root by exact thread ID, with missing-file lookups retried at most every 30 seconds. A missing TUI inventory row clears pane/process identity without retiring the hosted thread. Disconnection or an unmatched remote thread is **uncertain**, `source: host_unavailable`; teardown/rollback removes the owned hub entry. Disconnected host rows remove the coordination activity export, whose schema has no uncertain state, rather than publishing a false idle floor. Deliberately stopped attachments retain Offline roster status. A separate stoppable worker issues one `thread/read` RPC per host every 500 ms under a 250 ms operation deadline, without blocking the scanner. Each thread response is transferred and parsed; the worker only avoids the additional copies of cached events and requests used by the transcript view. Complete pending-read responses preserve tracked status. Transport, deadline and malformed-frame failures close the connection and publish unavailability, as do a stopped child or missing socket. The next probe reconnects under a separate five-second budget with a non-blocking lock; failed reconnects back off at 1, 2, 4, then 5 seconds and emit one `hosted.rpc.reconnect_failed` record per failure episode. A transcript/input request can also reconnect. Both verify the same account, build, thread and policy before restoring host activity and never replay input or approvals. Lost connection-scoped approval requests set the existing `outcomeUnknown` fence so the operator is told to stop and re-trigger instead of silently waiting without a control. Tracked transitions use the single `classification.rs` event emitter. No protocol bump is required: hosted source metadata is additive and omitted on ordinary sessions.

Managed hosted rendering suppresses the generated Codex `-c notify=…` override as well as the managed TUI hook-trust flag. The host's tracked status supersedes the notify edge; ordinary tmux launches retain both rendering rules.

Closing the pane closes a view; it does not stop the daemon-owned child. Resume of an unchanged live host verifies its thread and reattaches the pane without thread creation or a new host generation. An already attached live pane is reused without another command. Missing threads never open a picker or substitute a fresh conversation. Explicit member teardown stops the owned child and closes only the matching attached pane. The app transcript/input panel remains the second view, and controlled opt-out still performs the stage-5b named-thread relaunch into a plain pane.

Published `hosted` status gates transcript/input/approval/cancel controls. The host lock revalidates attachment/root authority, excludes compaction through stdout/bookkeeping, and defers busy liveness. Ordinary operations get five seconds, cold launch thirty. Send starts an idle turn or steers an active one; pending recovery waits for the next idle turn, preserving the draft without queuing. Hosted compaction is notification-detected and hook-deduplicated: completed owned-thread
`contextCompaction` admits a generation and pending obligation (`host_notification`).
Reconciliation services the connection without the panel. Idle submits the canonical card alone
via `turn/start`, with `injected` / `host_turn` and a submitted receipt; busy recovery defers.
Dedup uses thread and matching turn/item IDs, or 30 seconds between ID-less opposite observers,
preserving known conflicts. Hosted hooks restore the control contract without a task snapshot,
logging `received` plus `compaction.codex_host.*`. The transcript shows boundary and card.

On Codex 0.153.4, plain `thread/read` verifies identity, status, direct-input
readiness and settings; `includeTurns` is unsupported and returned turns are always
empty. The daemon tracks turn IDs and status per connection from notifications,
steers/interrupts the tracked active turn, and starts recovery on idle threads.
First-turn read errors (-32603) retry within the host deadline, then return `pending:`;
only a turn-result receipt clears input uncertainty. Hosted UI `thread.turns`
is synthesized from the existing bounded event deque (including completed items),
so history before this connection or outside that window is unavailable. Rejections
emit `hosted.rpc.rejected` with method, numeric code and at most 256 message
characters, without request params; public errors stay generic. The
[binding probe facts](../design/app-server-transport-amendment.md) define this build's
contract; the tmux path and protocol 27 remain unchanged.

Definite steer rejection permits a newly validated attempt; ambiguous input blocks replay. After stop, an explicit **abandon without replay** decision records the attachment generation and permits named relaunch without changing recovery receipts. Controlled opt-out rollback validates the dead child/root/account/thread, refuses unknown input or retained native attempts, and closes the owned attached pane under `detach_tui` exclusion before clearing `appServer` and the retained TUI pane identity at a new fence, forcing plain-session recovery into a fresh pane. A reused foreign pane is left untouched while its stale recorded identity is cleared. `hostRollback` retains old/new modes, opt-in, attachment tuple and unresolved attempts. Failed pane recovery retains that boundary; wrong-thread panes are cleaned up. Team-owned Mesh switching still needs the unbuilt paired packet; hot conversion
refuses. On `delivery_owner: team`, direct rollback returns
`app_server_rollback_on_team_owned_team: stop the seat, remove it, re-add it with delivery tmux`.
Operational rollback is **stop → remove member → add agent with the same name and
delivery tmux**. The replacement launches a plain pane with `terminalContract: 1`
and no member daemon. It is a new seat, not a promise to resume the hosted thread.

The paired record identities are `host: "taurhaus-daemon-owned-thread/1"`,
`configuration: "strict-config/1"`, and `trust: "daemon-owned/1"`.
`configurationDigest` retains the per-launch argument digest separately.
These classes permit an exact compiled Mesh pin after the integration trial;
they do not enable native delivery eligibility. Project instructions are allowed:
`appServer.instructionSources` records the host's sources and one
`hosted.instruction_sources.loaded` info event logs the thread ID and source count.
The strict per-member view config still carries and reasserts the thread's model,
effort, sandbox and approval policy. Only the TUI config disables its own project-document discovery; thread policy repair omits that key and verifies instruction-source parity before allowing further input.

The Codex `0.153.4` transport pin is `unix-websocket`: HTTP/1.1 Upgrade with a random 16-byte key and verified RFC 6455 accept, then masked client text frames and unmasked server text frames. Each message is one JSON object without `jsonrpc`; `initialize` with `clientInfo` and `capabilities.experimentalApi: true` comes first, with per-connection request IDs. Fragmentation and ping/pong are supported; close, malformed frames and messages over 64 KiB fail closed. Every read/write uses the remaining host-operation deadline. Raw NDJSON produces EOF on this build (stage-5b regression `cadd533e`).

The transport/attached-TUI evidence is scoped to build `0.153.4`. A CLI bump must rerun the HTTP Upgrade/initialize handshake probe before its descriptor remains enabled; an unknown build or failed handshake refuses before thread creation. The descriptor's `attachedTui: verified on 0.153.4` records the trial's exact-ID
remote-resume primitive, not a production-config verification. The generated
private home, `project_doc_max_bytes` and amendment-required `--strict-config`
flag are integration changes covered by fakes, awaiting the real integration trial.
Fake transports prove software only. Trust and paired Mesh eligibility remain separate and disabled pending their packet; this lane performs no paid trial or activation.

The attached client uses a daemon-generated `config.toml` in a private per-host,
per-member launch home. Model, effort, approval and sandbox settings come from
the effective thread/start or thread/resume response; only `auth.json` links to
the selected account. No account config, instructions, hooks or skills are copied.
The TUI config sets `project_doc_max_bytes = 0` to disable project instruction
file discovery. It adds no personality, developer-instruction or project-trust
entries. Host startup sends no extra instruction/config overrides. Loaded host
instruction sources are allowed and recorded; missing sources default to an empty
list. The lane does not claim to suppress host project instructions via RPC.
Workspace-write options are copied only when present and non-null. Approval enums
are validated and normalized to config/request spelling, while settings comparisons
retain the server's original wire spelling.
`--strict-config` rejects unknown keys; it is not itself an isolation mechanism.
The home is removed when the owned host is dropped. Runtime account identity
continues to mean the selected credential account, not this disposable view home.

A differing `thread/settings/updated` notification for the owned thread logs
`hosted.settings.diverged` without settings contents. After correlating the pending
response, the daemon reasserts its policy using a named `thread/resume` and checks
the effective response under the same host deadline. Failure keeps subsequent
operations gated on repair; repair never submits or replays input. This follows the
[documented resume configuration overrides](https://learn.chatgpt.com/docs/app-server).
Fake tests establish the software path; real attached-client drift/reassertion
still belongs to the paired integration trial.

Host relaunch retains the old pane identity until terminal-locked attachment:
a verified shell is reused, a verified old attached TUI is retired before a new
view opens, and foreign pane identities are left alone. A live host's repeated
attach remains idempotent and reports pane reuse truthfully.
