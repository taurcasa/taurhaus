# Gemini CLI → Antigravity CLI (`agy`) removal/replacement inventory

Repo: `~/projects/taurhaus` (READ-ONLY for this pass; branch at inventory
time was `feat/pr17b-accounts-core`, not the `chore/pr14-cleanup` in the session
snapshot — a Codex implementer is moving in the tree. Nothing was modified.)

Scope grep: `grep -rn -i gemini` over `*.rs *.js *.svelte *.ts *.md *.json *.toml
*.txt *.yaml *.yml`, excluding `node_modules/` and `*/target/`.

**Totals: 899 matching lines across 175 files.**
Rust 288 · JS/Svelte 321 · Markdown 263 · YAML/JSON/TXT 27.

---

## 0. What `agy` actually is (verified on this host)

All facts below come from command output or files on this machine. No
`agy install`, no `agy update`, no `--dangerously-skip-permissions` run, no
login flow. No credential contents were read or printed.

| Fact | Evidence |
|---|---|
| Binary at `~/.local/bin/agy`, ELF 64-bit PIE, stripped, 208,429,312 bytes, mtime 2026-08-28 01:06 | `file`, `ls -la` |
| Version `1.1.22` | `agy --version` |
| Upstream `github.com/google-antigravity/antigravity-cli` | changelog URL string in binary: `https://raw.githubusercontent.com/google-antigravity/antigravity-cli/refs/heads/main/CHANGELOG.md` |
| **Native Go binary, not a Node wrapper** | ELF, not `node .../cli.mjs` |

### `agy --help` (verbatim flag set)

```
--add-dir  --agent  -c/--continue  --conversation  --dangerously-skip-permissions
--disable-slash-commands  --effort (low|medium|high)  -i/--prompt-interactive
--input-format  --json-schema  --log-file  --mode (accept-edits, plan)
--model  --new-project  --output-format  -p/--print  --print/--prompt
--print-timeout  --project  --prompt-interactive  --sandbox
subcommands: agent(s) changelog help install mcp mic-serve models plugin(s) update
```

Consequences for the registry:
- **No `--yolo`.** Auto-approve is `--dangerously-skip-permissions` (same spelling
  as Claude Code).
- **No `--resume`.** Continue is `-c` / `--continue`; resume-by-id is
  `--conversation <ID>`.
