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
| Launch rendering | `--model`, `--effort`, `-n <agent>`, team flags | `-m`, reasoning config, notify + managed-hook trust flags | `--model`, `--effort`, `--dangerously-skip-permissions`; conversation resume | `--model`, `--effort` (validated per model), `--always-approve`; `--continue` / `--resume <uuid>` | user's base command verbatim |
| Session identity | sessions registry | fd-verified rollout binding | `last_conversations.json` + presence lock | `active_sessions.json` row bound by pid and cwd | `/proc` + tmux pane, no session id |
| Busy / idle | registry status (authoritative) | turn-complete notify (authoritative) | hooks sink, default on, needs workspace trust and agy 1.1.10; process-IO floor otherwise | `events.jsonl` turn lifecycle (authoritative) | rchar-rate hysteresis + pane liveness |
| Message delivery + wake | native inbox poller | inbox + tmux wake | inbox + tmux wake | inbox + tmux wake (plain Enter queues, Ctrl+Enter interjects) | inbox + tmux wake |
| Compaction signal | `SessionStart(source=compact)` hook | managed `SessionStart(source=compact)` hook by default on Codex >= 0.147; older versions log `compaction.codex_hook.unsupported` once and receive no reinjection | unavailable, `compaction.unsupported` logged once | own file in the always-trusted `~/.grok/hooks`; `PostCompact` fires the bridge (grok's start source is never `compact`), `SessionStart(compact)` catches the registration grok imports from Claude and is deduped; hooks are registered under PascalCase names but grok's envelope spells the `hookEventName` **value** in snake_case (`post_compact`), so the bridge matches either spelling; the card is queued in the mesh inbox because grok discards passive-hook stdout | none, logged once |
| Transcript parser | JSONL | rollout JSONL | none | none | none |
| Account selection | config-dir identities + selector | `auth.json` identities + selector | one implicit OAuth account under the shared Google tooling root | `auth.json` identities + `GROK_HOME` | implicit single account |
| Usage | OAuth usage windows | native 5-hour + weekly windows | native `/usage` command through an isolated provider process | unavailable; no quota endpoint, per-turn cost is in-band | unavailable |
| Stop / teardown | `/exit` | interrupt | `/exit`, wait for presence lock, then kill floor | `/quit`, wait for the registry row to clear, then kill floor | tmux kill + mesh daemon stop |

The registry (`src-tauri/src/session_scanner/cli_tool.rs`) is the one place tool identity may fan out; slices with two real implementations are traits (`SessionSource`, `ActivitySource`, `CompactionSignalSource`, `TranscriptParser`), everything else is data. A conformance suite runs every registry entry through every slice, so a new tool is proven by the same tests as the existing ones. The tracked metric is the number of `CliTool::…` branches outside the registry and slice files; it is meant to go down.

## Model and reasoning effort are first-class

`model` and `reasoning_effort` are separate fields everywhere: role templates and presets (`model:` + `reasoning_effort:`; legacy `"gpt-5.4 high"` spellings still load), persisted per team member, hydrated on resume (member → role default → catalog default), and rendered per CLI by `LaunchSpec` (`src-tauri/src/session_scanner/launch.rs`). Effort is validated per tool; an unknown or unsupported value is logged (`launch.effort.invalid`) and dropped — never silently. A backend `ModelCatalog` on the terminal contract feeds one effort-aware `ModelSelect` in the UI; the catalog is a suggestion list, not an allowlist, so user-added models keep their declared effort. The Claude arm lists `fable` (Fable 5) and `opus` (Opus 5) as the models roles run on; `sonnet`, `haiku` and the 4.x ids stay in the list so persisted roles still resolve, marked deprecated with `opus` as the replacement, and the retired Codex ids point at `gpt-5.6-sol` the same way. A deprecated model still launches — the hint is shown in `ModelSelect` and logged as `launch.model.deprecated`, never substituted. Every launch logs the rendered command (`launch.command.rendered`).

## Task-level effort

The launch effort is a property of the *member*: it is what the session was started with and it holds for the session's lifetime. The effort a piece of work deserves is a property of the *assignment*, and it changes from task to task. Those are two different numbers, and both are visible: the node and its detail show the launch effort, and beside it the level the current assignment carries, with the lead's reason on hover.

Effort travels with the assignment, in mesh, because mesh is the only component that owns both the assignment record and the pane submission for every CLI. `mesh task assign` requires `--effort` and `--why`; both are persisted on the task record and on the inbox message the assignee receives, and mesh applies the level before it delivers the notice wherever the harness takes it in its own prompt. taurhaus reads the pair back for the operational footer, the post-compaction card, the task card and the mesh canvas, and owns the two things mesh cannot do.

**How a running session changes effort** is a registry declaration (`CliCapabilities::runtime_effort`), because the two paths have different owners:

| Path | Harnesses | Who applies it |
|---|---|---|
| `SlashCommand` | Claude Code, Antigravity, Grok | mesh types `/effort <level>` into the pane before the notice, and only when the level differs from the one already in force |
| `ResumeWithFlag` | Codex | taurhaus, which stops the session and resumes the member's *own* conversation with the effort flag (`codex resume <session> -c model_reasoning_effort="<level>"`), on the operator's own launch settings and account — Codex 0.150.1 changes effort only through its interactive `/model` picker, which has no one-line grammar to type |
| `None` | — | nothing; the launch effort stands |

The two paths are not ordered the same way. mesh types `/effort` before it delivers the notice, so a `SlashCommand` member reads the assignment already at the level. taurhaus's Codex relaunch rides the background coordination pass instead — mesh owns notice delivery, and nothing on taurhaus's side gates it — so a Codex member can pick the assignment up at its previous level and be resumed into the same conversation moments later. Closing that gap means ordering it in mesh, which owns both ends.

Both owners read the same fact — `appliedEffort` on the member's runtime record, seeded by the launch — so neither acts on an assignment the other has already handled, and neither restates a level the member is already at.

A relaunch takes a session down, so taurhaus's side is bounded on every end. It switches only a member that is live, has a recorded session id to resume, and runs on a launch command that does not pin `model_reasoning_effort` itself — a member the operator stopped stays stopped, an effort switch never turns into a fresh conversation, and a level the rendered command could not carry is reported rather than recorded as applied. It answers only an assignment mesh delivered at or after the session attached: an inbox keeps every assignment ever sent, and without that rule an upgrade or a manual restart would take a working pane down again for work that is long finished. And a relaunch that fails leaves `appliedEffort` at the level the session was actually running — recording the requested one would report a success that never happened — retrying at most three times per level, cleared by any launch that commits.

**Claude Code's `/effort` has a side effect**: it also saves the level as the user's default for that model, in `modelSettings.<model>.effortLevel` under the account's `settings.json`. A team run would leave the operator's own default rewritten long after the team stopped, so taurhaus records the user's value before a managed member's first launch — from the account that launch actually selects, not from whichever directory the process would default to — and puts it back when the member or the team stops: atomically, through a symlink to its target rather than over it, keeping the file's own permissions, touching only that field, and only while the value on disk is still the level taurhaus knows the harness was asked for — the newest assignment mesh typed, with the launch effort behind it. With neither, the value on disk is the operator's own and is left exactly as it is. A restore that could not read or write the file has not run, so the record is kept for the next stop instead of being forgotten. A member that was already running when this build landed has its default recorded once by the background pass, before any assignment can reach it. Which harnesses have the side effect, and where they save it, is another registry declaration (`runtime_effort_default_sink`). `CLAUDE_CODE_EFFORT_LEVEL` outranks the saved default and is frozen per process, so managed Claude launches must not set it.

**Choosing the level is the lead's job**, not an algorithm's. The proportionality rule, in the order a lead should weigh it:

- **Stakes** — what breaks, and for whom, if the work is wrong.
- **Reversibility** — a migration, a release or a user-visible write earns more than something a revert undoes.
- **Uncertainty** — an unfamiliar subsystem or a diagnosis with no reproduction earns more than a mechanical change.
- **Scope** — the number of files, layers and contracts the change crosses.
- **Budget** — a higher level costs tokens and wall-clock for every turn of the task, not just the hard one.

Phase B evidence: **medium is the default for developer roles**, and **high is a deliberate exception with a stated reason**. `--why` exists to make that reason survive into the assignment, the footer and the post-compaction card — the lead states it once and every surface that shows the level shows why.

## Accounts and usage

Account selection is a capability slice, not a Claude-only path. A provider discovers tool-owned config directories and identities; the generic core remembers `pinned` and `last_used` choices per project and tool, resolves explicit → session → pin → last-used → global default → base-command selector → default-dir precedence, and renders the registry's selector in `LaunchSpec`. Resumes derive their account from the provider's transcript layout. Tools without a provider stay on the logged single-account floor.

Usage is a second provider slice attached to each detected account as an in-memory snapshot. Providers read native state at request time; taurhaus never logs, persists, refreshes, or otherwise owns a credential. Claude uses `CLAUDE_CONFIG_DIR` and its OAuth usage endpoint. Codex uses `CODEX_HOME`, display-only decoding of the `id_token`, and its native usage windows; API-key accounts remain selectable but explicitly report usage as unavailable. Antigravity exposes one implicit account and obtains its native windows by running `agy -p /usage --output-format json` through the injectable command boundary. Grok uses `GROK_HOME` and reads only the display names in its `auth.json`; it reports usage as unavailable because grok 1.0.5 publishes no quota endpoint, and the registry carries the sentence the UI shows in a meter's place. The retired Claude status-line bridge is uninstalled once without disturbing foreign status-line commands.

## App and daemon move together

The daemon (WSL2 on Windows, native elsewhere) owns process inventory, session identity, activity, tmux focus and the JSONL log sink. Native harness hook processes own compaction detection and call the same taurhaus hook bridge on every platform. The app and daemon speak a versioned JSON-line protocol; **the app validates the exact protocol version on every connect path** (startup, health, inline reconnects, the focus bridge's own socket) and refuses a mismatched daemon rather than half-working. Consequences:

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

## How changes are made

Each change is a small PR with red-first regression tests naming the breaking commit, implemented by one model family and reviewed by the other (Opus ↔ Codex) through two lenses — conformance to the spec, and an operational checklist (upgrade of persisted data, protocol bumps on wire vocabulary, Windows/WSL paths, user-config edit discipline, concurrency, honest tests, hygiene) — with the fix → re-review loop repeated until no majors remain. Implementers commit after every green step and never edit the ledger; the orchestrator writes the spec (reviewed by the other family first when it edits user config or persisted formats), fills the ledger at merge, and merges only on the check's conclusion. Each new CLI starts with two independent research reports (`docs/design/research/`), verified live on a host that has it; the plans' facts tables cite them.
