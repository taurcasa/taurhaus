# Security Audit Report

**Project**: taurhaus  
**Audit Type**: Deep Audit  
**Date**: 2026-03-19  
**Auditor**: security-auditor  
**Scope**: Full codebase (`src-tauri/src`, `src`, Tauri config/capabilities, dependency manifests, daemon/process/coordination surfaces)  
**Status**: Final

---

## Executive Summary

I completed a full project-wide security audit of Taurhaus as a Tauri desktop application with a local TCP daemon, tmux-based tool launching, mesh coordination, filesystem access, and persistent search/log state.

Current posture is still generally strong: path-boundary checks are present on file reads and incremental indexing, daemon auth is token-gated on localhost, Tauri opener permissions are narrowed to HTTPS/mailto, HTTP markdown links are blocked, `git2` has been upgraded to `0.20.4`, `cargo deny` is functioning again, and the crate now contains no active `unsafe` usage.

I validated **3 actionable findings**: **1 HIGH, 1 MEDIUM, 1 LOW**.

## Summary Table

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| F-01 | HIGH | Taurhaus still launches Claude/Codex/Gemini in no-approval mode by default | Open |
| F-02 | MEDIUM | API keys are still exposed to every pane in the shared `taurhaus` tmux session | Open |
| F-03 | LOW | Search index path still ships with vulnerable `lz4_flex` via `tantivy` | Open |

---

## Findings

### F-01: Taurhaus still launches Claude/Codex/Gemini in no-approval mode by default
**Severity**: HIGH  
**Location**: `src-tauri/src/models/mod.rs:236`, `src-tauri/src/session_scanner/control.rs:504`, `src/lib/Settings.svelte:137`

**Reachability**: User launches a CLI session from the Taurhaus UI -> `launch_cli_session_impl()` loads terminal settings -> `resolve_configured_tool_command()` selects the default command -> Taurhaus launches the tool locally or through the daemon. This is a normal product path today for fresh, continue, resume, and team-launch flows.

**Description**: Taurhaus still ships default launch commands that disable approval/permission gates for all supported agents: Claude uses `--dangerously-skip-permissions`, while Codex and Gemini use `--yolo`. That makes prompt injection or malicious repository content substantially more dangerous because the launched agent receives shell/filesystem access immediately, with no approval checkpoint.

**Evidence**:

```text
src-tauri/src/models/mod.rs
236 continue_cmd: "claude --dangerously-skip-permissions --continue"
237 fresh: "claude --dangerously-skip-permissions"
238 resume: "claude --dangerously-skip-permissions --resume"
241 continue_cmd: "codex --yolo"
242 fresh: "codex --yolo"
243 resume: "codex resume --last --yolo"
246 continue_cmd: "gemini --yolo --resume"
247 fresh: "gemini --yolo"
248 resume: "gemini --yolo --resume"

src-tauri/src/session_scanner/control.rs
504 "claude --dangerously-skip-permissions --continue"
505 "claude --dangerously-skip-permissions"
506 "claude --dangerously-skip-permissions --resume"
509 "codex --yolo"
510 "codex --yolo"
511 "codex resume --last --yolo"
514 "gemini --yolo --resume"
515 "gemini --yolo"
516 "gemini --yolo --resume"

src/lib/Settings.svelte
137 continue_cmd: 'claude --dangerously-skip-permissions --continue'
142 continue_cmd: 'codex --yolo'
147 continue_cmd: 'gemini --yolo --resume'

src-tauri/src/commands/command_center/launching.rs
93  let ts = load_terminal_settings(db);
94  let tool_cmd = resolve_configured_tool_command(...)
107 command_override: Some(tool_cmd)
```

Validation:
- `cargo test build_claude_fresh_command --lib` -> passed
- `cargo test terminal_settings_default_includes_cli_commands --lib` -> passed

**Fix Effort**: Moderate