- **Model flag is `--model`, not `-m`.** (Gemini's registry entry declares `-m`.)
- **`--effort` EXISTS and is argument-style** — `EffortFlag::Argument { flag:
  "--effort" }`, vocabulary `low|medium|high`. Gemini's entry declares
  `effort_flag: None`; agy can declare it. This is a capability *upgrade*, not a
  rename.

### `agy models` (verbatim, run in a scratch dir)

```
gemini-3.7-flash-high / -medium / -low
gemini-3.6-flash-high / -medium / -low
gemini-3.5-flash-high / -medium / -low
gemini-3.1-pro-high / gemini-3.1-pro-low
claude-sonnet-4-6
claude-opus-4-6-thinking
gpt-oss-120b-medium
```

Notes: model ids still start with `gemini-` for the Google models, but the
catalog is **cross-vendor** (Claude + GPT-OSS ids too). Effort is baked into most
ids *and* there is a separate `--effort` flag — how the two interact is
**UNVERIFIED** (would be verified by a `--print` run with `--model
gemini-3.1-pro-low --effort high` and reading the reported model in the
response/log).

This breaks two existing registry assumptions:
- `model_prefixes: &["gemini-"]` no longer identifies the harness (agy can run
  `claude-opus-4-6-thinking`).
- `model_markers: &["gemini"]` feeds `infer_from_model()`, which would now
  mis-route `claude-*` ids offered by agy to `CliTool::Claude`.

### On-disk layout (listing only; no file contents beyond the small JSON maps)

`agy` **reuses `~/.gemini/` as its base dir** but with a completely different
internal layout:

```
~/.gemini/
  antigravity-cli/
    conversations/<uuid>.db        <- SQLite, plus -shm / -wal
    presence/<uuid>.lock
    brain/<uuid>/{scratch,.system_generated,.user_uploaded}
    cache/projects.json            (exists, empty on this host)
    log/cli-YYYYMMDD_HHMMSS.log
    knowledge/ implicit/*.pb scratch/ crashes/ builtin/ annotations/ updater/ bin/
    settings.json                  (per binary strings)
  config/{config.json,mcp_config.json,projects/*.json,.migrated}
  projects.json                    { "/abs/project/path": "project-name" }
  history/<project-name>/.project_root      -> absolute path
  tmp/<project-name>/{.project_root,chats/,logs,logs.json}
  google_accounts.json  oauth_creds.json  settings.json  state.json
  trustedFolders.json  installation_id
```

Observed on this host: exactly one conversation db
(`d40c37fb-…-863016.db`), and **`~/.gemini/tmp/taurhaus/chats/` is EMPTY**
(created 2026-08-27 20:40, zero files). That is the exact directory today's
`GeminiResolver` reads. **So the current resolver returns "always idle" against
agy.** This is the single highest-value finding for the rewrite.

`~/.gemini/projects.json` on this host:
`{"projects":{"~/projects/taurhaus":"taurhaus","~/projects/localllms":"localllms"}}`

### Env vars (NAMES only, from binary strings)

- **`GEMINI_CLI_HOME` does not appear in the agy binary at all** (grep count 0).
  The registry's `account_selector: Some("GEMINI_CLI_HOME")` has **no verified
  replacement**.
- Names present that look relevant: `ANTIGRAVITY_EXECUTABLE_DATA_DIR`,
  `ANTIGRAVITY_CONVERSATION_ID`, `ANTIGRAVITY_PROJECT_ID`,
  `ANTIGRAVITY_LS_ADDRESS`, `AGY_CLI_HIDE_LOGO`, `AGY_CLI_HIDE_ACCOUNT_INFO`,
  `AGY_ADC_AUTH`. Semantics **UNVERIFIED** — would be verified by running a
  scratch `--print` with `ANTIGRAVITY_EXECUTABLE_DATA_DIR` pointed at a temp dir
  and checking whether `~/.gemini` or the temp dir grows.
- Recommendation: ship `account_selector: None` for agy until one is verified.
  Note `CliCapabilities.account_selector: Option<&'static str>` already models
  absence, and `cli_tool.rs:488,501` already return `None` providers for
  Codex/Gemini, so `None` is a supported state.

---

## 1. Registry — `src-tauri/src/session_scanner/cli_tool.rs` (37 hits)

The single source of truth. Everything downstream keys off it.

| Line(s) | What it is | Replacement |
|---|---|---|
| 3 | Module doc "Supports Claude Code, Codex CLI, and Gemini CLI" | rename |
| 20 | `enum CliTool { … Gemini }` variant | **rename** → `Agy` (serde `rename_all="lowercase"` makes the wire value `"agy"`) |
| 259–305 | The whole `CliToolSpec` entry | mixed — table below |
| 260–262 | `tool/name/aliases: &["gemini"]` | rename → `name: "agy"`, `aliases: &["agy", "antigravity"]` (keep `"gemini"` as a legacy alias only if old persisted team configs must still load — see §11) |
| 263 | `argv_signatures: &["gemini", "@google/gemini-cli"]` | **rewrite** — agy is a native Go binary; the `@google/gemini-cli` npm-path signature is dead. New: `&["agy"]` (+ maybe `"antigravity-cli"`). Verified by `file` on the binary. |
| 264 | `model_prefixes: &["gemini-"]` | **rewrite** — agy serves `claude-*` and `gpt-oss-*` too; a prefix list can no longer identify it. Either enumerate all catalog prefixes or make `model_is_compatible` catalog-driven. |
| 265 | `model_markers: &["gemini"]` | **rewrite** — `infer_from_model()` would mis-route agy's `claude-*` ids. Consider `&[]` (like Codex) and let `bridged_default()` handle it. |
| 266–270 | `default_commands` `gemini --yolo{,--resume}` | **rewrite** → `agy --dangerously-skip-permissions`, continue `agy --dangerously-skip-permissions -c`, resume `agy --dangerously-skip-permissions --conversation <id>` (resume-by-id is not a bare flag — see §2) |
| 271 | `label: "Gemini"` | rename → `"Antigravity"` (or `"Agy"`) |
| 272 | `accent: "violet"` | rename (same slot; frontend reads it from the contract) |
| 273 | `medallion_accent: "sky"` | rename |
| 274 | `default_agent_role_id: "custom-doc-writer"` | rename (see §7 — role fixtures) |
| 276 | `model_flag: Some("-m")` | **rewrite** → `Some("--model")` (verified in `--help`) |
| 277 | `effort_flag: None` | **rewrite** → `Some(EffortFlag::Argument { flag: "--effort" })` (verified) |
| 281 | `session_source: true` | keep, but the impl is a rewrite (§3) |
| 282–286 | `runtime_session_capture/authoritative_idle/compaction_hook/transcript_parser/transcript_compaction_signals: false` | keep false initially; `--log-file` and `presence/<uuid>.lock` are candidate upgrades (UNVERIFIED) |
| 287 | `catalog: true` | keep; contents rewrite (§5) |
| 289 | `account_selector: Some("GEMINI_CLI_HOME")` | **rewrite → `None`** (env var absent from binary) |
| 296 | `stop_strategy: StopStrategy::SlashExit` | keep (`exit_command: "/exit"`) — **UNVERIFIED for agy**; would be verified by an interactive pane test |
| 297 | `process_activity_signal: ProcessActivitySignal::Tcp` | **rewrite** — the TCP heuristic was calibrated on gemini-cli (§4). agy's SQLite+WAL conversation store makes mtime-on-`.db-wal` a better candidate. |
| 299–300 | `display_name/settings_label: "Gemini CLI"` | rename → `"Antigravity CLI"` |
| 301 | `base_dir_name: ".gemini"` | **keep as-is** — verified: agy still uses `~/.gemini` |
| 302 | `projects_subdir: "tmp"` | **rewrite** → `antigravity-cli/conversations` (note: this is a *two-segment* path; check every `join(projects_subdir)` call site tolerates it) |
| 303 | `session_extension: "jsonl"` | **rewrite** → `"db"` |
| 332–338 | `command_settings_for()` match arm `CliTool::Gemini => &settings.gemini` | rename (must move with `CliCommandSettings.gemini`, §5) |
| 488, 501 | `CliTool::Codex \| CliTool::Gemini => None` (account/usage provider) | rename |
| 523–524, 535–537 | `static GEMINI: OnceLock<GeminiResolver>` in `session_source()` | rewrite (new resolver type) |
| 552 | `CliTool::Gemini => &NONE` in `activity_source()` | rename |
| 572 | `CliTool::Gemini => None` in `compaction_signal_source()` | rename |
| 585 | `CliTool::Gemini => None` in `transcript_parser()` | rename |
| 596, 603–605 | `static GEMINI` in `session_resolver()` | rewrite |
| 626–627, 637–638, 650–651, 660, 667 | inline `#[cfg(test)] mod tests` assertions | rename (`"agy"` wire value, `.gemini` base dir assertion stays) |

---

## 2. Launch rendering — `launch.rs` + goldens (13 hits + 2 fixture files)

| File:line | What | Replacement |
|---|---|---|
| `src-tauri/src/session_scanner/launch.rs:360–385` | The `CliTool::Gemini` arm of `LaunchSpec::render()`. Pushes `EffortIgnored{reason: Invalid}` for any requested effort, then appends the model flag unless the base already has `-m`/`--model`. Carries the comment `// unverified (S12): Gemini is not installed on the audit host.` | **rewrite** — the effort-ignored branch is now wrong (agy has `--effort`). With `effort_flag: Some(Argument{"--effort"})` the arm can merge into the Claude-shaped branch at `:300–314`, which already handles argument-style effort + base override detection. The stale "unverified" comment goes. |
| `src-tauri/tests/fixtures/launch/gemini.golden.txt` | one line: `gemini --yolo -m 'gemini-3.1-pro'` | **rewrite** file (rename to `agy.golden.txt`; new content e.g. `agy --dangerously-skip-permissions --model 'gemini-3.1-pro-high' --effort 'high'`) |
| `src-tauri/tests/fixtures/launch/account-dirs.golden.txt:3` | `gemini=GEMINI_CLI_HOME='/accounts/gemini' gemini --yolo` | **rewrite or delete the row** — with `account_selector: None` the selector-prefix path yields `LaunchNote::CapabilityMissing{Selector}` instead of a prefix (`launch.rs:390–406`). The golden's shape changes. |
| `launch.rs:1136–1158` | test `gemini_render_adds_model_and_notes_unsupported_effort` | rewrite (the "unsupported effort" premise is now false) |
| `launch.rs:1160–1178` | test `gemini_render_respects_long_model_flag_and_notes_it` with `// Regression: 791f6be checked only Gemini's short model flag` | **keep the regression, rewrite the data** — the regression (long-flag detection) is still real; with `model_flag: "--model"` the mirror case becomes "base already has `--model`". Per CLAUDE.md the regression test stays forever. |

Note: `docs/design/harness-realignment-plan.md:75` records that the launch
goldens are treated as **immutable** ("the immutable launch goldens … remain
unchanged"). Changing `gemini.golden.txt` is a deliberate break of that
convention and should be called out in the PR description.

---

## 3. Idle / session source — `idle/` (49 hits)

| File:line | What | Replacement |
|---|---|---|
| `src-tauri/src/session_scanner/idle/gemini.rs` (298 lines, 38 hits) | `GeminiResolver` — resolves `~/.gemini/tmp/<dir-name-or-sha256(path)>/chats/session-*.json`, takes the newest `.json`, extracts the session id as the substring after the final `-` in the file stem, classifies by mtime vs `ACTIVE_THRESHOLD` (5s), returns `authoritative: false`. Implements both `SessionResolver` and `SessionSource` (the PR 15 addition). Has a `BASE_DIR_FOR_TEST` mutex override. | **REWRITE — mechanism change.** agy writes `~/.gemini/antigravity-cli/conversations/<uuid>.db` (+ `-shm`/`-wal`), not `tmp/<x>/chats/*.json`. Verified: `tmp/taurhaus/chats/` is empty while a conversation db exists. The `sha256(project_path)` scheme is dead; project scoping now comes from `~/.gemini/projects.json` (path→name) and `~/.gemini/tmp/<name>/.project_root` (name→path). Session id = the `.db` file stem (a UUID). Activity signal should be the `-wal` mtime (writes land there first), which needs verification. |
| `idle/gemini.rs:164–183` | `gemini_runtime_source_preserves_transcript_identity` — `// Regression: f90b362 replaced Gemini's project-scoped resolver with NoSessionSource` | **keep the regression, rewrite the fixture** — this is a live guard from PR 15 and per CLAUDE.md must survive. Rewrite the tempdir fixture to the conversations/db layout. |
| `idle/mod.rs:6` | doc line `**Gemini CLI**: ~/.gemini/tmp/<sha256(path)>/chats/session-*.json` | rewrite |
| `idle/mod.rs:26, 37` | `mod gemini;` / `pub use gemini::GeminiResolver;` | rename file + type |
| `idle/mod.rs:40` | `ACTIVE_THRESHOLD` doc "Used for Claude and Gemini" | rename |
| `idle/mod.rs:285–291` | `project_path_sha256()` — doc says "Used by Gemini CLI which stores sessions under `~/.gemini/tmp/<sha256>/`" | **likely DELETE** — check for other callers first; if `GeminiResolver` was the only one, the fn and the `sha2`/`hex` deps for it go with it. |
| `idle/mod.rs:402` | `resolver_for(CliTool::Gemini)` smoke assertion | rename |
| `idle/mod.rs:427–443` | `#[ignore]` live test `live_gemini_resolver_finds_session` | rewrite (or delete — it is `#[ignore]`d and hardcodes `/home/testuser/...`) |

Downstream: `cli_tool.rs:518–539` (`session_source()`) and `:589–607`
(`session_resolver()`) both instantiate `GeminiResolver` via `OnceLock` — both
follow the rewrite.

---

## 4. Process activity signal — `proc_io.rs` / `classification.rs` / `scans.rs`

| File:line | What | Replacement |
|---|---|---|
| `proc_io.rs:12–18` | Module doc: the whole TCP-socket strategy paragraph, calibrated to gemini-cli ("creates HTTPS connections on demand and closes them when idle") | **rewrite/delete** — the calibration is gemini-cli-specific and unverified for agy. |
| `proc_io.rs:27–28` | Empirical note "Gemini idle at prompt: 0 ESTABLISHED to :443 / working: 1+" (Feb 2026) | rewrite — must be re-measured against agy or deleted as stale. agy is a Go binary with its own connection pooling; the Codex-style keep-alive failure mode is a live risk. |
| `proc_io.rs:42` | rate-threshold doc "far below the smallest Codex/Gemini bursts (7K+)" | rename |
| `proc_io.rs:154, 159` | `// TCP socket detection (Codex/Gemini)` + `has_api_connections()` doc "primary activity signal for Gemini" | rewrite |
| `classification.rs:43` | "For Claude/Gemini we keep the existing behavior: project-level file signal" | rename (but re-check: the new resolver is conversation-scoped, not project-scoped) |
| `classification.rs:177–181, 291–299, 634, 652` | `ProcessActivitySignal::{ReadChars,Tcp}` dispatch + the `"tcp"` telemetry label + two test arms using `Tcp` | **rewrite if the signal changes** — if agy moves to `ReadChars` or a new WAL-mtime signal, `ProcessActivitySignal::Tcp` may become dead (no remaining user) and can be deleted along with `has_api_connections()` and `platform::has_established_443()`. Verify no other tool declares `Tcp` first. |
| `scans.rs:47` | doc "**TCP sockets** (Gemini only): ESTABLISHED connection to remote port 443" | rewrite/delete with the above |

`ProcessActivitySignal::Tcp` is declared by exactly one registry entry (Gemini)
— confirmed by reading `cli_tool.rs`. So the whole TCP branch is Gemini-only
today.

---

## 5. Process detection, models, settings

| File:line | What | Replacement |
|---|---|---|
| `process.rs:688` | `detect_cli_tool` doc: "**Gemini**: `gemini`, `/path/to/gemini`, `node .../@google/gemini-cli/...`" | rewrite |
| `process.rs:701` | comment about `node --no-warnings=DEP0040 /run/.../gemini --yolo` | delete (node-wrapper case does not exist for a Go binary) |
| `process.rs:1446–1481, 1506, 1523–1524` | 9 `detect_gemini_processes` assertions + a `ps` output fixture, all node/fnm/nvm shaped | **rewrite** — replace with agy argv shapes (`agy`, `/home/user/.local/bin/agy --dangerously-skip-permissions`). The `matches_argv_token` `@`-prefix branch (`cli_tool.rs:507`) exists only for `@anthropic-ai/claude-code` and `@google/gemini-cli`; after removal only Claude uses it. |
| `models/mod.rs:263, 284, 336(cli_tool.rs), 1397–1400` | `CliCommandSettings { pub gemini: ToolCommands }`, its `Default`, and the default-command test | **rename with a serde migration** — see §11 |
| `models/mod.rs:486, 561–566, 611, 652, 1561–1564, 1614–1615` | `ModelCatalog.gemini` field, the single `gemini-3.1-pro` / "Gemini 3.1 Pro" entry with `efforts: &[]`, the `entries_for` arm, `supports_effort → false`, and 2 tests | **rewrite** — real catalog from `agy models` is 14 ids across three vendors; effort vocab becomes `["low","medium","high"]` (a new `AGY_EFFORTS` const alongside `CLAUDE_EFFORTS`/`CODEX_EFFORTS_*`), and `supports_effort` gets a real arm instead of `false`. |
| `models/mod.rs:1397–1400` | `assert_eq!(cmds.gemini.fresh, "gemini --yolo")` etc. | rewrite |
| `platform_paths.rs:338, 346` | test `assert_eq!(gemini, home.join(".gemini").join("tmp"))` — derives from `base_dir_name` + `projects_subdir` | **rewrite** — becomes `.gemini/antigravity-cli/conversations` |
| `lib.rs:98` | env probe command echoing `GEMINI_API_KEY=$GEMINI_API_KEY` (name only; the value is echoed into a probe, so check where the output lands) | rename or delete — agy uses OAuth (`~/.gemini/settings.json` says `selectedType: oauth-personal`), so `GEMINI_API_KEY` may no longer be the right probe. |
| `platform/windows.rs:3` | doc "CLI tools (claude, codex, gemini) run inside WSL2" | rename |
| `control.rs:301` | doc "Claude & Gemini: `/exit` text command" | rename (verify agy's exit) |
| `control.rs:522` | `"GEMINI_API_KEY"` in an env passthrough/allowlist | **allowlist — see §10** |
| `control.rs:679–706, 713` | `build_gemini_fresh_command` / `build_gemini_resume_command` tests + a comment about the free-text Settings field | rewrite |

---

## 6. Task scanning (23 + 12 + 7 + 2 hits)

| File:line | What | Replacement |
|---|---|---|
| `src-tauri/src/task_scanner/gemini.rs` (500 lines) | Parses `TODO.md` markdown checkboxes in the project dir under source key `"gemini-todo"`; also `session_time_range()` / `find_gemini_session_file()` reading `~/.gemini/tmp/<…>/chats/*.json` (`:198`, `:196–258`) and `gemini_session_id_from_file` / `gemini_time_range_from_file` | **split**: the TODO.md checkbox parser is harness-agnostic and can be **renamed** (source key `agy-todo`); the session-file half (`:173–300`) is **rewrite** — same dead `tmp/<x>/chats` path as §3. |
| `task_scanner/mod.rs:1, 6, 15, 52, 61, 85–87` | module doc, `pub mod gemini`, the `gemini::get_tasks` wiring, `get_gemini_tasks` generic param, `apply_source_outcome(&mut result, "gemini", …)` with the comment "Gemini's TODO.md integration is project-local, not a verified transcript" | rename (source string `"gemini"` → `"agy"` is a **persisted DB value** — see §11) |
| `task_scanner/types.rs:3` | doc "tasks from any CLI tool (Claude, Codex, Gemini)" | rename |
| `services/task_query.rs:581` | `"gemini" => crate::task_scanner::gemini::session_time_range(...)` — a **string-literal dispatch** on the persisted source column | rename + migration |
| `db/task_queries.rs:703, 711, 1281–1440` (12 hits) | test fixtures using source `"gemini"` and `default_source_key("gemini")` | rename |

---

## 7. Coordination / templates / roles

| File:line | What | Replacement |
|---|---|---|
| `coordination/backend/bridged.rs:250` | hardcoded warning string `"Unsupported CLI tool '{}' for agent '{}'. Choose claude, codex, or gemini."` | **rename — this is a literal allowlist in prose.** The neighbouring `required_binary_for_cli_tool` (`:265`) and `cli_tool_label` (`:270`) already derive from the registry, so only this string is hardcoded. |
| `coordination/backend/bridged.rs:107` | comment "PATH-based lookup for tmux, claude, codex, gemini, etc." | rename |
| `coordination/backend/selector.rs:57, 65` | `assert_eq!(selector.select(CliTool::Gemini), BackendKind::MeshBridged)` | rename |
| `coordination/member_activation.rs:375–385` | test with `cli_tool: "gemini"`, `model: "gemini-2.5-pro"` | rewrite (`gemini-2.5-pro` is not in agy's catalog) |
| `coordination/stores/config.rs:2140` | persisted-config test fixture `"cli_tool": "gemini"` | rename + migration (§11) |
| `coordination/orchestrator/tests.rs:888, 1052, 4271–4277` | incl. `initialize_team_gemini_lead_launch_new_uses_sidecar_lifecycle` with `// Regression: e86980b used Gemini's project-scoped SessionSource as a …` | **keep the regression, rewrite data** — it guards the sidecar-lifecycle path for a non-native-inbox lead. |
| `coordination/pipelines/tests.rs:1339–1368` | asserts `"gemini --yolo --sandbox read-only -m 'gemini-2.5-pro'"` (a user-override base + model append) | rewrite |
| `commands/coordination/tests.rs:216, 392–452, 2441, 2901–2902` | `MockBinaryLookup::with_available(&[… "gemini"])` and `"Gemini CLI not found"` | rewrite (binary name `agy`, label `Antigravity`) |
| `commands/command_center/tests.rs:885, 918–926, 1828, 1842` | per-tool launch-command table + `assert_eq!(stored_tool, "gemini")` | rewrite |
| `templates/adapters.rs:184, 331, 547, 604–605, 1461–1462, 1591–1598` | `RoleExportFormat::GeminiMd` → serde `"gemini_md"` → `render_instruction_only_document("GEMINI.md", …)` | **KEEP AS-IS or handle separately.** `GEMINI.md` is Gemini-CLI's *instruction file convention*, not the harness. agy reads project instruction files too (needs verification of the filename — `AGENTS.md` vs `GEMINI.md`). Removing the export format is a **separate decision** from removing the harness; deleting it silently drops a user-facing export target. |
| `templates/adapters.rs:17` | doc "`instruction_only` covers `AGENTS.md`, `GEMINI.md`, Cursor rules, Windsurf rules" | as above |
| `templates/adapters.rs:1031` | comment "Canonical names only (`claude`/`codex`/`gemini`): the exporter …" | rename |
| `templates/storage/roles.rs:331` | `RoleExportFormat::GeminiMd => "gemini_md"` | as above |
| `templates/types.rs:1187–1188` | assertion that the built-in role `gemini-orchestrator` exists | rename |

### Bundled role YAMLs (`src-tauri/resources/templates/roles/`)

| File | Lines | Replacement |
|---|---|---|
| `gemini-orchestrator.yaml` | `:5 role_id`, `:6 name`, `:11 cli_tool: gemini`, `:12 model: gemini-3.1-pro`, `:30` prose "Operate through Gemini CLI conventions…" | **rename the file + role_id**, rewrite the model to a real agy id |
| `gemini-ui-specialist.yaml` | `:5,:6,:11,:12` | rename file + role_id + model |
| `taurhaus-designer.yaml` | `:11 cli_tool: gemini`, `:12 model: gemini-3.1-pro` | rename in place (file name is tool-neutral) |

Note `gemini-3.1-pro` is **not** an id `agy models` returns —
`gemini-3.1-pro-high` / `gemini-3.1-pro-low` are. All three YAMLs need the model
value corrected, not just the tool name.

**User role stores shadow the bundle** (`storage/mod.rs:398-404, 471-478`, per
`harness-realignment-plan.md:117`), so a renamed built-in role does not
retroactively fix a user's saved copy referencing `cli_tool: gemini`.

`~/projects/taureval` — **checked, zero gemini references.** Its
`roles/` dir holds only `v2-*`/`v3-*` claude/codex files. Nothing to do there.

---

## 8. Frontend (321 JS/Svelte hits)

### Source (non-test)

| File:line | What | Replacement |
|---|---|---|
| `src/lib/toolRegistry.js:86–115` | `FALLBACK_TOOLS` third entry: `id:'gemini'`, `label:'Gemini'`, `displayName:'Gemini CLI'`, `accent:'violet'`, `medallionAccent:'sky'`, `defaultAgentRoleId:'custom-doc-writer'`, `aliases:['gemini']`, and the full capability block incl. `modelFlag:'-m'`, `effortFlag:null`, `accountSelector:'GEMINI_CLI_HOME'` | **rename slot, rewrite data** — must mirror the Rust registry exactly (it is the fallback for when the contract is unavailable). This file is the **only** frontend file allowed to spell a tool name (§10). |
| `src/lib/fixtures/tool-registry.json:63–84` | same entry as a JSON fixture | rewrite to match |
| `src/lib/toolLogos.js:11, 25–28, 41–44` | `TOOL_ICONS.gemini` (Google four-pointed sparkle, 65×65 viewBox) and `TOOL_SIDEBAR_SMALL_ICONS.gemini` (16×16) | **rewrite** — the Google sparkle is Gemini branding, not Antigravity's. Needs a new icon path. Keyed by the tool id string, so the key renames with the id. |
| `src/lib/sessionIndicator.js:8` | `const TOOL_ORDER = ['claude','codex','gemini']` | rename (hardcoded ordering array) |
| `src/lib/modelCatalog.js:12` | `EMPTY_MODEL_CATALOG = Object.freeze({ claude: [], codex: [], gemini: [] })` | rename |
| `src/lib/ipc/system.js:23–26, 33, 112, 148, 192, 258` | the frontend fallback `TerminalPlatformContract`: `gemini` default commands (`gemini --yolo …`), `gemini: []` catalog, and 4 normalizer call sites | **rewrite** (commands) + rename (keys) |
| `src/lib/Settings.svelte:70–73` | `cloneCliCommands()` hardcodes the three keys incl. `gemini` | rename. Note the *rows* render from `tools()` (`:26 const cliTools = $derived(tools())`), so only the clone helper is hardcoded. |
| `src/lib/Sidebar.svelte:540, 542, 546` | context-menu items `'New Gemini Session'` / `'Resume Gemini'` with literal `'gemini'` tool arg | rename |
| `src/lib/OverviewTab.svelte:158` | `const TOOLS = ['claude','codex','gemini']` | rename |
| `src/lib/TaskBoard.svelte:415` | empty-state copy "…when Claude, Codex, or Gemini create plans…" | rename |
| `src/lib/components/MeshNodeDetail.svelte:584` | `<option value="gemini">Gemini</option>` | rename |
| `src/lib/components/MeshRuntimeView.svelte:679` | `<option value="gemini">Gemini</option>` | rename |
| `src/lib/components/MeshRoleEditorDialog.svelte:356` | `<option value="gemini">Gemini</option>` | rename |
| `src/lib/components/MeshAvailabilityGate.svelte:48–50` | fixture agent `name:'gemini-check'`, `cliTool:'gemini'`, `model: defaultModelFor(catalog,'gemini')` | rename |
| `src/lib/components/RoleCatalog.svelte:34, 48–49` | export-format option `{ value:'gemini_md', label:'GEMINI.md' }` | tied to §7's `GeminiMd` decision |
| `src/lib/components/templateBrowserController.svelte.js:33–34` | `case 'gemini_md': return 'GEMINI.md'` | same |
| `src/lib/ipc/mocks/base.js` (13), `mocks/templates.js` (10), `mocks/tasks.js` (3) | IPC mock payloads | rename |
| `src/visual-host/hosts/RosterDesign{A,B,C}Host.svelte` (4/4/6) | e.g. `RosterDesignBHost:39–60` — `if (tool === 'gemini') return dark…` and `{claude:'C', codex:'X', gemini:'G'}[tool]` | rename (visual-host is outside the `src/lib` boundary guard, so these literals are legal today) |

**`MeshTeamBuilder.svelte` contains ZERO `gemini` literals** — it is fully
registry-driven after PR 15. Only its test file (`MeshTeamBuilder.test.js`, 14
hits: `roleId:'agent-gemini'`, `cliTool:'gemini'`, `model:'gemini-2.5-pro'`,
`tools:['gemini','claude']`, and 6 `data-testid` assertions) carries them.

**`errorCopy.js` contains ZERO `gemini` (and zero claude/codex) literals** —
contrary to `harness-realignment-plan.md:73` which lists `errorCopy.js` with 3
branches; PR 15 removed them. Nothing to do there.

### Test / fixture files (rename fixture data)

`TemplateBrowserPanel.test.js` (24) · `sessionIndicator.test.js` (21) ·
`ipc.test.js` (16) · `MeshTeamBuilder.test.js` (14) · `MeshTab.test.js` (14) ·
`sidebar.test.js` (7) · `settings.test.js` (7) · `sessionStore.test.js` (7) ·
`taskDetailPanel.test.js` (6) · `TeamCustomizerPreset.test.js` (6) ·
`ModelSelect.test.js` (6) · `taskBoard.test.js` (5) · `overviewTab.test.js` (5) ·
`modelCatalog.test.js` (5) · `Sidebar.component.test.js` (5) ·
`meshDefaults.test.js` (4) · `AgentCard.test.js` (4) · `toolRegistry.test.js` (2)
· `sessionHistory.test.js` (2) · `meshTabUtils.test.js` (2) ·
`RoleCatalog.test.js` (2) · `MeshCanvas.test.js` (2) · `HoverCard.test.js` (2) ·
`meshLayout.test.js` (1) · `mockData.test.js` (1) · `SidebarAccounts.test.js` (1)
· `toolRegistryBoundary.test.js` (1, see §10).

### Visual fixtures / specs

`meshNodeDetail.fixtures.js` (20 — incl. exported scenario names
`idle_gemini_dark`, `cross_project_gemini_dark`, `cross_project_gemini_light`,
which are **screenshot filenames**) · `sidebar.fixtures.js` (11 — incl. a
`labels: [… 'Gemini: running']` assertion) · `readmeScreenshots.fixtures.js` (7)
· `modelSelect.fixtures.js` (5) · `meshTeamBuilder.fixtures.js` (4) ·
`settings.visual.test.js` (4) · `meshNodeDetail.visual.test.js` (3) ·
`builders.js` (3 — `toolCycle` array + a `tool === 'gemini' ? 'Gemini' :` ternary
+ `model: … '2.5-pro'`) · `rosterDesigns.fixtures.js` (2) ·
`meshCanvas.fixtures.js` (2) · `hoverCard.fixtures.js` (1) ·
`src/test/fixtures/modelCatalog.js` (3).

Renaming scenario keys invalidates the committed screenshot baselines — plan a
baseline refresh.

---

## 9. E2E, docs, CHANGELOG

### E2E (`e2e/specs/`)

| File:line | What | Replacement |
|---|---|---|
| `overview-interactions.js:148, 150` | `it('Gemini launch button exists and is enabled')` + `$('[data-testid="action-launch-gemini"]')` | rename (the testid is generated from the tool id) |
| `command-center-real-actions.js:137`, `mesh-recovery.js:111`, `session-management.js:143` | `gemini: canonicalizeToolCommands(cliCommands.gemini)` | rename |

### Docs — top-level, must change

| File:line | What |
|---|---|
| `CLAUDE.md:236` | "Gemini's `-m` arm is **unverified**" — now verifiably wrong (`--model`, and it is installed) |
| `CLAUDE.md:373–374`, `AGENTS.md:363–364` | "Gemini Pro 3 cross-review" / "The UI specialist (Gemini) is the design lead" — **process** references, not harness. Decide separately whether to re-point at agy. |
| `AGENTS.md:57` | "## Mesh Team Coordination (for Codex/Gemini agents)" |
| `README.md:7, 17, 57, 113` | product copy + the install link `https://github.com/google-gemini/gemini-cli` → must become the Antigravity CLI source |
| `ARCHITECTURE.md:23, 69, 111, 223, 246, 253, 264` | incl. the harness table row `\| Gemini CLI \| Process name + SHA-256 path hash \| TCP socket state to :443 \|` — **both columns are now wrong** |
| `docs/architecture/harness-model.md:3, 7, 39` | the authoritative harness doc; `:39` states Gemini "declares its selector in the registry" — becomes false with `account_selector: None` |
| `docs/architecture/data-model.md` (3), `data-architecture.md` (3), `daemon-protocol.md` (2), `path-handling-guide.md` (1) | |
| `docs/operations/testing-guide.md:213` | "Cross-review by Gemini Pro 3" — process, not harness |
| `docs/features/*.md` | `session-management.md` (8), `task-board.md` (8), `command-center.md` (5), `mesh.md` (3), `project-management.md` (2), `first-run-and-settings.md` (1) |
| `docs/getting-started.md` (8), `docs/team-templates.md` (8), `docs/coordination-architecture.md` (4), `docs/ui/layout-and-navigation.md` (2), `docs/ui/design-system.md` (1), `docs/RETROSPECTIVE.md` (3), `SECURITY.md` (1) | |
| `docs/images/infographics.manifest.yaml` (9) | infographic source text — the rendered `.jpg`s carry "Gemini" pixels and would need re-rendering |
| `docs/design/harness-realignment-plan.md` (18) | living plan doc; `:73`, `:75`, `:91` ("Gemini is floor-only today and **unverified** throughout (no binary on PATH…)"), `:117`, `:118`, `:135`, `:172`, `:196`, `:212` (S12 "Gemini `-m`, `--yolo`, model ids (no binary installed)" — **now answered by this inventory**), `:217` |
| `docs/design/accounts-and-usage-plan.md` (9) | |
| `CHANGELOG.md:93, 97, 372, 696, 700, 732, 785, 1057, 1059, 1064` | **HISTORICAL — do not rewrite.** Shipped-release records. A new entry describes the swap. |

### Docs — historical, leave alone

`docs/research/compaction-detection-across-cli-tools.md` (32 — the single
largest file, a dated research record), `docs/research/*` (skill-md-format 6,
mesh-jsonl-evaluation 5, operational-learnings-for-roles 2,
developer-environment-bundling 1), all of `docs/analysis/*` (~35 across 14 dated
audit files), all of `docs/archive/*` (~30 across 17 files), and the two
untracked `docs/pi-*.md` drafts (4 + 1).

### Adjacent repo (list only — DO NOT TOUCH)

`~/projects/mesh`: `src/daemon.rs:351`
(`matches!(basename.as_str(), "claude" | "codex" | "gemini")` — **a literal
allowlist that will reject an `agy` pane**) and `:521` (comment). Docs:
`README.md:3`, `USAGE.md` (12 hits incl. a "Gemini CLI | `--yolo`" flag table at
`:90` and a troubleshooting row at `:478`), `docs/taurhaus-integration-proposal.md`
(6). `mesh/src/daemon.rs:351` is the one that is functionally load-bearing.

---

## 10. Literal guards / allowlists that must be updated

These fail the build (or silently stop guarding) if the token set changes.

| File:line | Guard | Action |
|---|---|---|
| `src-tauri/tests/module_boundary_assertions.rs:70` | `fn cli_tool_literal_count` — array `["CliTool::Claude","CliTool::Codex","CliTool::Gemini"]` | **update the literal array** |
| `src-tauri/tests/module_boundary_assertions.rs:220–235` | `ALLOWED_RUNTIME_FILES` — 13 paths allowed to spell `CliTool::*`, incl. `"src/task_scanner/gemini.rs"` and `"src/session_scanner/idle/claude.rs"`/`codex.rs` (note: `idle/gemini.rs` is **NOT** in the list) | **update paths on rename**; adding `idle/agy.rs` needs a decision — today the Gemini idle slice is *not* allowlisted |
| `src-tauri/tests/module_boundary_assertions.rs:236` | `const EXPECTED_RUNTIME_LITERAL_COUNT: usize = 66;` | **will change** — a hard-coded count; any arm added/removed shifts it |
| `src-tauri/tests/module_boundary_assertions.rs:305–340` | `generic_account_core_contains_no_tool_identity_literals` — forbids `"CliTool::Gemini"` and `"gemini"` in 3 generic account files | **update the literal list** |
| `src/lib/toolRegistryBoundary.test.js:6–7` | `ALLOWED_FILES = new Set(['toolRegistry.js','ipc/mocks/tasks.js'])` and the regex `/(?:[!=]==\s*['"](?:claude\|codex\|gemini)['"]\|case\s+…\|includes\(…\|['"]…['"]\s*:)/g` | **update the regex alternation** |
| `src/lib/toolRegistryBoundary.test.js:36` | `expect(allowedComparisonCount).toBe(2)` | hard-coded count; verify after the swap |
| `src-tauri/src/coordination/backend/bridged.rs:250` | prose allowlist `"Choose claude, codex, or gemini."` | update string |
| `src-tauri/src/session_scanner/control.rs:522` | `"GEMINI_API_KEY"` inside an env passthrough list | update or drop (agy authenticates via OAuth on this host) |
| `src-tauri/src/lib.rs:98` | env probe echoing `GEMINI_API_KEY` | update or drop |
| `~/projects/mesh/src/daemon.rs:351` | `matches!(basename, "claude"\|"codex"\|"gemini")` | **cross-repo** — an `agy` pane is rejected until this is updated |
| `src-tauri/tests/harness_conformance.rs:52–56, 217–223, 238–268, 433–453, 604–628, 683, 715` | the parameterised conformance suite: golden-file table entry, capability assertions (`!runtime_session_capture`, `!compaction_hook`, `!transcript_parser`, `StopStrategy::SlashExit`), the `(CliTool::Gemini, Some("GEMINI_CLI_HOME"))` selector table, the "no account provider" assertion, and the PR-15 regression `// Regression: f90b362 mapped Gemini's declared session source to the floor` | **every assertion needs revisiting** — several become false (effort flag now exists; selector becomes `None`) |

---

## 11. Migration hazards (persisted values)

1. **`CliCommandSettings` has no `#[serde(default)]` / `#[serde(alias)]`**
   (`models/mod.rs:258–273`). Renaming the `gemini` field means an existing
   settings row `terminal.cli_commands` (persisted as JSON,
   `db/settings_queries.rs:51, 248–250`) fails to deserialize — and
   `settings_queries.rs:107–119` **catches the failure and falls back to
   defaults for the whole struct**, silently discarding the user's customised
   *Claude and Codex* commands too. Add `#[serde(alias = "gemini")]` on the new
   field, or a one-shot migration, before renaming.

2. **`CliTool` serde value `"gemini"` is persisted** in team configs
   (`coordination/stores/config.rs:2140` fixture shows `"cli_tool": "gemini"`),
   in role YAMLs under the user's template store, and in the tasks DB `source`
   column (`services/task_query.rs:581` dispatches on the string). Keeping
   `"gemini"` in `aliases` makes `CliTool::from_alias` still accept it
   (`cli_tool.rs:59–66`), but plain `FromStr` (`:44–54`) matches only `name`, and
   `#[derive(Deserialize)]` matches only the lowercase variant name. Decide
   explicitly whether old values load.

3. **Screenshot baselines** — renamed visual fixture scenario keys
   (`idle_gemini_dark` etc.) change output filenames.

4. **`EXPECTED_RUNTIME_LITERAL_COUNT = 66`** and
   `allowedComparisonCount === 2` are hard-coded counts that will drift.

---

## 12. Suggested ordering

1. Registry entry + enum variant + `CliCommandSettings` serde alias (§1, §5, §11).
2. Rewrite `idle/gemini.rs` → `idle/agy.rs` against the real conversations/db
   layout, keeping the two PR-15 regressions green (§3).
3. Decide the activity signal; if `Tcp` loses its last user, delete the branch
   and `has_established_443` (§4).
4. Launch arm + goldens + conformance suite (§2, §10).
5. Model catalog from `agy models`, effort vocab `low|medium|high` (§5).
6. `process.rs` argv fixtures (§5).
7. Frontend registry fallback + fixture JSON + icon, then the mechanical
   fixture/test renames (§8).
8. Role YAMLs with corrected model ids (§7).
9. Docs — live docs only; leave `docs/analysis`, `docs/archive`,
   `docs/research`, and `CHANGELOG` history alone (§9).
10. Cross-repo: `mesh/src/daemon.rs:351` (§9, §10).

---

## 13. Open questions / UNVERIFIED

| Claim | How to verify |
|---|---|
| agy's exit command is `/exit` (`stop_strategy: SlashExit`) | interactive pane; type `/exit` and observe |
| The right activity signal (WAL mtime? presence lock? TCP?) | run a long `--print` in a scratch dir and sample `-wal` mtime, `presence/*.lock`, and `/proc/PID/net/tcp` |
| Whether an `agy` conversation `.db` can be attributed to a project without opening SQLite | inspect table names with `sqlite3 … .tables` on a **copy** of the db |
| Whether `--effort` and the effort-suffixed model ids conflict | `agy -p --model gemini-3.1-pro-low --effort high 'reply with the single word OK'` in a scratch dir, then read `~/.gemini/antigravity-cli/log/cli-*.log` |
| Any home/config-dir override env var (for per-account selection) | run a scratch `--print` with `ANTIGRAVITY_EXECUTABLE_DATA_DIR=<tmp>` and see whether `~/.gemini` or `<tmp>` grows |
| Whether agy reads `GEMINI.md` or `AGENTS.md` as project instructions (decides the `GeminiMd` export format's fate) | place both in a scratch dir and ask the model which it saw |
| Whether `--yolo` is silently accepted as an alias | `agy --yolo --help` in a scratch dir (safe, no session) |

None of these were probed further, to stay inside the "stop at the first sign of
interactive sign-in / no real-project runs" rule. `agy --version`, `agy --help`,
and `agy models` all ran in the scratch directory and completed without any
sign-in prompt (the host already holds an OAuth session:
`~/.gemini/settings.json` → `security.auth.selectedType: "oauth-personal"`; the
credential file was listed but never read or printed).
