# Changelog

All notable changes to taurhaus are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Changed

- **Bundled mesh 0.2.21** — mesh recognises `agy` and `grok` member CLIs (Gemini CLI removed, no alias), carries per-CLI submission behaviour (grok: `ACTION REQUIRED:` notices interject with `C-i`, everything else queues on Enter; claude/codex/agy sequences unchanged), and its USAGE guide covers launch flags and `/exit` / `/quit` stops for the four harnesses.
- **`v3-developer-agy` role** — the Antigravity counterpart of the V3 vertical-slice developer, same contract and gates, running `gemini-3.7-flash-high` at high effort.
- **Antigravity activity hooks are on by default** — agy 1.1.22 was observed firing `PreInvocation` and `Stop` for interactive sessions once the workspace is trusted, so the busy/idle sink no longer has to be opted into. It stays inert until the member answers the folder-trust prompt on first launch, which Antigravity onboarding now spells out, and it is gated on agy 1.1.10 or newer because `Stop` never fires below that. An agy version that cannot be resolved leaves an installed hook alone rather than uninstalling it, and the gated outcome is logged once per run as `agy.hooks.degraded`.
- **Antigravity hooks are written to the shared `~/.gemini/config/hooks.json`** — agy 1.0.8 moved user-level hooks there and its migration symlinks the old `antigravity-cli/hooks.json` onto it. taurhaus now merges its single entry into the shared file by hook name (anything else in the file is preserved), follows a symlinked target instead of replacing it with a private regular file, and clears any entry left behind in the legacy path. The `Stop` payload's `terminationReason` is kept as an open string — only `NO_TOOL_CALL` has ever been observed, and an unseen value must never drop an idle edge. `harness.agy_hooks` also gained the snake_case alias its siblings have; without it the setting the frontend sends was silently discarded.

### Added

- **`v3-developer-agy` role** — the Antigravity counterpart of the V3 vertical-slice developer, same contract and gates, running `gemini-3.7-flash-high` at high effort.

### Fixed

- **Antigravity hooks are written to the shared `~/.gemini/config/hooks.json`** — agy 1.0.8 moved user-level hooks there and its migration symlinks the old `antigravity-cli/hooks.json` onto it. taurhaus now merges its single entry into the shared file by hook name (anything else in the file is preserved), follows a symlinked target instead of replacing it with a private regular file, and clears any entry left behind in the legacy path. The `Stop` payload's `terminationReason` is kept as an open string — only `NO_TOOL_CALL` has ever been observed, and an unseen value must never drop an idle edge. `harness.agy_hooks` also gained the snake_case alias its siblings have; without it the setting the frontend sends was silently discarded.

## [0.8.1] - 2026-08-28

### Changed

- **Infographics regenerated** — the eight architecture diagrams are rendered again from their manifest prompts (`gpt-image-2`, via `just infographics` reading `OPENAI_API_KEY` from `.env`); the stale-render callouts are gone. Grok and Antigravity use their real brand marks (thesvg.org, MIT; trademarks of xAI and Google).

## [0.8.0] - 2026-08-28

Two harness changes and a documentation reset. Google's coding CLI is now the **Antigravity CLI (`agy`)** — Gemini Code Assist for individuals refuses the old Gemini CLI client, so Gemini CLI support is removed — and the **Grok CLI (`grok`)** joins as a fourth harness. Both were added slice by slice behind the tool registry (nothing outside the registry and the per-tool slice files learned a new tool name), and the documentation set was swept against the code. Daemon protocol is **13** — the app and the bundled daemon update together (`just install-daemon` after `just install-windows`).

### Added