**Fix**: Change the shipped defaults to the tools' approval-preserving modes and make unsafe flags an explicit opt-in setting with a warning. Update both the Rust defaults and the frontend settings defaults together, then migrate persisted settings carefully so existing users are not silently left on dangerous defaults.

**Verify**: From a clean profile, launch each tool and confirm the spawned args omit `--dangerously-skip-permissions` and `--yolo`. Update the default-command unit tests so they fail if unsafe flags reappear.

---

### F-02: API keys are still exposed to every pane in the shared `taurhaus` tmux session
**Severity**: MEDIUM  
**Location**: `src-tauri/src/session_scanner/control.rs:525`, `src-tauri/src/session_scanner/control.rs:571`

**Reachability**: Taurhaus launches or resumes a CLI session -> `ensure_taurhaus_session()` creates/uses the shared `taurhaus` tmux session -> `propagate_env_to_tmux()` writes API keys into that session environment -> any process in any pane that can run `tmux show-environment -t taurhaus` can read them. In practice, F-01 makes this especially easy for launched AI agents.

**Description**: Taurhaus no longer writes secrets to tmux global environment, but it still writes them into the shared `taurhaus` session environment. That is not a secret-isolation boundary. Every pane in that session shares the same session environment, so one compromised or malicious workload can read the API keys used by every other workload in the session.

**Evidence**:

```text
src-tauri/src/session_scanner/control.rs
525 pub const TMUX_SESSION_NAME: &str = "taurhaus";
556 // Propagate critical env vars to tmux global environment.
571 fn propagate_env_to_tmux() {
572     const PROPAGATE_VARS: &[&str] = &[
573         "ANTHROPIC_API_KEY",
574         "OPENAI_API_KEY",
575         "GEMINI_API_KEY",
576         "NODE_EXTRA_CA_CERTS",
577         "PATH",
583         tmux_command()
584             .args(["set-environment", "-t", TMUX_SESSION_NAME, var, &val])
```

Live reproduction on the audit host:

```text
$ tmux new-session -d -s taursec-audit-$$
$ tmux set-environment -t taursec-audit-$$ TEST_SECRET supersecret123
$ tmux show-environment -t taursec-audit-$$ TEST_SECRET
TEST_SECRET=supersecret123
```

**Fix Effort**: Moderate

**Fix**: Do not store secrets in tmux shared environment at all. Inject required keys only into the specific spawned child process environment or into a short-lived wrapper that exports them immediately before `exec` for that one pane/process. If shared tmux is retained, treat it as shared state and do not use it for credential isolation.

**Verify**: After the fix, launch a session and confirm `tmux show-environment -t taurhaus` does not reveal API keys. Also verify the launched tool still receives the credentials it needs.

---

### F-03: Search index path still ships with vulnerable `lz4_flex` via `tantivy`
**Severity**: LOW  
**Location**: `src-tauri/Cargo.toml:49`, `src-tauri/src/search/indexer.rs:59`, `src-tauri/src/startup/search.rs:14`, `src-tauri/src/commands/search.rs:21`

**Reachability**: App startup opens the persistent search index under `app_data_dir()/search_index` -> later `search()` IPC reads from that index through Tantivy. The vulnerable code path is present in normal product behavior, but exploitability appears limited because Taurhaus generates the index itself; the practical attacker prerequisite is local tampering with the persisted index files.

**Description**: `cargo audit` and `cargo deny` now both report `RUSTSEC-2026-0041` for `lz4_flex 0.11.5`, pulled in through `tantivy 0.22.1`. The advisory affects block decompression. I did not find a realistic project-controlled input path that lets an untrusted repository author feed arbitrary compressed blocks directly into Tantivy, so this is not a high-severity app bug today, but the vulnerable code is still shipped on a reachable persistence path and should be upgraded.

**Evidence**:

