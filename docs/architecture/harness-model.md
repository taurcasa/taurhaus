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
| Launch rendering | `--model`, `--effort`, `-n <agent>`, team flags | `-m`, reasoning config, notify + hook-trust flags | `--model`, `--effort`, `--dangerously-skip-permissions`; conversation resume | `--model`, `--effort` (validated per model), `--always-approve`; `--continue` / `--resume <uuid>` | user's base command verbatim |
| Session identity | sessions registry | fd-verified rollout binding | `last_conversations.json` + presence lock | `active_sessions.json` row bound by pid and cwd | `/proc` + tmux pane, no session id |
| Busy / idle | registry status (authoritative) | turn-complete notify (authoritative) | hooks sink, default on, needs workspace trust and agy 1.1.10; process-IO floor otherwise | `events.jsonl` turn lifecycle (authoritative) | rchar-rate hysteresis + pane liveness |
| Message delivery + wake | native inbox poller | inbox + tmux wake | inbox + tmux wake | inbox + tmux wake (plain Enter queues, Ctrl+Enter interjects) | inbox + tmux wake |
| Compaction signal | `SessionStart(source=compact)` hook | opt-in hook, transcript tailer fallback | unavailable, `compaction.unsupported` logged once | own file in the always-trusted `~/.grok/hooks`; `PostCompact` fires the bridge (grok's start source is never `compact`), `SessionStart(compact)` catches the registration grok imports from Claude and is deduped; hooks are registered under PascalCase names but grok's envelope spells the `hookEventName` **value** in snake_case (`post_compact`), so the bridge matches either spelling; the card is queued in the mesh inbox because grok discards passive-hook stdout | none, logged once |
| Transcript parser | JSONL | rollout JSONL | none | none | none |
| Account selection | config-dir identities + selector | `auth.json` identities + selector | one implicit OAuth account under the shared Google tooling root | `auth.json` identities + `GROK_HOME` | implicit single account |
| Usage | OAuth usage windows | native 5-hour + weekly windows | native `/usage` command through an isolated provider process | unavailable; no quota endpoint, per-turn cost is in-band | unavailable |
| Stop / teardown | `/exit` | interrupt | `/exit`, wait for presence lock, then kill floor | `/quit`, wait for the registry row to clear, then kill floor | tmux kill + mesh daemon stop |

The registry (`src-tauri/src/session_scanner/cli_tool.rs`) is the one place tool identity may fan out; slices with two real implementations are traits (`SessionSource`, `ActivitySource`, `CompactionSignalSource`, `TranscriptParser`), everything else is data. A conformance suite runs every registry entry through every slice, so a new tool is proven by the same tests as the existing ones. The tracked metric is the number of `CliTool::…` branches outside the registry and slice files; it is meant to go down.

## Model and reasoning effort are first-class

`model` and `reasoning_effort` are separate fields everywhere: role templates and presets (`model:` + `reasoning_effort:`; legacy `"gpt-5.4 high"` spellings still load), persisted per team member, hydrated on resume (member → role default → catalog default), and rendered per CLI by `LaunchSpec` (`src-tauri/src/session_scanner/launch.rs`). Effort is validated per tool; an unknown or unsupported value is logged (`launch.effort.invalid`) and dropped — never silently. A backend `ModelCatalog` on the terminal contract feeds one effort-aware `ModelSelect` in the UI; the catalog is a suggestion list, not an allowlist, so user-added models keep their declared effort. Every launch logs the rendered command (`launch.command.rendered`).

## Accounts and usage

Account selection is a capability slice, not a Claude-only path. A provider discovers tool-owned config directories and identities; the generic core remembers `pinned` and `last_used` choices per project and tool, resolves explicit → session → pin → last-used → global default → base-command selector → default-dir precedence, and renders the registry's selector in `LaunchSpec`. Resumes derive their account from the provider's transcript layout. Tools without a provider stay on the logged single-account floor.

Usage is a second provider slice attached to each detected account as an in-memory snapshot. Providers read native state at request time; taurhaus never logs, persists, refreshes, or otherwise owns a credential. Claude uses `CLAUDE_CONFIG_DIR` and its OAuth usage endpoint. Codex uses `CODEX_HOME`, display-only decoding of the `id_token`, and its native usage windows; API-key accounts remain selectable but explicitly report usage as unavailable. Antigravity exposes one implicit account and obtains its native windows by running `agy -p /usage --output-format json` through the injectable command boundary. Grok uses `GROK_HOME` and reads only the display names in its `auth.json`; it reports usage as unavailable because grok 1.0.5 publishes no quota endpoint, and the registry carries the sentence the UI shows in a meter's place. The retired Claude status-line bridge is uninstalled once without disturbing foreign status-line commands.

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
- A harness that imports another vendor's hook registrations (grok reads `~/.claude/settings.json` by default) can invoke one bridge twice for one event; the registry declares that and the bridge deduplicates, so one compaction is one reinjection.
- The hook that observes a compaction is not always the channel that can deliver the card: the registry names the delivery per harness (`additionalContext` on the hook's stdout for Claude Code and Codex, the member's mesh inbox for grok, whose passive-hook stdout is documented as ignored), and the delivery is recorded only once it has actually happened.

## Retired

- **Gemini CLI** (0.8.0): Gemini Code Assist for individuals refuses the client ("migrate to the Antigravity suite"), so the registry entry, launch arm, TCP idle heuristic, task scanner, catalog entries and role templates are gone. Persisted `gemini` tool values load as an unknown tool instead of aborting the record that carries them — a role catalog or team config from 0.7.x still opens; re-pick Antigravity where a member needs it. There is deliberately no alias: `agy` is a different binary with different flags and directories.
- **Claude status-line bridge** (0.7.0): the 0.6.8 wrapper around `settings.json`'s `statusLine` could never carry the per-model buckets and edited user config; the OAuth usage endpoint replaced it, and the bridge is uninstalled once, restoring the original status line byte-for-byte.
- **Gemini CLI account/usage provider** (never shipped): the fixture-driven `retrieveUserQuota` provider planned as 17d was cancelled with the CLI.
- The eight architecture infographics under `docs/images/` were regenerated on 2026-08-28 from their prompts in `infographics.manifest.yaml` (`just infographics`, see `docs/operations/infographics.md`).

## How changes are made

Each change is a small PR with red-first regression tests naming the breaking commit, implemented by one model family and reviewed by the other (Opus ↔ Codex) through two lenses — conformance to the spec, and an operational checklist (upgrade of persisted data, protocol bumps on wire vocabulary, Windows/WSL paths, user-config edit discipline, concurrency, honest tests, hygiene) — with the fix → re-review loop repeated until no majors remain. Implementers commit after every green step and never edit the ledger; the orchestrator writes the spec (reviewed by the other family first when it edits user config or persisted formats), fills the ledger at merge, and merges only on the check's conclusion. Each new CLI starts with two independent research reports (`docs/design/research/`), verified live on a host that has it; the plans' facts tables cite them.
