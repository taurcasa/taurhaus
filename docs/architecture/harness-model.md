# Harness Model

How taurhaus relates to the AI CLIs it runs (Claude Code, Codex CLI, Gemini CLI), what it owns itself, and the rules that keep the two in step. This is the architecture that landed in 0.6.4–0.6.8 (2026-08); the execution record is [`docs/design/harness-realignment-plan.md`](../design/harness-realignment-plan.md).

## The constraint that shapes everything

Claude models on a subscription are only reachable through the Claude Code CLI. Codex and Gemini have their own CLIs. So there is no single harness for all models: **Claude Code is the Claude harness, the other CLIs are theirs, and taurhaus coordinates them from outside** — tmux panes for processes, the mesh bridge for team messaging between CLIs. The rule that follows, and that every subsystem applies:

> Use what the harness does natively where it exists; tmux and mesh where it does not.

tmux + mesh is the *floor*: as long as a model has any CLI, taurhaus can reach it. Native capabilities (a sessions registry, hooks, a turn-complete notification, a peer message queue) are per-tool *upgrades* over that floor, never replacements for it.

## Capability slices, not tool adapters

Adding a CLI must touch only the slices where that tool differs; the rest of the system consumes capabilities and never branches on tool identity. Each slice has a declared default when a tool provides nothing:

| Slice | Claude Code | Codex CLI | Default (floor) |
|---|---|---|---|
| Registry entry (aliases, default base command, label, capability flags) | data | data | required |
| Process signature (argv → tool) | pattern | pattern | tool invisible, logged once |
| Launch rendering (`LaunchSpec`) | `--model`, `--effort`, `-n <agent>`, team flags | `-m`, `-c model_reasoning_effort=…`, `-c notify=[…]`, hook-trust flag | user's base command verbatim |
| Session identity | sessions registry `<CLAUDE_CONFIG_DIR>/sessions/<pid>.json` | fd-verified rollout binding | `/proc` + `tmux list-panes`, no session id |
| Busy / idle | registry `status` (authoritative) | `agent-turn-complete` notify sink (authoritative) | rchar-rate hysteresis + pane liveness |
| Message delivery + wake | inbox file, Claude's own poller | inbox file + `mesh daemon` tmux wake | inbox file + tmux wake |
| Compaction signal | `SessionStart(source=compact)` hook | same hook shape via `hooks.json` (opt-in), transcript tailer fallback | none, logged once |
| Transcript parser | JSONL | rollout JSONL (format not stable upstream) | none |
| Account selection | config-dir identities + `CLAUDE_CONFIG_DIR` | `auth.json` identity + `CODEX_HOME` | implicit single account |
| Usage | OAuth 5-hour + weekly windows | `wham/usage` 5-hour + weekly windows per model family | unavailable |
| Stop / teardown | `/exit` | interrupt | tmux kill + mesh daemon stop |

The registry (`src-tauri/src/session_scanner/cli_tool.rs`) is the one place tool identity may fan out; slices with two real implementations are traits (`SessionSource`, `ActivitySource`, `CompactionSignalSource`, `TranscriptParser`), everything else is data. A conformance suite runs every registry entry through every slice, so a new tool is proven by the same tests as the existing ones. The tracked metric is the number of `CliTool::…` branches outside the registry and slice files; it is meant to go down.

## Model and reasoning effort are first-class

`model` and `reasoning_effort` are separate fields everywhere: role templates and presets (`model:` + `reasoning_effort:`; legacy `"gpt-5.4 high"` spellings still load), persisted per team member, hydrated on resume (member → role default → catalog default), and rendered per CLI by `LaunchSpec` (`src-tauri/src/session_scanner/launch.rs`). Effort is validated per tool; an unknown or unsupported value is logged (`launch.effort.invalid`) and dropped — never silently. A backend `ModelCatalog` on the terminal contract feeds one effort-aware `ModelSelect` in the UI; the catalog is a suggestion list, not an allowlist, so user-added models keep their declared effort. Every launch logs the rendered command (`launch.command.rendered`).

## Accounts and usage

Account selection is a capability slice, not a Claude-only path. A provider discovers tool-owned config directories and identities; the generic core remembers `pinned` and `last_used` choices per project and tool, resolves explicit → session → pin → last-used → global default → base-command selector → default-dir precedence, and renders the registry's selector in `LaunchSpec`. Resumes derive their account from the provider's transcript layout. Tools without a provider stay on the logged single-account floor.

Usage is a second provider slice attached to each detected account as an in-memory snapshot. Providers read the CLI's credential file at request time and call its native usage endpoint through an injectable HTTP boundary; taurhaus never logs, persists, refreshes, or otherwise owns the token. Claude implements both slices through `CLAUDE_CONFIG_DIR` and its OAuth usage endpoint. Codex implements them through `CODEX_HOME`, display-only decoding of the `id_token`, and the native `wham/usage` windows; API-key accounts remain selectable but explicitly report usage as unavailable. Gemini declares its selector in the registry while its provider arrives independently. The retired Claude status-line bridge is uninstalled once without disturbing foreign status-line commands.

## App and daemon move together

The daemon (WSL2 on Windows, native elsewhere) owns process inventory, session identity, activity, tmux focus, compaction ownership and the JSONL log sink. The app and daemon speak a versioned JSON-line protocol; **the app validates the exact protocol version on every connect path** (startup, health, inline reconnects, the focus bridge's own socket) and refuses a mismatched daemon rather than half-working. Consequences:

- New methods and fields are additive (`#[serde(default)]`) and do not bump the version; a changed contract does.
- A version bump means the app release and `just install-daemon` ship together. Reinstalling only the daemon under an older app leaves the app daemon-less.
- The app auto-updates its bundled daemon when the installed one is older (semver); mesh is bundled, lock-pinned and installed the same way (`CONTRIBUTING.md` release checklist).

## Stability rules learned the hard way

- A scan whose inventory cannot be read is **degraded** and inert: nothing is pruned, no state changes, no exports, the last good snapshot stands; degradation is visible to the UI within a poll.
- Processes without a controlling terminal are not sessions.
- Activity events fire on real transitions only; first sight of an idle process is not a transition.
- One writer per shared file, with mesh's lock discipline (flock + inode re-check, tmp+rename under the lock); unknown fields written by other tools survive every save.
- A tmux pane is identified by pid + start time, not by pane id; a foreign pane is quarantined, never typed into.
- Compaction has exactly one owner (the daemon where configured and reachable, the app otherwise); the fallback is revoked on recovery.

## How changes are made

Each change is a small PR with red-first regression tests naming the breaking commit, implemented by one model family and reviewed by the other (Opus ↔ Codex), with the review loop repeated until no majors remain; the orchestrator writes specs for design-heavy work and makes the merge call. The per-PR ledger in the realignment plan records implementer, reviewers, rounds and what the reviewers found.