```text
src-tauri/Cargo.toml
49 tantivy = "0.22"

src-tauri/src/search/indexer.rs
59 let dir = tantivy::directory::MmapDirectory::open(index_dir)
65 Index::open(dir)

src-tauri/src/startup/search.rs
14 let index_dir = context.data_dir.join("search_index");
15 let search_index = open_with_fallback(&index_dir)?;

src-tauri/src/commands/search.rs
21 pub fn search(...)
39 let index = search_state.0.lock()...
43 index.search(&query, limit)
```

Scanner results:

```text
$ cargo audit
RUSTSEC-2026-0041
Crate: lz4_flex 0.11.5
Dependency tree: lz4_flex -> tantivy 0.22.1 -> taurhaus

$ cargo deny check
error[vulnerability]: RUSTSEC-2026-0041
Solution: Upgrade to >=0.11.6, <0.12.0 OR >=0.12.1
```

**Fix Effort**: Moderate

**Fix**: Upgrade `tantivy` or otherwise force `lz4_flex` to a fixed version if the dependency graph allows it. If immediate upgrade is blocked, document the local-tampering prerequisite explicitly and consider rebuilding the index rather than trusting persisted segments after corruption or version drift.

**Verify**: Rebuild the lockfile, then rerun `cargo audit` and `cargo deny check`. Both should stop reporting `RUSTSEC-2026-0041`.

---

## Scan Results

| Tool | Result | Notes |
|------|--------|-------|
| `cargo audit` | 1 vulnerability | `RUSTSEC-2026-0041` (`lz4_flex` via `tantivy`) |
| `cargo deny check` | 1 vulnerability + yanked warning | Same `lz4_flex` issue; config itself is now valid |
| `SQLX_OFFLINE=true cargo clippy -- -W clippy::all` | Clean | No captured lint findings |
| `rg 'unsafe '` + `rg 'deny\\(unsafe_code\\)'` | Clean | No active unsafe blocks; `#![deny(unsafe_code)]` present |
| `gitleaks detect --source .` | Clean | No secrets detected |
| `osv-scanner scan source -r .` | Mixed | Same Rust finding plus GTK/Tauri transitive noise and JS advisories |
| `bun audit` | Mixed | `undici`, `fast-xml-parser`, `devalue` advisories triaged to dev/test/tooling paths |

Not applicable in this repo snapshot:
- Docker / Trivy / Hadolint: no `Dockerfile`
- CI workflow audit (`zizmor`, `actionlint`): no `.github/workflows/`
- Terraform / IaC: no `.tf` files

## Fixed Or Not Reproduced Since Earlier Audits

- The incremental-indexing symlink escape is fixed: `search/indexer.rs` now canonicalizes both project root and candidate path before indexing.
- The prior direct `git2` advisory is fixed: `git2` is now `0.20.4`.
- The insecure HTTP external-link path is fixed:
  - opener capability is limited to `https://**` and `mailto:*`
  - frontend blocks `http://` markdown links before calling the opener
- I found no active `unsafe` block in the current crate.
- The old `cargo deny` config breakage is fixed; the command now runs and reports real advisories.

## Triage Notes

- The GTK/GDK/ATK/unmaintained transitive advisories reported by `osv-scanner` match the known Tauri-on-Linux ecosystem pattern and are already handled in `src-tauri/.cargo/audit.toml`.
- The JavaScript advisories currently land in dev/test/tooling paths:
  - `fast-xml-parser` through WebdriverIO browser tooling
  - `undici` through `jsdom` and WebdriverIO
  - `devalue` through the Svelte toolchain
- I reviewed the current Tauri capability/CSP posture and did not find a new capability overexposure issue:
  - no `shell:allow-execute`
  - opener restricted to HTTPS/mailto
  - CSP present in `tauri.conf.json`

## Recommendations

1. Make AI agent launches safe by default and require explicit opt-in for no-approval flags.
2. Stop storing API keys in shared tmux environment state; scope them to the one launched process that needs them.
3. Upgrade the `tantivy` / `lz4_flex` path until `cargo audit` and `cargo deny` are clean again.
4. Keep the current path-boundary and opener hardening in place; those earlier fixes are meaningful and should retain regression coverage.