- **Grok CLI (`grok`) added as a harness** (#40) — registry entry, process detection (interactive TUI vs headless drivers, agent services and management subcommands), `--model`/`--effort`/`--always-approve` launch rendering with per-model effort validation, session identity from `active_sessions.json`, authoritative busy/idle from the session's `events.jsonl` turn lifecycle, continue and resume from the project's persisted session history in the account home the launch selects (grok clears its live registry on `/quit`), `/quit` with the registry row as the clean-stop proof, given a 15-second stop timeout so grok's own documented ten-second exit budget can run before the pane is killed, `GROK_HOME` accounts, compaction hooks in grok's always-trusted `~/.grok/hooks` directory whose restored-context card arrives through the mesh inbox on grok's own `PostCompact` event because grok discards passive-hook stdout and never reports a `compact` start source, a `grok-developer` role and a `Grok Pair` preset so a grok member can actually be staffed, and a graphite UI identity across sidebar, team builder, mesh runtime and Settings. Because grok also loads `~/.claude/settings.json` hooks, one compaction can reach the bridge through two registrations; the registry declares that and the bridge deduplicates, so one compaction is still one reinjection. Usage windows are deliberately absent: grok 1.0.5 publishes no quota endpoint and Settings says where usage lives instead.
- **Antigravity CLI (`agy`) is the Google harness** — registry, process detection, model/effort launch rendering (`--dangerously-skip-permissions` as the auto-approve), conversation identity, presence-aware stop, native `/usage` windows (Gemini / Claude+GPT groups, weekly + 5h), an implicit account, opt-in activity hooks, Google-blue UI treatment, bundled roles, and conformance coverage now ship as one capability-sliced integration. (#39)
- **Process inventory keeps argv boundaries on Linux** — `/proc/<pid>/cmdline` elements are classified as delivered, with `--` ending option parsing and a trailing COMMAND recognised, so `grok "help me"` is a session and `grok help` is not; the macOS `ps` path remains a documented lossy fallback. (#40)
- Registry capabilities `auto_approve_flag` and `managed_home` replace the last tool-identity proxies. (#39)

### Changed

- **Documentation swept against the code** — README, ARCHITECTURE, CLAUDE.md, CONTRIBUTING, AGENTS, `docs/**`, e2e README and tool-naming module docs (35 files; Opus drift sweep, Codex claim verification over four rounds): Gemini CLI references removed or rewritten for Antigravity, Grok added to every tool enumeration, accounts/usage and `just visual-shot` documented, counts re-measured (90 IPC commands, 28 daemon methods, protocol history 11 → 12 → 13); `docs/architecture/harness-model.md` gained a "Retired" section and the current review process. The eight architecture infographics under `docs/images/` are marked stale until regenerated from their (corrected) prompts. (#41)
- **Gemini CLI removed** (#39) — Google now refuses that client for individual Code Assist accounts and directs users to Antigravity. Persisted `gemini` tool values deliberately load as an unknown tool; reselect Antigravity explicitly because it is a different binary with different flags, and no compatibility alias is provided.
- **Daemon protocol 13** — the `grok` tool wire value joins the vocabulary that protocol 12 fixed at three harnesses. The app and the bundled daemon ship together; reinstall the paired daemon with `just install-daemon`, because a protocol 12 daemon would decode `grok` sessions as an unknown tool and is deliberately rejected instead.
- **Daemon protocol 12** — the Antigravity tool wire value replaces the retired Gemini value. Reinstall the paired daemon with `just install-daemon`; protocol 11 daemons are deliberately rejected instead of failing individual launch and stop requests.
- Mesh and taureval need separate follow-ups before their own external role/tool contracts can launch `agy` or `grok`; these PRs intentionally do not modify those repositories.

### Security

- Dependency patches from Dependabot: `openssl` 0.10.81 / `openssl-sys` 0.9.117 (pulled in by `reqwest`'s `native-tls` in 0.7.0).

## [0.7.0] - 2026-08-28

Accounts and usage become first-class across CLIs: every tool that can run on more than one account (Claude Code via `CLAUDE_CONFIG_DIR`, Codex via `CODEX_HOME`) gets the same flow — detection, a per-project memory that follows the account the project used last, a one-gesture choice from the sidebar's right-click menu, resume on the account that owns the session, and usage shown exactly as the CLI's own `/usage` or `/status` shows it. Daemon protocol is now **11** — the app and the bundled daemon update together and refuse a mismatched pair.

### Added

- **Account choice where you launch** — every Claude and Codex launch entry in the sidebar's right-click menu (New / Continue / Resume / restart) opens a submenu of accounts with their usage and a tick on the one that would be used; picking one launches immediately without a modal. A `<Tool> account` submenu pins or clears the project's account. Team-delegated Continue/Resume say so instead of silently ignoring a pick. (#34)
- **Usage like the CLIs show it** — Claude: *Current session · Current week (all models) · Current week (Fable)* from Claude Code's own OAuth usage endpoint; Codex: *5h limit · Weekly limit* per model family from `wham/usage`. Meters live on the account chip, in the chooser, in the submenus and in Settings → Accounts; compact meters show the weekly buckets. Tokens are read at request time, kept in memory only, never logged, persisted or refreshed by taurhaus — an expired or rejected credential shows as "sign in again" until the CLI refreshes it. (#35, #38)
- **Per-project account memory** — a project defaults to the account it last used (from taurhaus launches or sessions taurhaus sees running), then to a pin, a global default, or a selector already in your launch command (`claude2`); Settings → Accounts shows the effective default and *why*. (#35)
- **Generic account and usage core** — `AccountProvider`/`UsageProvider` capability slices behind the tool registry; adding a tool's accounts or usage touches only its slice. Migration 013 (`project_tool_accounts`) carries the 0.6.8 Claude pins over. (#35)
- **`just visual-shot`** — screenshots of any visual-host fixture through Windows Edge headless (`?component=&scenario=&viewport=&theme=`), used to reproduce and verify UI fixes with real renders. (#34)

### Changed

- **Daemon protocol 11** — generic `list_accounts`, `project_transcript` and `refresh_usage` replace the Claude-only methods; the 0.7.0 app and daemon ship together (`just install-daemon` after `just install-windows`) and reject a mismatched pair. (#35)
- **Status-line bridge retired** — the 0.6.8 bridge that wrapped your Claude `statusLine` is removed and uninstalled once on first start (your original status line is restored byte-for-byte; a status line that merely references the old script is left alone). It could never carry the per-model buckets and it edited user config. (#35)
- Research reports backing the accounts/usage design (Codex, Gemini, Antigravity, Grok CLIs) live under `docs/design/research/`. (#36, #37)

### Fixed

- **Account popups no longer land at the bottom of the window half cut off** — the shell frame's `position: relative` rule overrode the chooser overlay, and the chip menu was clipped by its header; both are viewport-anchored now, reproduced and verified with real renders. (#34)
- Codex accounts without an `auth_mode` key and compact meters for providers without flagged weekly windows render correctly. (#38)

## [0.6.8] - 2026-08-27

Harness realignment wrap-up. Several Claude subscriptions per host become a first-class choice (per project, with live usage per subscription), Codex idle detection is authoritative via its own turn-complete notification, tool-specific code is confined to capability slices behind one registry, and the documentation set describes the code as it is. Daemon protocol stays **10** — the bundled daemon is still updated with the app (new additive daemon method: `LIST_CLAUDE_ACCOUNTS`).

### Added

- **Choose the Claude subscription per project** — taurhaus detects every Claude Code config dir (`~/.claude`, `~/.claude-*`, live `CLAUDE_CONFIG_DIR`s), labels them from their signed-in account, and asks once per project which one to use when more than one is logged in (remembered per project; global default in Settings → Claude accounts). Launches render `CLAUDE_CONFIG_DIR=…` (a base command that already sets it wins); resume/continue derive the account from the session's transcript location, so a session always resumes on the subscription that owns its history. Teams keep running on the default config dir (per-team accounts are a follow-up). (#27)
- **Usage per subscription** — each account's 5-hour and 7-day usage (`used_percentage`, resets-in) is shown on the account chip and in the chooser, fed by Claude Code's documented status-line payload through a per-account bridge that wraps any status line you already have (it keeps rendering) or renders a minimal one. The bridge is daemon-owned, idempotent, removable, exact-command-aware, symlink-safe, private (0700/0600 artifacts), reconciled on every accounts request, and gated on Claude ≥ 2.1.246. Usage flows while an interactive session of that account is open. (#31)
- **Codex idle is authoritative** — Codex launches carry `-c notify=[…]`, and the `agent-turn-complete` notification lands in a daemon sink that classification treats as the authoritative busy/idle edge (rchar-rate hysteresis stays as the fallback). Native CLI versions are probed once per run (`claude --version`, `codex --version`) and exposed on the terminal contract; capability gates (Claude `--effort`, Codex `-c notify`, Codex hooks) read them. (#25)
- **Capability-sliced tool registry** — a single `CliToolSpec` registry describes each CLI (aliases, argv signatures, default commands, label/accent, capability flags); session identity, activity, compaction signal and transcript parsing are traits with per-tool implementations; the frontend reads tool descriptors from the terminal contract via `toolRegistry.js`. A conformance suite runs every registry entry through every slice against golden launch fixtures, and module-boundary guards fail the build when a tool literal appears outside the allowed files. Metric: `CliTool::…` branches outside registry/slice files 421 lines / 66 files → 340 / 52; frontend tool-name comparisons 33 → 1. No behaviour change; rendered commands are byte-identical. (#33)

### Changed

- **Documentation realigned with the code** — README, ARCHITECTURE, CLAUDE.md, CONTRIBUTING and `docs/**` swept for drift (152 verified fixes), plus a new `docs/architecture/harness-model.md` explaining the harness model: Claude Code hosts Claude, other CLIs host theirs, tmux + mesh is the floor, capability slices, app ↔ daemon pairing, stability rules. (#28, #29, #30)
- **Mesh bundling and daemon install** — `resolve-mesh-binary.sh` rebuilds mesh when the lock's `git_commit` no longer matches the checked-out mesh; `just install-daemon` restarts the daemon with explicit `--data-dir`/`--port` and the previous process's environment; the taureval harness reads role `model`/`reasoning_effort` with the same fidelity as taurhaus. (#24, #26)
- **Windows build runner fails honestly** — `scripts/build-windows.ps1` now throws when `bun install` or `tauri build` exits non-zero; previously a failed `tauri build` still ended in "Windows build complete". Tauri-generated `src-tauri/gen/schemas` are no longer tracked (they were gitignored but three files had stayed in the index, dirtying the tree on every tauri bump).

### Testing

- **Five load-sensitive tests made deterministic** — the runtime scan override is scoped to the test that installs it, the mesh detail test holds the component to its scheduling rather than a wall-clock budget it cannot measure, launch log assertions flush the sink and select records by their emitter, the live `/proc` scan waits for the child's exec, and daemon-unavailable is injected instead of racing a freed port. (#32)

### Security

- Dependency patches from Dependabot (moderate): `serde_with` 3.22.0, `tauri` 2.11.1.

## [0.6.7] - 2026-08-26

Follow-up to 0.6.6. One activity signal across the UI, honest wall-clock activity accounting, compaction reinjection for Codex via its native hooks (opt-in), and two post-0.6.6 noise regressions fixed. Daemon protocol is now **10** — app and daemon update together.

### Added

- **Codex compaction reinjection via Codex's own hooks** — `SessionStart(source=compact)`/`PostCompact` from Codex now flow through the same bridge Claude uses (tool inferred from the transcript path), with a managed `hooks.json` installer (idempotent, removable, exe-path self-repairing). Off by default: Settings → Mesh → "Codex compaction source" stays on the transcript tailer until the hook path is validated on a live team. Empirically proven on Codex 0.147/0.149 (`additionalContext` injection, exact-once events).

### Changed

- **One activity signal** — every surface (sidebar, hover card, mesh canvas/nodes/runtime) derives its status from a single module: working/active/idle/uncertain/offline, with attribution deciding working vs uncertain and reused/dead panes reading as offline. Activity time (`active` percentages) now accumulates only observed wall-clock intervals — no tick math, no backfilled blackouts; scanner degradations are visible to the UI within a poll via a dedicated cursor.

### Fixed

- **Post-0.6.6 event noise** — first sight of an idle process no longer logs a state change, and background CLI processes without a controlling terminal (e.g. detached `codex exec` automation) are no longer treated as sessions at all: no phantom sidebar rows, no tracker churn (measured: 64.5 → ~5 events/min on a busy host, inventory perfectly stable).


## [0.6.6] - 2026-08-26

Harness realignment release. Fixes the three long-standing live bugs (session-indicator blackouts, permanently-uncertain activity icons, the dead tmux focus indicator), makes model and reasoning effort first-class end to end, and hardens the coordination stores against the mesh bridge. Daemon protocol is now **8** — the app auto-updates its bundled daemon on startup; running app and daemon must be updated together.

### Fixed



- **Activity icons no longer stay "uncertain" forever** — Claude session identity and busy/idle now come from Claude Code's own sessions registry (`<CLAUDE_CONFIG_DIR>/sessions/<pid>.json`), resolved per process under that process's `CLAUDE_CONFIG_DIR`, so sessions on alternate config dirs finally bind to their transcripts. Registry states (`busy`/`idle`/`waiting`/`shell`) are authoritative and skip the I/O heuristics; rchar activity is a rate, not bytes-per-poll; Codex transcript bindings need fd proof before they persist.
- **The tmux focus indicator works again and no longer depends on tmux hooks** — focus is probed by the daemon hub (`tmux list-clients`, per-pane resolution) and travels inside the versioned session snapshot; the entire hook → focus-file → inotify chain is gone, so an env-less daemon restart or a rebooted tmux server can no longer kill the indicator. Every daemon connect path validates the protocol.
- **Mesh interop** — taurhaus no longer erases mesh-written `config.json` fields (auth hashes, activity/status fields) on save; operator notices are written once to the member inbox with a truthful delivery audit (no sender-candidate chain, no self-send); Claude team leads join mesh so the team daemon can start (skipped with a reason when no credential exists); stale tmux panes are detected by pane identity (pid + start time) and quarantined per member instead of restarting mesh daemons into foreign panes; onboarding text teaches real mesh commands (`mesh tasks`, `mesh task get`, lifecycle verbs) instead of a nonexistent `task list` and a lead-only `task update`.
- **Compaction pipeline hardening** — one teams-dir authority, the signal watcher survives failed signals and truncated state, the extractor heartbeat is sampled (≤1/min), offsets and bindings survive empty scans, and compaction has exactly one owner (daemon when configured and reachable, app otherwise, fallback revoked on recovery).

### Changed

- **Model and reasoning effort are separate fields end to end** — roles/presets (`model:` + `reasoning_effort:`, legacy `"gpt-5.4 high"` spellings still load), persisted per member, hydrated on resume (member → role → catalog), and rendered per CLI (`codex -m … -c 'model_reasoning_effort="…"'`, `claude --model/--effort/-n <agent>`). Roles that declared `"gpt-5.4 high"` previously ran at the user's global Codex effort; `gpt-5.3` is no longer rewritten to an alias that fails on ChatGPT auth. Every launch logs the rendered command.
- **Model catalog in the UI** — one effort-aware `ModelSelect` everywhere a model is chosen (unknown values preserved as custom entries, deprecation hints), fed by a backend catalog on the terminal contract; preset overrides now capture user intent only, and preset lead pins persist.
- The never-instantiated stall-detector module (~3.2k LOC) was removed.


### Fixed

- **Session indicators no longer black out for seconds at a time** — the process inventory is fail-soft. The scanner reads `/proc/*/cmdline` directly on Linux (`ps` stays for macOS, now with its stdout drained concurrently so a large argv can no longer block it past the 2 s budget; a stdout read error or drain-thread failure is a degraded read, never a partial inventory; Windows has no native inventory and never invokes `ps`); a scan whose inventory cannot be read is reported as degraded and is inert: it short-circuits before Codex binding reconciliation, idle detection, process-I/O sampling, hysteresis and `activity.state.changed`, returns the last fully classified snapshot (`scan_sessions_for_display`/`scan_sessions_for_runtime` now return the degraded flag alongside the sessions), no activity trackers are pruned, the daemon hub neither bumps its snapshot version nor exports stall snapshots nor resets its cadence, and the frontend ignores snapshots without a sessions array instead of flushing activity stats. A healthy empty inventory now hits the scan cache instead of re-walking `/proc` every cycle, and `process_scan_ms` covers the fingerprint read too. New structured events: `session_scanner.process_scan.degraded` (edge-triggered: once on entry, a bounded 60 s reminder while it lasts) and `session_scanner.process_scan.recovered`, plus `activity.state.changed {pid, tool, from, to, source}`. Activity snapshot export also skips teams with no live tmux pane instead of probing every team that ever had a runtime record. Stateful consumers treat a degraded scan as no observation: member session-identity detection never binds the cached pane→session mapping (it keeps polling and reports no session if the outage outlasts the detection window), the local session-list fallback returns the cached sessions for continuity but no longer promotes project activity from them, and the display and runtime entry points share one last-good snapshot so the first degraded runtime scan of an outage sees what the authoritative scan last classified. On Windows (app + WSL daemon) the same rule now crosses the daemon boundary: the daemon hub marks the snapshot it preserves across degraded scanner cycles as `degraded` (cleared on the next healthy cycle), `get_runtime_session_snapshot` carries that flag (additive, older daemons decode as healthy), and the Windows app reads its session scan from that snapshot, so a degraded daemon snapshot stays continuity data — identity detection keeps polling instead of binding the cached pane→transcript mapping, and the daemon session list no longer promotes project activity from it. Regression tests added, including end-to-end degraded-scan coverage through an injectable inventory provider, identity detection across degraded scans (local and through a scripted daemon snapshot), a failing stdout reader, and cfg-independent inventory-backend selection (Linux→`/proc`, macOS→`ps`, Windows→none).

## [0.6.5] - 2026-08-21

Patch release: repairs the bundled mesh resource. The v0.6.4 Windows installer shipped `resources/mesh` as a directory instead of the mesh binary, which broke mesh installation on fresh installs (machines with an already-installed matching mesh were unaffected).

### Fixed

- **Bundled mesh resource shipped as a directory** — a stray directory at `src-tauri/resources/mesh` made `just bundle-mesh` copy the binary to `resources/mesh/mesh`, and the runtime accepted it because it only checked existence. The resolver now requires a regular file and reports a clear error, and the `bundle-mesh` recipe removes a directory at the target before bundling and asserts the result is a file. Regression tests added.

## [0.6.4] - 2026-08-20

Patch release: CLI launch commands configured in Settings are now free-form, fixing launches for users with aliased or alternate Claude/Codex/Gemini installs.

### Fixed

- **CLI launch commands are free-form** — the commands configured in Settings → CLI commands (Claude/Codex/Gemini × fresh/continue/resume) are no longer restricted to an exact `claude`/`codex`/`gemini` binary name or filtered for shell syntax. Aliases, alternate installs (`claude2`), environment prefixes (`CLAUDE_CONFIG_DIR=… claude`), wrappers, and ordinary shell syntax now launch exactly as they would in your terminal. Previously such commands failed with "Could not start Claude. Please try again." Only empty or multi-line commands are rejected. The validation runs in the daemon, which this release bundles and auto-updates on startup.

## [0.6.3] - 2026-03-29

Pipeline unification release. Unified the initialize, resume, and add-agent member-activation flows into a single shared executor, eliminating a class of divergence bugs. Added live per-member progress during team resume, replacing the previous dead-air experience.

### Pipeline Unification

- **Shared member-activation executor** — initialize, resume, and add-agent now delegate per-member activation through one canonical pipeline with shared stages: acquire pane, launch session, capture identity, join mesh, start daemon, commit runtime, deliver onboarding.
- **Canonical stage vocabulary** — `MemberActivationStage` enum defines the authoritative stage list used by all wrappers and progress events.
- **Explicit onboarding delivery policy** — wrappers declare `deferred_barrier` (initialize) or `immediate` (resume, add-agent) instead of relying on incidental helper placement. Structurally eliminates the race class behind the original resume member drop bug.
- **Unified session launch/capture** — three separate session detection implementations merged into one shared helper for Claude and Codex.
- **Unified mesh join and daemon start** — shared helpers with policy hooks for resume-specific stale-pid handling.
- **Shared runtime commit helper** — one path for persisting pane, session, daemon, and metadata state.
- **Team-daemon ownership consolidated** — each wrapper ensures the team daemon once at the wrapper level, removing duplicate member-level side effects.
- **Cross-wrapper parity tests** — dedicated test coverage proving shared stage alignment and explicitly asserting legitimate wrapper differences.
- **Initialize batch-stage adapter** — shared executor powers initialize while preserving the existing `MeshInitProgress` step UI contract.

### Resume Progress UX

- **Streamed per-member progress** — team resume now emits live progress events per member and per stage, replacing the static placeholder list that showed no feedback until completion.
- **Runtime snapshot freshness** — runtime bar shows "Up to date", "May be slightly outdated", or "Loading latest status…" so users know whether the view is live or stale.
- **Expanded resume summary** — completion tray now shows partial failures, daemon warnings, no-op resumes, and backend warnings translated to user language. Footer hidden on clean success to reduce noise.

### Fixes

- **Resume member drop** — fixed a race condition where the last member in the roster was not resumed because onboarding was sent before the member's inbox was ready. Added bounded retry delivery as an immediate fix, then structurally resolved via the unified onboarding policy.
- **Terminal on resume** — terminal now opens when resuming or starting a mesh team.
- **Compact hook log sink race** — fixed test race condition in compact hook log sink.
- **Onboarding E2E** — updated fake tmux to handle `list-windows` and `list-panes` commands.
- **Clippy cleanup** — replaced `map_err` with `inspect_err` for logging-only closures, extracted type aliases for complex callback types.

### Quality & Stability

- **IPC error standardization** — remaining IPC commands migrated to `IpcResult<T>`.
- **Daemon fail-fast normalization** — unified foreground read behavior when daemon is busy.
- **WSL distro selection** — coordination bridge now threads WSL distro from startup daemon, fixing split-brain discovery.
- **Windows Claude dir override** — coordination respects `TAURHAUS_CLAUDE_DIR` override consistently.
- **UNC team state saves** — hardened for Windows UNC path handling.
- **Tmux cold-start bootstrap** — more resilient tmux session initialization.
- **Vitest jsdom worker** — fixed ESM startup issue in frontend test suite.

### Infrastructure

- **Frontend-design developer role** — new role template for Claude agents using the `/frontend-design` skill for production-grade UI work.
- **Architecture review documents** — resume progress UX assessment, pipeline unification assessment, and phased implementation plan at `docs/reviews/`.
- **Event processor split proposal** — design document for queue-based event processor refactor.
- **SQLite pooling proposal** — design document for dual-pool connection strategy.

## [0.6.2] - 2026-03-22

Role system evolution release. Extended role schema with workflow metadata, created eval-validated new roles, and fixed tmux focus detection.

### Role Schema Evolution

- **7 new role fields** — `communication_style`, `quality_gates`, `definition_of_done`, `phase_scope`, `mode`, `inherits_from`, `required_artifacts`. All optional, backward-compatible with existing roles.
- **All 37 built-in roles enriched** — every role now has communication style, quality gates, completion checklists, phase scope, and mode tags with role-specific values.
- **Behavioral contracts strengthened** — escalation triggers, handoff expectations, and quality gate language improved across all roles based on ECC and BMAD-METHOD research.
- **Import/export round-trip** — new fields survive export to Claude Agent, Copilot Agent, and YAML formats and parse correctly on import.

### New Roles

- **Adversarial Reviewer** (Claude, 90% eval score) — skeptical code reviewer that assumes defects exist, produces evidence-backed findings with file:line references, and confirms clean code without inventing issues.
- **Docs Verifier** (Codex, 93% eval score) — confirms documentation claims against primary sources, flags stale references, and refuses to blindly trust unverifiable assertions.
- **Quick Dev** (Codex, 90% eval score) — low-ceremony implementer for small tasks with mandatory final review reporting. Always reports what changed and what to verify.

### Role Detail & Editing

- **New fields in detail view** — communication style shown as paragraph, quality gates and definition of done as checklists, phase scope as pill badges, mode as a badge.
- **Edit mode supports new fields** — auto-growing textareas for text, bullet list editors for lists, dropdown for mode, tag input for phase scope.
- **Mode filter in roster builder** — filter roles by Implementation, Review, Research, or Coordination mode.

### Fixes

- **Tmux focus hook** — fixed JSON quoting that caused malformed writes (literal backslashes in JSON), restoring foreground project detection on window switch.
- **Onboarding summary** — Claude agents now receive explicit SendMessage summary instructions, eliminating "summary is required" errors during team operation.
- **Preset composition** — Pair preset updated to use quick-dev role with appropriate naming.

### Research & Validation

- **ECC + BMAD research** — analyzed everything-claude-code and BMAD-METHOD projects from 4 perspectives (architect, design lead, product, developer) to inform role system improvements.
- **Taureval baseline** — all 9 roles scored 86-100% after infrastructure fixes (stale inbox, context bleed) and one optimization iteration.
- **Resource monitoring** — confirmed stable daemon profile (~23MB RSS, 4 inotify instances) with no regressions.

## [0.6.1] - 2026-03-21

Mesh team setup and role management overhaul. Roster builder redesigned from the ground up, role detail view with markdown rendering and in-place editing, role CRUD with YAML import/export.

### Roster Builder

- **Two-column "Roster Builder" layout** — roles always visible on the left with search, tool/kind filters, favorites, and presets. Your team builds on the right with compact member cards. Replaces the old side-by-side form-based setup.
- **Compact role catalog** — single-line role rows with behavioral summaries, tool icons, Lead/Agent badges, and one-click add. Favorites pinned at top.
- **Version dedup** — only the latest version of each role shown by default. "Show all versions" toggle for legacy access.
- **Preset → customize flow** — quick presets (Pair, Dev Team, Full Team, Research) load into the editable roster for customization before initialization.
- **Interaction feedback** — hover highlights, click-to-add flash, card entry/exit animations, star bounce on pin toggle.
- **Proper member naming** — multiple developers correctly named dev-1, dev-2, dev-3 instead of dev-1-1-2.
- **Project selector** — dropdown of registered projects instead of manual path entry.

### Role Detail View

- **Full-screen page view overlay** — 640px centered reading column with markdown-rendered content. Controls (Resume, Stop, Focus Pane, Capture) pinned at top, never buried below scroll.
- **Formatted role descriptions** — Focus Area in a tinted card, Context Summary as proper paragraphs, Behavior Boundaries as styled lists. No more wall of unformatted text.
- **Compact hover popover** — hovering a mesh node shows a glanceable summary (name, status, one-line focus area, "click for details") instead of the full description.
- **Dual context** — runtime mesh shows operational controls, roster management shows "Add to Team".

### Role Editing & Management

- **In-place stacked sections editor** — edit roles in a document-like view with auto-growing textareas, borderless fields, and consistent typography across light and dark themes.
- **Role CRUD** — create new roles, edit existing ones, and delete with confirmation, all from the role detail view.
- **YAML import/export** — import role definitions from .yml files and export roles as YAML for sharing.

### Fixes

- **Team initialization from roster builder** — preset-derived rosters properly detach to custom mode before initialization, fixing "failed to set up team" errors.
- **Legacy mesh config compatibility** — team configs without a top-level name field are now accepted during discovery instead of showing undismissable "Skipped team folder" warnings.
- **CI quality gate** — fixed lint drift (clippy warnings, unused exports) so the GitHub Actions quality gate passes clean.
- **Light/dark theme parity** — roster builder and role detail view properly follow standard theme logic in both modes.

## [0.6.0] - 2026-03-20

Major release combining a thorough quality phase with significant Windows/WSL stability improvements and Mesh runtime hardening. 359 commits since v0.5.9 — every user-facing surface was audited for functional honesty, error messaging, accessibility, and documentation accuracy.

### Quality & Reliability

- **Settings now control real behavior** — scan directories and ignore patterns were previously saved but silently ignored by the scanner and search indexer. They now drive both project discovery and index rebuilds, with "Active" badges in Settings confirming enforcement.
- **Accurate session duration tracking** — session activity durations were undercounted by 10x due to a polling-interval mismatch. Fixed to use the correct interval for both Tauri and mock modes.
- **Consistent terminal settings** — terminal emulator defaults are now sourced from a shared backend contract so the frontend, backend, and tests all agree on what's available per platform.
- **Event-driven updates restored** — tmux focus changes, file modifications, task updates, and session state changes now propagate in real-time. Fixed multiple root causes: missing tmux focus hook install, overlapping activity-watch reconciles, and missing startup watch triggers.
- **Honest UI labels** — mesh node "Stop" button renamed to "Remove" (it removes from team, not just stops), dead Edit/Remove buttons removed from Overview tab, "Open in Terminal" now shows a notice instead of silently failing when no session exists.
- **Mesh cold-resume detection** — restarting the app with an existing team now correctly surfaces the Resume Team UI instead of showing the setup flow.
- **Mesh runtime status honesty** — when all team members are stopped, the runtime bar now shows "All members stopped" instead of "Team running normally", and hides the Add Agent button until the team is active.
- **Mesh pane-loss reconciliation** — when a member's tmux pane dies, the member is now reconciled as offline/degraded instead of showing stale state.

### Windows & WSL Stability

- **Startup freeze fix** — resolved white-screen startup failures caused by blocking daemon status checks on the pre-window path.
- **WSL UNC path handling** — search index rebuilds, project creation, and git trust operations now handle WSL UNC paths correctly instead of freezing or failing silently.
- **Daemon bootstrap race** — fixed a race between startup and the health check that could leave the daemon in an inconsistent state.
- **Silent install improvements** — the installer now kills running instances before install and verifies the installed binary hash matches the built payload.
- **Task tracking fix** — Claude task scanning on Windows was watching the wrong directory. All path resolution now uses the platform-aware authority.
- **Session bridge recovery** — stale session presence is preserved across daemon connection gaps so the sidebar doesn't flicker on reconnect.

### Mesh Runtime Hardening

- **Mesh 0.2.17** — bundled mesh binary updated through 6 versions (0.2.12→0.2.17) with fixes for daemon recovery, auth failures, and member lifecycle.
- **3-slot daemon connection pool** — replaced single connection with a pool, with dedicated connections for session bridge seeding.
- **Add-agent pipeline** — fixed error masking in the add-agent flow and routed onboarding notices through proper delivery channels.
- **Team daemon auto-rotation** — daemon processes automatically rotate after mesh binary updates.
- **Compaction reinjection** — post-compaction recovery now includes full role instructions and runtime compact summaries for better context restoration.
- **V3 role definitions** — updated Claude and Codex role definitions based on production experience, with context-steering metadata instead of capability labels.
- **Fresh sessions on resume** — removed checkpoint-based continue mode in favor of always starting fresh sessions, which is more reliable across tools.

### Accessibility

- **WCAG Level A compliance** — all dialogs and overlays now have proper `role="dialog"`, focus traps, Escape-to-close, and focus restoration. Background content is marked `inert` when modals are open.
- **Keyboard navigation** — tab bar supports Arrow Left/Right/Home/End, context menus open via Shift+F10, type-ahead search in menus, visible focus-visible rings on all interactive elements.
- **Screen reader support** — icon-only buttons have `aria-label`, daemon status changes use `aria-live` regions, tab bar uses `role="tablist"` with `aria-selected`, settings textareas have accessible labels.

### Error Handling

- **Visible error feedback** — session launch, stop, and navigation failures now show sidebar notices instead of silently logging to the console.
- **Human-readable error messages** — technical backend errors are translated to plain language via a centralized error copy module. Daemon install failures, scan errors, and mesh init issues all show actionable guidance.
- **Daemon reconnect escalation** — a calm "connecting" banner appears initially; after 30 seconds without reconnection, the message escalates and offers a "Restart helper" button. Distinguishes between busy, reconnecting, and failed states.
- **Batch registration feedback** — when some projects fail to register, the failure count and individual errors are now shown instead of only the success count.
- **"Helper service" language** — all user-facing error messages and setup flows now use "helper service" instead of internal "daemon" terminology.

### Performance

- **Inotify watcher consolidation** — the daemon now uses ~4 inotify instances instead of ~25, through a global shared watch registry with reference-counted subscriptions. App-side watchers use shared pools instead of one watcher per project.
- **Startup optimization** — install work moved off the pre-window path, deferred retryable load warnings until daemon recovery settles, and split project selection into critical and deferred phases.
- **Task update efficiency** — removed blocking task recovery from request paths and eliminated redundant task reloads after background refresh.
- **Task board self-heal** — recovers from empty DB by scanning Claude task files directly.
- **Member daemon lifecycle** — re-adding a team member no longer inherits stale daemon PIDs from a previous session.

### Code Quality

- **5 major file decompositions** — Shell.svelte (1503→589 LOC + 4 controllers), stall_detector (2978→focused modules), orchestrator (1895→7 modules, max 545), session_scanner (1945→6 modules, max 595), and meshTabController (1565→260 orchestrator + 4 state modules). All under the 600 LOC target.
- **Shared contracts** — tmux layout allocation, scan/index policy, path normalization, and terminal platform contract extracted from duplicated inline logic.
- **Structured logging** — coordination lifecycle, project mutations, startup events, and daemon caller context now emit machine-readable JSONL events with correlation IDs.
- **Inotify telemetry** — daemon emits structured diagnostics for instance/watch counts and capacity warnings.

### Documentation

- **Full docs refresh** — all user-facing docs reviewed for accuracy. 24 specific clarity rewrites applied across 8 documents to remove jargon and lead with user behavior.
- **Keyboard shortcuts reference** — added to getting-started.md with all supported shortcuts.
- **"How taurhaus works"** — new conceptual overview paragraph in getting-started for users who want the mental model.
- **inotify resource note** — Linux/WSL users can find guidance on watcher limits and how to raise them.
- **Architecture doc reconciliation** — ARCHITECTURE.md, data-architecture, and coordination-architecture now have clear ownership boundaries with no overlapping explanations.
- **Settings docs promoted** — now in the Quick Links navigation for easier discovery.

### Testing

- **4 new E2E specs** — first-run wizard, command center real actions, session management runtime truth, and mesh recovery now have end-to-end coverage with real tmux integration.
- **2,000+ new E2E test lines** — proving that launch modes execute correct commands, stop actually kills sessions, activity detection reflects real I/O, and mesh lifecycle works end-to-end.
- **Security and code quality re-audits** — both passed clean after all quality phase changes. Code quality re-audit confirmed substantial structural improvement.
- **1,203 frontend unit tests** — all passing. Full `just check` gate green.

### Known Issues

- **Mesh degraded-path E2E** — some mesh recovery edge cases (degraded member resume after pane loss) have skipped E2E checks due to team-daemon startup verification timeouts after resume.
- **Mesh team-daemon resume** — after resuming a team, the team-daemon startup verification can time out, leaving agents in idle state despite coordination reporting success. Tracked for follow-up.

## [0.5.9] - 2026-03-10

Performance, compaction reliability, and mesh team setup release. This version focuses on removing unnecessary work from long-running background paths, tightening compaction reinjection across Claude and Codex, and replacing the old mesh setup flow with a faster drag-and-drop builder.

### Added

- **Drag-and-drop mesh team builder** — Mesh setup now uses a persistent role catalog and roster builder instead of the older linear setup flow. Roles can be composed inline, reordered visually, and reused for runtime add-agent.
- **Compact role catalog filters** — the mesh builder now includes tool and role-kind filters so large role catalogs stay navigable in a narrow side-panel layout.
- **Four new built-in role templates** — added design lead, product checker, product lead, and vertical slice developer templates.
- **Windows silent install recipe** — added a verified silent installer path for native Windows installs.

### Fixed

- **Claude compact hook payload parsing** — the SessionStart compact hook bridge now accepts the current snake_case Claude payload format while remaining backward-compatible with legacy camelCase fields.
- **Claude hook stdout isolation** — standalone compact-hook invocations no longer contaminate normal app stdout paths.
- **Runtime session liveness refresh** — stale session attachment metadata is refreshed more reliably during compaction-related recovery paths.
- **Compaction delivery diagnostics** — Claude/Codex compaction failures and skips now record clearer reasons, making reinjection failures easier to debug in production.
- **Team initialization readback race** — backend team config writes now retry readback visibility before initialize continues, fixing intermittent `config.json not found` failures.
- **Stale initialize request reuse** — Mesh no longer reuses an old init request after disband/remount, which previously revived or replayed stale setup state.
- **Configured lead startup for mesh team-daemon** — the mesh team-daemon now starts the configured lead correctly instead of falling back to the wrong member.
- **Standalone Claude compaction event logging** — compact-hook activity is now logged correctly even when invoked outside the main app lifecycle.
- **Project activity detection** — unattributed shared-project activity now promotes project recency correctly without over-promoting everything to fully active.
- **Windows install verification** — NSIS installs are now verified against the patched payload hash rather than the raw build hash.
- **Watcher callback stability** — watcher ownership/callback lifecycle issues that caused instability in the full quality gate were corrected.
- **Built-in role validation drift** — backend template count and validation expectations were brought back in sync with the shipped role set.

### Performance

- **Inotify watch pre-pruning** — watcher registration now skips ignored directories up front, cutting watch count dramatically and reducing daemon overhead.
- **Startup optimization** — daemon reconnect and watcher startup paths rely on readiness polling/handshakes instead of fixed sleeps.
- **Render caching** — markdown/render-heavy views avoid unnecessary rerender work, improving perceived responsiveness during project inspection.
- **Project-switch dedupe** — redundant startup and project-switch selection work was removed so only the final intended load path runs.
- **Authoritative runtime snapshot** — live consumers now share a single daemon-owned runtime session snapshot instead of rebuilding overlapping views from separate scans.

### Changed

- **Mesh builder flow** — the old setup flow code was removed in favor of the persistent builder and role-catalog-driven composition path.
- **Mesh version pin** — bumped from 0.2.7 to 0.2.10.
- **Windows build pipeline** — build recipes now auto-detect `sccache`, emit better instrumentation, and avoid duplicate daemon compilation during Windows builds.

### macOS

- **Ghostty launch on macOS** — terminal launch now goes through LaunchServices so Ghostty opens reliably on the Mac build.

## [0.5.8] - 2026-03-09

Cross-project mesh delivery fix and UI cleanup.

### Fixed

- **Mesh cross-project agent delivery** — outbound messages (`mesh send`, `mesh task assign`) to agents registered in a different project now work correctly. Previously failed with "agent not found" even when the inbox file existed on disk. Mesh 0.2.7.

### Changed

- **Compaction diagnostics removed from UI** — the debug-level compaction reinjection audit surface has been removed from the mesh runtime view. Compaction health data remains accessible through backend logs and `just analyze-compaction`.
- **Mesh version pin** — bumped from 0.2.6 to 0.2.7

## [0.5.7] - 2026-03-09

Event-driven compaction pipeline, daemon CPU optimization, multi-CLI lead support, and role import/export. The compaction detection chain is now fully notify-based — no more polling in the middle of an event-driven architecture. Daemon steady-state CPU dropped from ~49% to ~31% of one core.

### Added

- **Event-driven compaction detection** — compaction signal extraction now uses inotify/notify on Codex JSONL files instead of 500ms polling, with offset persistence and paired-record normalization
- **CompactionSignalWatcher** — file-system watcher on the signal log with reconciliation fallback, replacing the old poll-based consumption loop
- **CompactionSignalProcessor** — extracted downstream delivery logic into a clean single-responsibility processor
- **Config-dir topology watching** — team watcher reconciliation driven by inotify on `~/.claude/teams/` instead of periodic directory scanning
- **Shared runtime-session cache** — single scanner path feeds both display and compaction consumers, eliminating duplicate scanning
- **Stale daemon binary detection** — app startup validates running daemon via `/proc/<pid>/exe` against installed binary, auto-restarts on mismatch
- **Claude compact hook observability** — pipeline health reports and structured audit events for compaction lifecycle
- **PlatformPaths authority** — centralized cross-platform path resolution for Windows, WSL UNC, and Linux path forms
- **Compaction analysis tool** — `just analyze-compaction` recipe for live pipeline debugging
- **Role import/export** — adapter schema for Claude Code and Copilot custom agent formats with round-trip provenance tracking
- **Multi-CLI lead roles** — non-Claude agents (Codex, Gemini) can now serve as team lead with tool-appropriate presets and lifecycle
- **Unified team roster query** — single join point for member runtime state across all coordination consumers
- **Imperative resume card** — post-compaction reinjection card explicitly instructs agents to continue working rather than summarizing metadata
- **Compaction reinjection audit surface** — mesh runtime view shows compaction detection and delivery events

### Fixed

- **Inbox corruption handling** — corrupt inbox files are now quarantined instead of silently treated as empty, preventing delivered messages from being hidden
- **Paired Codex compaction boundaries** — extractor collapses `compacted` + `context_compacted` records within 2s into a single delivery
- **Liveness reconcile session_id overwrite** — reconciliation no longer clobbers existing session_id when backfilling missing jsonl_path
- **Daemon offline indicator** — recovers correctly when daemon comes back online
- **Cross-platform path normalization** — Codex normalizer and config aliases handle Windows ↔ WSL ↔ Linux path translation

### Performance

- **Daemon CPU ~31% steady-state** (down from ~61% pre-optimization, ~49% after first pass) — removed redundant 500ms compaction scan loop and switched to diff-based downstream fanout
- **Diff-based daemon fanout** — session activity exports and extractor updates only pushed when data actually changes, not every 500ms tick

### Changed

- **Mesh version pin** — bumped from 0.2.5 to 0.2.6 (selective mark-read: only marks displayed messages, not entire inbox)
- **Session type split** — `DisplaySession` and `RuntimeSession` are now separate types with distinct responsibilities
- **Legacy compaction module removed** — deleted `session_scanner/compaction.rs` (superseded by event-driven pipeline)
- **Dead defensive branches removed** — `EmptyAdditionalContext` skip, pane foreground guard, JSONL boundary guard all removed as irrelevant for inbox-file delivery

### Reliability

- **Flaky integration tests hardened** — TCP server tests (daemon_client, event_listener, session_listener) now use port-readiness waits instead of fixed sleeps

## [0.5.6] - 2026-03-08

Tmux foreground detection, non-blocking team initialization, and backend-owned role hydration. Mesh 0.2.5 with serde flatten to preserve extension fields.

### Added

- **Sidebar foreground indicator** — two horizontal brand-400 lines (top + bottom) highlight the project whose tmux window is currently focused, with 150ms fade-in animation
- **Tmux focus detection backend** — after-select-window hooks write a focus file; backend watches it and emits `foreground-project-changed` events to the frontend
- **Optimistic foreground clicks** — clicking a project immediately sets the foreground indicator while the backend event catches up

### Fixed

- **Tmux hooks on Windows** — hook commands now route through `wsl.exe` so they can write the focus file from the WSL tmux server to the correct Windows app data directory
- **Stale tmux hooks** — hooks are force-reinstalled on every app startup, clearing leftovers from previous versions
- **Focus file path drift** — canonicalized to `app_data_dir()` only, removing the `dirs::data_local_dir` fallback that caused path mismatch on Windows
- **App freeze during team init** — `coordination_initialize_team` converted to async with `spawn_blocking`, keeping the UI responsive
- **Preset role metadata missing on hover** — backend now hydrates role metadata (focus_area, context_summary, behavior_summary, instructions, behavioral_contract, capabilities) from template storage when the frontend sends minimal payloads
- **Mesh stripping extension fields** — mesh 0.2.5 uses serde flatten on TeamConfig/Member types, preserving taurhaus-specific role metadata through heartbeat config rewrites

### Changed

- **Backend-owned preset resolution** — frontend sends minimal preset init payload (preset ID + agent names + project bindings); backend resolves full role definitions from template storage via the composition engine
- **Mesh version pin** — bumped from 0.2.4 to 0.2.5

## [0.5.5] - 2026-03-07

Security, code quality, and performance hardening release. Full security and quality audits drove targeted fixes across both taurhaus and mesh. Mesh tab navigation is now fully non-blocking on all platforms.

### Security

- **Mesh PID file validation** — `timer-cancel` now verifies process identity before kill, preventing forged PID files from terminating unrelated processes (mesh 0.2.4)
- **Daemon singleton locking** — exclusive lock files (`create_new` + lifetime-held) replace the racy check-then-create PID file pattern, preventing duplicate daemon instances (mesh 0.2.4)
- **Session activity stats preserved** — `stopPolling()` now flushes tracker data before clearing, preventing data loss during daemon bridge handoff

### Refactored

- **Shell.svelte decomposition** — extracted navigation helpers and event wiring into `src/lib/shell/navigation.svelte.js` and `src/lib/shell/events.svelte.js` (1302 → ~1200 LOC)
- **command_center.rs split** — domain-based submodules (`session_listing`, `launching`, `navigation`, `activity_tracking`) with thin `#[tauri::command]` wrappers
- **Doc/metadata drift fixed** — corrected IPC command count, removed stale db placeholder recipes, cleaned up duplicate lockfile

### Performance

- **Non-blocking mesh live status** — `coordination_get_live_team_status` converted from synchronous to async Tauri command with `spawn_blocking`, eliminating tab-switch blocking entirely
- **Mesh runtime refresh coalescing** — deferred refresh and periodic polling share an in-flight gate, preventing duplicate ~2.5s backend calls from stacking
- **Stale refresh cleanup** — in-flight promises are severed on tab deactivation, preventing request accumulation across rapid tab cycling
- **Project switch debounce** — 25ms batch window coalesces rapid project switches so only the final IPC fan-out fires

### Changed

- Mesh binary bumped to 0.2.4 (PID file security hardening, daemon singleton locking)
- `just check` output now tees to `.check-logs/` with 5-file auto-rotation

## [0.5.4] - 2026-03-07

Daemon reliability and team lifecycle release. Automatic hot-swap eliminates manual runbooks for mesh upgrades, background self-heal no longer freezes the UI, and cold restart recovery lets you pick up running teams after an app restart.

### Added

**Team Resume & Cold Restart Recovery**
- Resume Team banner appears when the app detects a previously running team — one click to reconnect to existing agent panes
- Snapshot classification (active panes / stale daemons / cold start) drives the recovery flow
- IPC commands for team resume with progress reporting
- Lifecycle header replaces the old runtime warning banner with richer state context

**Daemon Hot-Swap**
- Automatic version drift detection — background self-heal compares running daemon binary against bundled version
- Atomic binary install: temp-stage + `mv -f` prevents "text file busy" and partial-copy corruption
- Full daemon cycling after upgrade: team-daemon self-restart + member daemon restart
- Works on both Linux and macOS (no `/proc` dependency — uses `ps`/`kill` universally)

**Mesh Canvas Polish**
- Cross-project member distinction — agents working on other projects get a visual treatment
- Runtime role hover card with context-steering metadata
- Sidebar session grouping with team connector rail and stacked tool logos

**Role System Overhaul**
- Context-steering model replaces capability-centric role definitions
- Role summary fields propagated through the full coordination pipeline
- Frontend role editor and catalog updated to show context-steering metadata

### Fixed

- **30-second UI freeze** — background self-heal held the shared IPC mutex, causing brief grey-out. Now uses an isolated orchestrator instance.
- **Windows process spawning storm** — Mesh view triggered rapid `mesh` process launches on every poll. Added in-flight guard + console window suppression.
- **Windows switch-away stall** (~5.1s → ~1.3s) — eliminated blocking runtime probes when switching away from Mesh tab.
- **Liveness reconciliation** — stale `SessionDead` records repaired during reconcile pass; dead member daemons restarted for active panes.
- **Daemon pidfile race** — resume daemon start now verifies pidfile before persisting `daemon_pid`, preventing ghost PID entries.
- **Mesh discovery on Windows** — path normalization for snapshot discovery, skipped liveness probes on Windows snapshot path.
- **Agent detail popup** — opens immediately instead of waiting for data, auto-closes after actions.
- **Mesh view remount** — eliminated unnecessary component remount on project switch.
- **Stale team folder warnings** — silenced noisy discovery warnings for team folders without configs.
- **serde_yml deprecation** — replaced unmaintained `serde_yml` with `serde_norway`.

### Changed

- Mesh binary bumped to 0.2.3 (canonical HOME resolution for sandboxed agents, cross-platform daemon process identity, `ps`-based process checks on macOS)
- Unified project mesh snapshot path across all platforms — single code path replaces platform-gated runtime probes
- Sidebar team indicators refined: standalone icon style, CSS grouping, connector rail

### Performance

- Instant Mesh tab render via cache-first snapshot with background refresh
- Instant project switch away from Mesh view (no blocking teardown)
- Agent detail popup latency halved (212ms → 102ms)

## [0.5.3] - 2026-03-06

Bug fix release targeting team creation and agent communication reliability.

### Fixed

- **Codex model flag rejected** — `gpt-5.4-high` (hyphenated) was rejected by ChatGPT accounts. Changed to `gpt-5.4 high` (space-separated) with backward-compat normalization for legacy values.
- **Claude hot-add "no inbox"** — Adding a Claude Code agent to a running team failed with "agent not found (no inbox)" because the add-agent pipeline launched the agent before registering it in team config. Fixed by pre-registering the member before pane creation.

### Changed

- **Shell depth treatment** — Subtle sealed-panel effect on main content area (faint top highlight, inner border, deeper shadow) and material gradient on dark teal frame. Both dark and light modes. No blur or translucency.

## [0.5.2] - 2026-03-06

Mesh canvas reliability release. Structurally resolves the recurring connection routing bug class by extracting a pure layout engine, adds a visual testing lane, and redesigns the project HoverCard.

### Added

**Mesh Layout Engine**
- Pure `meshLayout.js` module that computes node placement and connection routing in one coordinated pass
- Replaces scalar `bend` with explicit cubic control points (`start`, `end`, `control1`, `control2`)
- 34 layout invariant tests covering 1–8 agents, row collapse, non-crossing ordering, center-agent degeneracy, and viewBox bounds
- Architecture concept doc: `docs/architecture/mesh-canvas-layout-engine-concept.md`

**Visual Testing Lane**
- Vitest Browser Mode with Playwright provider — 34 screenshot tests across 5 component specs (MeshCanvas, HoverCard, MeshNodeDetail, Sidebar, smoke) in 7.6s
- Fixture modules with named scenarios and shared builders for each component
- Standalone Vite fixture host (`visual-host.html`) for manual component browsing with mock data
- `just test-visual` recipe and `bun run dev:visual` script
- Testing guide: `docs/testing-guide.md`

**HoverCard Redesign**
- Verdict-first layout: header → attention verdict → evidence stack → optional relationship
- Prefers session summary over commit list, surfaces unresolved handoff items
- Conversational copy replacing technical/formal phrasing
- Dark/light theming via `$derived` tokens, 100ms/70ms enter/exit hover timing

### Fixed

- **Mesh connection routing** — 4 rounds of connection bugs (#395, #400, #401, #412) structurally resolved by the layout engine extraction. No more ad-hoc bend patching.
- **Center agent invisible line** — straight line fallback when bezier would collapse to near-zero horizontal bend
- **Lead anchor fan-out** — connections now use distinct anchor points spread across the lead card instead of originating from a single center point
- **Connection curve overlap** — outer agents route outward, center agents stay straight
- **Focus button wiring** — mesh Focus button now navigates to the correct tmux pane
- **Onboarding test assertion** — aligned e2e test model expectation with gpt-5.3 fixture

### Changed

- `MeshConnection.svelte` is now a dumb renderer — receives pre-computed control points, no longer computes bezier curves internally
- `MeshCanvas.svelte` delegates all layout to `meshLayout.js` — inline row-packing, anchor fan-out, and bend logic removed
- Default Codex model updated to `gpt-5.4-high`
- Mesh binary bumped to 0.2.1 (rejoin reactivation fix for `mesh send` after daemon restart)

### Documentation

- Synced `ARCHITECTURE.md`, `CLAUDE.md`, and `AGENTS.md` with current implementation details (80 registered IPC commands, updated module/build references)
- Refreshed `docs/architecture/ipc-reference.md` to match the active command surface
- Updated `docs/coordination-architecture.md` to point to the practical orchestration direction and explicitly mark the v0.2 protocol design as archived
- Layout engine pipeline retro and visual testing pipeline lessons in `docs/retros/`
- HoverCard vision and UI concept design documents
- Mesh canvas library assessment (dagre, ELK — neither adopted; custom engine chosen)

## [0.5.1] - 2026-03-06

The largest release since the project started. 81 commits spanning a complete observability overhaul, architecture refactoring on both sides of the stack, a new coordination subsystem, Windows stability fixes, toolchain migration, and the first bundled mesh CLI with team-daemon support.

### Added

**Structured Logging Pipeline**
- JSONL structured log sink with per-event context, replacing unstructured stderr logging
- Complete IPC command lifecycle instrumentation — all 80 registered commands emit start/finish/error spans
- Startup and daemon bootstrap events with phase-level timing
- Watcher and event processor structured instrumentation with batch metrics
- Frontend log bridge rewritten with interaction IDs and structured payloads (`console.*` → IPC → JSONL file)

**Stall Detection & Coordination**
- New `StallDetectorService` — detects agents that stop making progress on assigned tasks
- Signal fusion scaffolding: combines session scanner signals, pane status checks, and mesh task state
- Escalation delivery with suppression rules and rate limiting to prevent alert fatigue
- Per-member activity snapshot export for mesh IdleMonitor integration

**Mesh 0.2.0 Integration**
- Bundled mesh 0.2.0: IdleMonitor (30s poll cycle), `mesh task assign`, `mesh nudge`, actionable message lint, centralized team-daemon
- Mesh version lock manifest (`mesh.version` + `mesh.lock.json`) tracked in git with build-time verification
- New build recipes: `mesh-verify-lock`, `update-mesh-lock`, `bundle-mesh`

**E2E Test Infrastructure**
- Failure artifact bundles collected automatically in `afterTest` hook (screenshots, logs, DOM state)
- Template CRUD UI E2E coverage with slide-over interaction helpers
- Annotated regression tests for sessionStore and bridge-missing fallback cases

**Developer Tooling**
- `just check-quick` fast feedback recipe: `cargo fmt` + `cargo check --tests` + frontend typecheck + frontend unit tests
- Practical orchestration design doc (auto-idle detection + communication quality patterns)

### Changed

**Architecture Refactoring — Backend**
- Split `coordination/pipelines.rs` (2541 LOC) into domain-specific stage modules (`initialize`, `members`, `lifecycle`, `helpers`)
- Split `templates/storage.rs` into focused modules (`roles`, `presets`, `git`, `state`)
- Extracted `sentinels.rs` — shared watch-target planner module for watcher reconciliation
- Startup refactored into phased bootstrap pipeline (`bootstrap`, `daemon`, `search`, `watchers`)
- IPC error envelope standardized with `SanitizeErr` trait for user-safe error surfaces
- Project identity normalization centralized across command handlers
- Coordination command overloads collapsed to canonical internal implementations
- Template mutations moved behind shared `mutate_and_commit` scaffold with store API

**Architecture Refactoring — Frontend**
- `MeshTab` decomposed: extracted `MeshRuntimeView`, `meshTabController.svelte.js`
- IPC layer split from monolithic `ipc.js` into domain modules (`client`, `projects`, `sessions`, `tasks`, `templates`, `coordination`, `system`)
- Context providers extracted to `src/lib/context/` (`ProjectContext.js`, `SessionContext.js`)
- Shell theme and mesh gate/notification modules extracted into focused files
- IPC payload normalizers consolidated into shared module

**Toolchain & Infrastructure**
- Migrated from npm to bun for all JS tooling (`bun install`, `bun run`, `bunx`)
- Replaced `notify-debouncer-full` with direct `notify`, migrated `serde_yaml` → `serde_yml`
- Replaced bash resource monitor with Python implementation

**Coordination Protocol**
- Removed abandoned v0.2.0 orchestration protocol assumptions from `CLAUDE.md` and `AGENTS.md`
- Documented practical orchestration direction grounded in available signals (file-based mesh + real-time taurhaus)

### Fixed

**Windows Stability**
- App crash on project selection with large workspaces — watcher reconciliation moved off IPC thread
- Daemon connection stall — removed blocking reconnect, added IPC timeout with regression tests
- IPC camelCase/snake_case normalization — root cause of mesh setup wizard hang and E2E failures
- P1 regressions: retry thread cap, stall detector timeout, removed panicking `expect` calls

**Frontend**
- Atomic project view reveal restored (parallel loading with `Promise.all`, no waterfall)
- Unified content-enter transitions across all tab views
- Search overlay layout fixed — CSS conflicts were breaking fixed positioning
- Session scanner camelCase payload normalization + polling fallback for missing bridge events
- User-facing error messages normalized, settings save feedback added
- Accessibility improvements: theme tokens, focus management, error surfacing

**Backend**
- Release builds now log at INFO level (was ERROR-only without `RUST_LOG`)
- Silent error swallowing eliminated in event processor, daemon lifecycle, logging pipeline
- Daemon watch gitignore filtering, template concurrency, watcher classification fixes
- Logger recursion guard + daemon error variant reclassification
- Post-compaction idle prevention in onboarding templates

**E2E**
- Config group stall eliminated — settings fast-paths, resilient clicks, driver cleanup
- Fresh selector in `ensureMainApp` to avoid stale element handles

### Security

- Bumped DOMPurify to 3.3.2+ to fix XSS vulnerability (GHSA-v2wj-7wpq-c8vv)

## [0.5.0] - 2026-03-05

### Added

**Mesh View Redesign (M1-M3)**
- M1 foundation: node-canvas primitives, `SlideOver`, and mesh design-token groundwork
- M2 integration: `MeshTab` orchestration flow, slide-over panel integration, and card-style agent presentation
- M3 runtime: initialization/runtime animations, `MeshRuntimeBar`, and runtime-mode visual continuity
- Designer-approved finish pass: shadows, glow, gradients, and a full-bleed canvas/surface overhaul
- Expanded light-mode variants for mesh connections/surfaces to preserve contrast in non-dark themes

**Team Composition & Presets**
- New built-in role template: `codex-architect` for structural decision ownership
- New built-in team preset: `standard-team` (orchestrator + architect + two developers + UI specialist)
- Preset setup now resolves member names from slot `name_pattern` overrides and role `default_name_pattern` fallbacks, producing role-appropriate names (for example `architect`, `developer1`, `developer2`, `ui-specialist`) instead of generic `agent-N`

**Project Bootstrap**
- Create New Project flow in `AddProjectModal` for creating and registering projects directly from the app

### Changed

**Performance Sprint (Backend + Frontend)**
- Daemon IPC latency reduced from **44ms to 0.114ms**
- Git timeline/range queries moved to single-pass scans with TTL memoization
- Session-scanner cycle cost reduced with batched search-commit queries
- Frontend rendering optimized with virtualization for heavy lists, bounded caches, and lazy-loaded markdown/Shiki paths
- Template IPC calls deduplicated with stricter stale async result guards

**Backend Error Handling & Core Hygiene**
- Error handling overhauled around `SanitizeErr` for user-safe error surfaces
- Mutex poison recovery and silent-drop logging added to improve degraded-path resilience
- IPC casing normalization and targeted deduplication landed across shared paths

**Frontend Reliability & Template UX**
- Async guard hardening applied across file/markdown/search surfaces (including `CodeViewer`, `MarkdownRenderer`, and `SearchOverlay`) to prevent stale UI states
- Template CRUD surfaces refined with role-aware agent forms, improved IPC wrappers, and Gemini session detection correctness
- Built-in role behavioral contracts updated for clearer specialization:
  - Orchestrator: stronger delegation-first execution contract
  - Codex developer: explicit architect escalation for structural decisions
  - Gemini UI specialist: frontend-only scope boundary

**Quality & Test Infrastructure**
- Added `just metrics` KPI reporting lane
- Clarified `just test` vs `just check` semantics and added faster test workflow
- Frontend branch coverage improved from **54% to 65%**

### Fixed

- Files tab loading regressions after project switches and metadata-only updates (stuck/blank first-load cases)
- Window-state restore behavior for undecorated windows (height/restore correctness on reopen)
- Mesh visual regressions introduced during redesign (overlay behavior, runtime-surface continuity, and light-mode connection contrast)
- Platform review hardening findings, including macOS `/proc` guard handling
- Onboarding E2E flakiness via real temp project directories and deterministic harness improvements

### Documentation

- Added design-first workflow guide: `docs/design-workflow.md`
- Refreshed release docs for v0.5.0 scope across architecture/contributing/coordination surfaces (`ARCHITECTURE.md`, `CONTRIBUTING.md`, and coordination documentation updates)

## [0.4.5] - 2026-03-04

### Added

**Team Template System**
- Git-backed template command surface: role/preset CRUD, composition/validation, storage status, history, diff, revert, import, and pending flush endpoints (`templates_*`)
- Template catalog and composition UI: role/preset browsing, quick compose preview, editable roster composition, and mesh-setup integration
- Template history UX: global/selected-template commit history, commit detail metadata, diff hunk view, dirty-state indicator, and revert action
- Template E2E workflow coverage (`e2e/specs/templates.js`) for catalog flow, composer validation, and role/preset CRUD paths

**Role Context Delivery**
- Template-launched agents now receive role-specific instructions, behavioral contract, and capabilities in their onboarding
- Dual delivery path: Codex/Gemini agents receive role context in tmux onboarding message; Claude agents receive it as first team message after session detection
- Role metadata persisted in member config for restart resilience

**E2E Isolation**
- Session-level E2E sandboxing in WebdriverIO: per-session temp roots for app data + Claude data, plus an isolated fixture git project for deterministic onboarding

### Changed

- Mesh setup now supports template-first onboarding paths (preset quick-select, catalog browse, custom composition) while preserving manual blank-slate fallback
- Frontend IPC layer migrated template calls from temporary mock command names to backend `templates_*` commands
- Runtime path resolution now supports `TAURHAUS_DATA_DIR` (app data) and `TAURHAUS_CLAUDE_DIR` (Claude tasks/teams roots) overrides for isolated runs
- E2E recipes (`just test-e2e*`) are now safe-by-default and do not auto-run `install-daemon`; opt in with `E2E_INSTALL_DAEMON=1`
- Template backend writes now use an atomic mutation pipeline (`mutate_and_commit`), shared agent-slot validation, and direct ID-path lookups for role/preset reads
- Compose IPC request handling now accepts camelCase/snake_case DTO aliases for agent slot fields
- Template UI polish: accessibility labels on agent controls, duplicate-name submit enforcement, sequence guards for async preset/diff races, save-as-preset slug validation, 12px label sizing, and shared derived surface tokens

### Fixed

- Template import failures now report detailed parse/validation context for role and preset attempts instead of a generic invalid-file error
- Template catalog CLI tool filter now correctly reads nested `defaults.cli_tool` field; previously all templates appeared as Claude regardless of actual tool

### Documentation

- Added `docs/team-templates.md` user guide for role templates, team presets, composition, history, and revert workflows
- Updated architecture docs (`ARCHITECTURE.md`, `docs/coordination-architecture.md`) for template storage, composition, and coordination integration points

## [0.4.4] - 2026-03-04

### Added

**Agent Resume Lifecycle**
- Resume offline members: `coordination_resume_member` pipeline with `Continue` and `Fresh` modes
- Resume contracts: `ResumeContextMode`, `ResumeMemberRequest`, and `ResumeAgentReport` IPC types
- MeshTeamRoster resume UX: Resume action on offline rows with mode-aware relaunch

**Liveness Reconciliation**
- Write-on-drift liveness reconciliation in live status queries (`reconcile_team_liveness`)
- Shell-return drift detection: `pane_is_shell` checks `#{pane_current_command}` for shell fallthrough
- Offline drift daemon cleanup: non-Claude `daemon_pid` check/terminate/clear behavior

**Documentation & Infographics**
- Regenerated 7 infographics for accuracy (mesh-view-lifecycle, coordination-architecture, system-architecture, data-model, task-aggregation, file-rendering-pipeline, build-release-pipeline)
- New mesh-resume-liveness-sequence infographic: end-to-end sequence diagram for resume and write-on-drift flows
- Updated ARCHITECTURE.md, CONTRIBUTING.md, mesh-view-design.md, coordination-architecture.md for resume and liveness features
- Added feature-matrix.md and phase-4-architecture.md documentation
- Security audit report for v0.4.3 release

## [0.4.3] - 2026-03-04

### Added

**Task Identity & Session Attribution**
- Task identity model: `source_key` column (migration 009) disambiguates tasks from different Claude source directories (session-id vs team-name)
- Codex/Gemini session identity: persist session ID from JSONL metadata with filename-stem fallback
- Transcript-derived commit time windows: use JSONL session timestamps instead of DB persistence timestamps for accurate commit association
- Structured enrichment warnings: surface commit-enrichment failures in API response instead of silently returning zero counts

**Scan Robustness & Performance**
- Tri-state scan outcomes: `Data` / `DefinitivelyEmpty` / `Unavailable` prevent false task pruning on degraded I/O
- Targeted project invalidation: task file changes rescan only affected project, not all registered projects
- Per-cycle index caching: `ClaudeSourceIndex` and session list built once per scan cycle and reused across projects
- Diff-based event emission: `project-tasks-changed` only emits on meaningful task count/status changes

**Mesh Agent Management**
- Agent removal from existing teams: Remove action on non-lead agents in mesh roster with confirmation dialog
- `RemoveAgentReport` with per-step outcomes (daemon terminate, mesh leave, pane kill, config/runtime cleanup)
- Lead-removal guard: backend hard-blocks removing the team lead
- Pane ownership pre-check: verify tmux pane belongs to expected session before killing
- Team-lead removal notification: lead-only mesh notification when an agent is removed (who, by whom, cleanup status)

**UI Task Board Polish**
- Archive metadata display: `archived_reason`, `state_changed_at`, `last_status` surfaced in SessionHistory and TaskDetailPanel
- Live session history refresh: subscribe to `project-tasks-changed` while history tab is active
- Deterministic task column sorting: in_progress by recency, pending by dependency count, completed by update time
- `active_form` secondary text on in-progress task cards
- Enrichment warning badge on sessions with suspect commit counts

### Fixed

- Always run task reconciliation on startup even for empty scans
- Tri-state enforcement on degraded I/O: read_dir/parse failures map to `Unavailable` instead of `DefinitivelyEmpty`
- Async event listener cleanup race in TaskBoard and SessionHistory (unmount before listen resolves)
- Sort tiebreaker: stable secondary key prevents ordering jitter when primary sort keys tie
- Archived task detail: targeted DB query replaces O(n) linear scan
- Generation map bounded with retention-window eviction (prevents unbounded memory growth)
- Inline dark/light ternaries in SessionHistory extracted to `$derived` tokens
- Add-agent project path: pass explicit cwd to `join_mesh` instead of falling back to app data directory
- Roster update idempotent: if member already exists from join step, update entry instead of failing on duplicate
- Skip transcript lookup for team-scoped Claude sessions: team names have no JSONL transcript, use task timestamps directly without warning
- Rust implementation gate documented and enforced via `just agent-quality` (`cargo fmt` + `clippy -D warnings` + `cargo check --tests`)

## [0.4.2] - 2026-03-04

### Added

**Unified Task Management**
- Unified task scanner: scan all `~/.claude/tasks/` subdirectories with index-based classification (session-ID and team-name dirs)
- Claude source index: maps session IDs and team names to project paths via live sessions, JSONL fallback, and team configs
- Snapshot-based task archiving: reconcile DB against disk on every scan cycle, including empty scans
- Archive metadata: `state_changed_at`, `last_status`, `archived_reason` fields with migration 008

### Fixed

- Handle empty git repos gracefully in recent commits (return empty list instead of error for unborn HEAD)

## [0.4.1] - 2026-03-04

### Fixed

**Markdown Link Navigation**
- Fix broken relative link clicks in rendered markdown (undefined `resolveImagePath` function)
- Tab-aware path resolution: Overview tab resolves links against README, Files tab resolves against selected file
- Cross-file anchor navigation: clicking `docs/foo.md#section` now opens the file and scrolls to the heading
- Directory links (`docs/`): expand in file tree and open README.md if present
- Platform route links (`../../releases`, `../../issues`): detect above-root paths, resolve via git remote URL, open in system browser
- Add `check_path_type` IPC command for file vs directory classification
- Add `get_remote_url` IPC command with SSH-to-HTTPS remote conversion
- Fix daemon test assertion for empty session store version

## [0.4.0] - 2026-03-03

### Added

**Mesh View — Multi-Agent Team Coordination**
- Complete Mesh tab: setup form, initialization progress tracker, live team roster, and team cleanup panel
- Coordination backend: orchestrator with lifecycle management, delivery routing, and audit events
- Coordination stores with advisory file locking and domain types
- Coordination IPC commands for team CRUD, agent management, and live status
- Mesh CLI bundling: `install-mesh` recipe builds and bundles the mesh binary into app resources
- MeshAvailabilityGate: prerequisite checker (mesh CLI, tmux) before team setup
- MeshSetupForm: agent roster builder with per-agent tool/model/project selection and custom chevron selects
- MeshInitProgress: 7-step initialization tracker with real-time IPC progress events
- MeshTeamRoster: live member status (active/idle/offline) with 5s auto-refresh and tool brand icons
- Team cleanup panel: discover and disband existing teams before starting new ones
- Team-conflict recovery: "Open Existing Team" and "Disband & Retry" actions when init hits a name collision
- ConfirmDialog component: themed `<dialog>` replacement for native `window.confirm()` — backdrop, Escape key, danger/default variants
- Per-agent CLI warnings surfaced in mesh preflight
- Coordination event pipeline with drift reconciliation
- Coordination runtime boundary refactoring and onboarding delivery stabilization

**Session Management**
- Daemon streaming: session updates via versioned long-poll API replacing Tauri polling
- Activity attribution model: distinguish tool-originated vs unattributed project activity
- Session indicator hydration on Tauri startup
- Codex activity disambiguation per process via session file mtime
- Unattributed project activity detection in session indicators

**Markdown & Rendering**
- Mermaid diagram rendering in markdown pipeline with fallback on parse errors

**Documentation**
- Architecture reference docs and updated ARCHITECTURE.md
- Feature documentation, UI documentation, operations documentation
- Security documentation with risk register and audit history
- Documentation guidelines and index
- Teal-themed infographics for architecture, file rendering pipeline, session management, and workflow
- Data model ERD and image optimization script
- Session activity docs aligned with daemon event stream

**Infrastructure**
- Persist dark/light theme selection across app restarts
- Remember window position and size across restarts (tauri-plugin-window-state)
- Unified coordination pane creation on native tmux layouts
- E2E: install daemon before all e2e runs

### Changed
- Coordination modules decomposed: types, pipelines, validation extracted from monolithic files
- Backend module decomposition: lib.rs split into bootstrap + event_processor + daemon_lifecycle
- Server.rs decomposed into handlers + watch submodules
- Idle.rs decomposed into per-resolver submodules
- Commands/tasks.rs extracted from command_center.rs
- Launch command resolution shared across command center and coordination
- Default Codex model updated to gpt-5.3-codex in mesh flows
- Mermaid session-management diagrams replaced with infographics

### Fixed
- Windows daemon session path normalization before UI events
- Metadata-only session update churn in daemon avoided
- Markdown relative image/link path resolution in file viewer
- WSL UNC path handling in coordination config writes and team discovery
- Mesh WSL home parsing hardened against shell banner noise
- DirectoryBrowser init race and gitTab midnight test flakes
- Cargo fmt formatting drift normalized across codebase
- Overflow menu hover hardcoded to dark mode — now theme-aware with click-outside dismiss
- Add-agent select styling unified to custom-chevron pattern matching setup form
- Init disband button height mismatch and visual hierarchy (Retry demoted when conflict recovery visible)
- Inline dark/light ternaries in cleanup panel extracted to $derived tokens per CLAUDE.md convention
- Cleanup toggle label "Manage (0)" edge case when only warnings exist

### Security
- Daemon authentication: shared token validates every request
- Command override validation: allowlist + shell metacharacter rejection
- Scoped tmux environment variables to session
- Scoped opener capability to http/https URLs only
- Bounded read before allocation in daemon server
- Error path sanitization: home directory paths replaced with ~
- `#![forbid(unsafe_code)]` at crate root
- Supply chain policy: cargo-deny configuration
- DOMPurify: forbid `<style>` elements in markdown output
- Coordination: reject `.` and `..` team/member names
- Search: block symlink escapes in incremental indexing
- Search: de-index unreadable files on incremental updates
- Provider: cap README asset reads at 5MB
- Daemon fail-open auth fixed: abort on token failure

### Performance
- Frontend log bridge pressure reduced
- Hidden-tab background refresh churn eliminated
- Shell: surface degraded project loads with retry

### Removed
- Windows E2E test infrastructure (recipes, platform detection, cross-filesystem tests)
- Native `window.confirm()` dialogs — replaced with themed ConfirmDialog component

## [0.3.8] - 2026-02-28

### Fixed
- Search→file navigation: normalize backslash paths at read time so stale search indexes work without manual reindex

### Changed
- E2E search tests: dynamic cross-filesystem discovery — tests WSL and Windows FS projects with subdirectory files instead of root-level README

## [0.3.7] - 2026-02-28

### Fixed
- Search→file navigation broken on Windows for WSL projects — search index stored backslash paths (`src\main.rs`) that the Linux daemon couldn't resolve

## [0.3.6] - 2026-02-28

### Added
- Search button in titlebar — magnifying glass icon left of the theme toggle makes search discoverable without knowing Ctrl+K
- Comprehensive E2E test suite — 138+ functional workflow tests replacing render-only checks, verified on both Linux and Windows

### Fixed
- Windows E2E: projects registered with WSL UNC paths for correct daemon provider routing
- Tantivy search index lock crash when multiple app instances run concurrently — graceful fallback to in-memory index
- Windows E2E: file tree first cold load through UNC bridge handled with skeleton wait + retry
- Windows E2E: cross-tab Git navigation pre-warms commit list to avoid cold-load timeout

## [0.3.5] - 2026-02-28

### Fixed
- Startup white screen: heavy bootstrap work (daemon spawn, tmux, protocol check) moved to background thread — synchronous setup reduced from ~10s to ~100ms

### Changed
- Release workflow: `just bump` now also updates package.json and Cargo.lock; `just release` pushes to remote before creating GitHub release

## [0.3.4] - 2026-02-27

### Fixed
- Daemon startup on macOS: `SO_REUSEADDR` prevents TIME_WAIT port conflict on app restart
- Health check timing: faster recovery when daemon disconnected at startup (10s → 3s)
- Daemon auth on reconnect: re-read token when daemon restarts with new token
- Sidebar filter: replaced static div with functional input for project name filtering

## [0.3.3] - 2026-02-27

### Security
- Daemon authentication: shared token validates every request (F-01)
- Command override validation: allowlist + shell metachar rejection (F-02)
- Scoped tmux environment variables to session (F-03)
- Scoped opener capability to http/https URLs only (F-04)
- Bounded read before allocation in daemon server (F-05)
- Error path sanitization: home directory paths replaced with ~ (F-06)
- `#![forbid(unsafe_code)]` at crate root (F-07)
- Supply chain policy: `deny.toml` for cargo-deny (F-08)
- DOMPurify: forbid `<style>` elements in markdown output (F-09)

### Fixed
- Tab-switch performance regression: removed CSS animation from tab internals that caused GPU compositor thrashing with large Shiki-highlighted content
- Window controls: replaced Preview button with minimize, maximize, close

## [0.3.2] - 2026-02-26

### Fixed
- Taskbar icon: use transparent background with padding so logo silhouette is visible

## [0.3.1] - 2026-02-25

### Added
- Splash screen with state-driven boot animation (clip-path reveal)
- Taurhaus logo (Horned Keystone) replacing placeholder icons
- Windows app icons (ICO bundle, all PNG sizes)
- Comprehensive post-split Shell.svelte tests (56 tests)
- README.md with screenshots, architecture diagram, setup guide
- End-user getting started guide
- Navigation history (back/forward) store

### Changed
- Extract OverviewTab, FilesTab, Sidebar, DirectoryBrowser from Shell.svelte
- Extract shared theme tokens, mock data, IPC modules
- Refactor large Rust functions in command_center and daemon server
- Titlebar logo: real logo image replaces "t" placeholder

### Fixed
- README markdown rendering on Overview tab with Shiki fallback
- Duplicate Windows Terminal tab on session launch
- Flaky Codex idle detection tests

## [0.3.0] - 2026-02-24

### Added
- Bootstrap chain: auto-start daemon and tmux on app launch
- Daemon status indicator in sidebar footer
- Setup guide documenting prerequisites and bootstrap chain
- Per-project position memory with `$bindable` pattern

### Changed
- Refactored R01-R14: dynamic paths, a11y fixes, WCAG contrast, cache eviction, layout rebalancing, branch pill contrast, WSL distro validation, shared tool logos
- Improved sidebar visual hierarchy: brighter text, branch pills, spacing

### Fixed
- Daemon spawn: use long-lived `wsl.exe` child instead of detaching
- File tree collapse and layout overflow on Git-to-Files navigation
- Branch/dirty status not showing on first launch
- Multi-tool session activity detection reliability

## [0.2.1] - 2026-02-23

### Added
- Code theme selector in Settings (light and dark Shiki themes)

## [0.2.0] - 2026-02-23

### Added
- Multi-CLI session management: Claude Code, Codex, Gemini CLI
- Live activity detection per tool (IO hysteresis, TCP sockets, file mtime)
- Tool indicator logos in sidebar (Anthropic, OpenAI, Gemini)
- Context menu with per-tool launch, stop, restart
- HoverCard showing all running sessions per project
- Git tab with commit history, inline diffs, infinite scroll, cross-tab navigation
- Session history enrichment with commit and file change context
- Kanban-style task board aggregating tasks from Claude Code, Codex, Gemini

### Changed
- Session store: groups sessions as `Map<path, session[]>` for multi-tool support
- Sidebar indicators: monochrome SVG logos with activity-state colors

## [0.1.0] - 2026-02-21

### Added
- Initial release: Tauri 2 + Svelte 5 + Rust scaffold
- SQLite database with project CRUD and migrations
- Git module: commit history, status, diffs via libgit2
- File browser with syntax-highlighted preview (Shiki)
- Session handoff parser (YAML frontmatter + JSON sidecar)
- File watcher with notify + ignore crates
- Full-text search with tantivy and Cmd+K overlay
- Relationship auto-detection from Cargo.toml, CLAUDE.md, sessions
- Settings persistence (KV store)
- First-run wizard with project scanning
- Floating Panel layout with dark teal frame
- Light/dark theme toggle
