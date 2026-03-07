# Taurhaus Security Audit — Team Lead Report

**Date**: 2026-03-03
**Auditor**: Team Lead (Claude Opus 4.6)
**Framework**: TaurSec v1 (6-phase methodology)
**Scope**: Full codebase — `~/projects/taurhaus` (Tauri 2.x desktop app, Rust backend, Svelte 5 frontend, SQLite, WSL daemon)

## Executive Summary

Taurhaus is a well-engineered desktop application with strong security fundamentals. Path traversal protection is triple-layered, XSS prevention uses DOMPurify consistently, SQL queries are parameterized, the daemon uses constant-time token comparison on localhost only, and Tauri capabilities are minimal. No critical vulnerabilities were found.

The highest-impact finding is a compound attack chain: API keys propagated to tmux global environment combined with unrestricted AI agent defaults (`--dangerously-skip-permissions`) create a prompt injection → API key exfiltration path. This crosses three component boundaries and represents the primary risk to address.

**Finding Summary**: 0 CRITICAL, 1 HIGH, 4 MEDIUM, 4 LOW, 1 INFO

---

## Automated Tool Results

| Tool | Result | Notes |
|------|--------|-------|
| `cargo audit` | CLEAN | 606 deps, 939 advisories checked, 0 matches |
| `cargo deny check` | FAILED | Broken `deny.toml` config (see F-01) |
| `cargo clippy` | CLEAN | Zero warnings (SQLX_OFFLINE=true) |
| `unsafe` code check | 1 block | `lib.rs:85` — git2 owner validation bypass (see F-02) |
| `gitleaks` | CLEAN | 425 commits scanned, 0 secrets |
| `npm audit` | CLEAN | 0 vulnerabilities |

---

## Findings

### F-01: API Keys Exposed via Tmux Global Environment + Unrestricted Agent Defaults
**Severity**: HIGH
**Location**: `src-tauri/src/session_scanner/control.rs:472-490` (env propagation), `control.rs:250-280` (agent defaults)
**Reachability**: `launch_claude_session` IPC → `launch_in_tmux_with_layout` → `propagate_env_to_tmux()` + `build_launch_command()`. Called on every session launch.

**Description**: Two individually medium-severity issues compound into a high-severity attack chain:

1. `propagate_env_to_tmux()` writes `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, and `GEMINI_API_KEY` to tmux **global** environment via `tmux set-environment -g`. Any process in any tmux pane can read these with `tmux show-environment`.

2. `build_launch_command()` hardcodes `--dangerously-skip-permissions` for Claude and `--yolo` for Codex/Gemini as defaults. These flags grant unrestricted filesystem and shell access to the AI agents.

Together: a prompt injection attack against any launched AI agent can execute `tmux show-environment`, read all API keys, and exfiltrate them via the agent's unrestricted capabilities.

**Evidence**:
```rust
// control.rs:472-479
fn propagate_env_to_tmux() {
    const PROPAGATE_VARS: &[&str] = &[
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "NODE_EXTRA_CA_CERTS",
        "PATH",
    ];
```

```rust
// control.rs:254-258 (build_launch_command)
CliTool::Claude => {
    parts.push("--dangerously-skip-permissions".to_string());
}
CliTool::Codex => {
    parts.push("--yolo".to_string());
```

**Fix Effort**: Moderate
**Fix**:
1. Use tmux **pane-level** environment instead of global: `tmux set-environment -t <pane>` (or pass env vars via the launch command's environment, not tmux global state).
2. Make `--dangerously-skip-permissions` / `--yolo` opt-in rather than default. The user-configurable `cli_commands` setting already supports custom commands — use a safer default (e.g., `claude --continue` without the skip flag) and document that users can enable unrestricted mode in settings.

**Verify**: After fix, run `tmux show-environment -g` from a different tmux pane and confirm API keys are NOT visible. Verify default launch command no longer includes permission-bypass flags.

---

### F-02: Broken deny.toml Silently Disables Dependency Security Scanning
**Severity**: MEDIUM
**Location**: `src-tauri/deny.toml`
**Reachability**: CI/CD pipeline (if configured) and local developer workflow.

**Description**: The `deny.toml` configuration contains `unmaintained = "warn"` which is invalid syntax for current `cargo-deny` versions (valid values are `allow`, `deny`, `warn` but only for `[advisories]` section, not at top level or the format has changed). Running `cargo deny check` fails completely with a parse error, silently disabling all dependency license, advisory, and ban checks.

**Evidence**:
```
$ cargo deny check
error: failed to parse deny config: unmaintained = "warn" is not valid
```

**Fix Effort**: Trivial
**Fix**: Update `deny.toml` to use valid cargo-deny v0.16+ syntax. Reference: https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html

**Verify**: `cargo deny check` completes without parse errors.

---

### F-03: Team Name Validation Missing `..` Check (Path Traversal)
**Severity**: MEDIUM
**Location**: `src-tauri/src/coordination/validation.rs:3-10`
**Reachability**: `coordination_create_team` IPC → `orchestrator.create_team()` → `validate_team_name()`. Frontend can call this via Tauri IPC.

**Description**: `validate_team_name()` rejects path separators (`/`, `\`) but does not reject `..`. A team name of `..` would pass validation and `teams_dir.join("..")` resolves to the parent directory. Lock files and team config files would be written outside the intended teams directory.

**Evidence**:
```rust
pub(crate) fn validate_team_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("team name", name)?;
    if has_path_separator(name) {  // Only checks / and \
        return Err(CoordinationError::Validation(...));
    }
    Ok(())
}
```

**Fix Effort**: Trivial
**Fix**: Add `..` check:
```rust
if name == ".." || name == "." || has_path_separator(name) {
    return Err(CoordinationError::Validation(...));
}
```

Apply the same fix to `validate_member_name()`.

**Verify**: `validate_team_name("..")` returns `Err`.

---

### F-04: Inconsistent Error Sanitization Leaks Filesystem Paths
**Severity**: MEDIUM
**Location**: `src-tauri/src/commands/sessions.rs`, `settings.rs`, `search.rs`, `relationships.rs`, `command_center.rs`, `coordination.rs`
**Reachability**: Any IPC command in these modules that encounters a database or IO error.

**Description**: The codebase defines `sanitize_error()` (`errors.rs:33`) which replaces the user's home directory path with `~` to prevent leaking filesystem structure to the frontend. This is applied in `files.rs`, `git.rs`, and `tasks.rs`, but 6 out of 10 IPC command modules use raw `.map_err(|e| e.to_string())` instead, potentially exposing full filesystem paths (e.g., `/home/username/...`) in error messages sent to the WebView.

**Evidence**:
```rust
// sessions.rs:13 — unsanitized
session_queries::get_latest_session(&conn, &project_id).map_err(|e| e.to_string())

// files.rs:31 — sanitized
provider.file_tree(&path).map_err(|e| sanitize_error(&e.to_string()))
```

**Fix Effort**: Trivial
**Fix**: Apply `sanitize_error()` consistently to all `.map_err()` calls in IPC command handlers, or integrate it into the `AppError::Serialize` impl so it's applied automatically.

**Verify**: Trigger an error in `sessions.rs` (e.g., query on missing session) and verify the error message doesn't contain the home directory path.

---

### F-05: `#![deny(unsafe_code)]` Instead of `#![forbid(unsafe_code)]` with Git Owner Validation Bypass
**Severity**: LOW
**Location**: `src-tauri/src/lib.rs:1` (`deny`), `src-tauri/src/lib.rs:83-88` (`allow(unsafe_code)`)
**Reachability**: Application startup.

**Description**: The crate uses `#![deny(unsafe_code)]` which can be overridden with `#[allow(unsafe_code)]` (as demonstrated at line 83). The single unsafe block calls `git2::opts::set_verify_owner_validation(false)`, which disables git's safe directory check — a protection added after CVE-2022-24765 to prevent git from operating on repositories owned by different users.

The `deny` vs `forbid` distinction means future developers can add more `unsafe` blocks with `allow` annotations. The git validation bypass is needed for WSL (cross-filesystem ownership differences) but should be documented.

**Evidence**:
```rust
#![deny(unsafe_code)]  // line 1

#[allow(unsafe_code)]   // line 83 — overrides deny
unsafe { git2::opts::set_verify_owner_validation(false) }
```

**Fix Effort**: Trivial
**Fix**: Keep `deny(unsafe_code)` (since the exception is legitimate for WSL), but add a comment explaining why the exception exists and the security implications. Consider restricting the bypass to Windows/WSL builds only via `#[cfg(target_os = "windows")]`.

**Verify**: Build succeeds, git operations work in WSL projects.

---

### F-06: Missing SQLite `busy_timeout` PRAGMA
**Severity**: LOW
**Location**: `src-tauri/src/db/mod.rs:17-29`
**Reachability**: Database initialization on every app start.

**Description**: `init_db()` correctly sets `journal_mode=WAL` and `foreign_keys=ON`, but does not set `busy_timeout`. Without it, concurrent writes from background threads (event processor, bootstrap scans, watcher callbacks) can immediately fail with `SQLITE_BUSY` instead of waiting for the lock. This may cause intermittent "database is locked" errors under load.

**Evidence**:
```rust
pub fn init_db(path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // Missing: conn.pragma_update(None, "busy_timeout", "5000")?;
    run_migrations(&conn)?;
    Ok(conn)
}
```

**Fix Effort**: Trivial
**Fix**: Add `conn.pragma_update(None, "busy_timeout", "5000")?;` (5 seconds is a reasonable default).

**Verify**: Multiple concurrent DB writes succeed without `SQLITE_BUSY` errors.

---

### F-07: macOS AppleScript Injection via Unsanitized tmux_session
**Severity**: LOW
**Location**: `src-tauri/src/terminal.rs:410-451` (macOS only)
**Reachability**: `navigate_to_session` IPC → `handle_terminal(EnsureOpen)` → AppleScript execution. Only on macOS.

**Description**: The `tmux_session` string is interpolated directly into AppleScript format strings without escaping double quotes. If a malicious tmux_session name containing `"` characters were passed, it could break out of the AppleScript string context and execute arbitrary AppleScript commands.

In practice, `tmux_session` is always the constant `"taurhaus"`, but the `navigate_to_session` IPC command accepts it as a frontend-supplied string parameter.

**Evidence**:
```rust
// terminal.rs:414 — unescaped interpolation into AppleScript
format!(r#"write text "tmux attach-session -t {tmux_session}""#)
```

**Fix Effort**: Trivial
**Fix**: Validate or sanitize `tmux_session` before interpolation — restrict to alphanumeric + `-` + `_` (same pattern as `validate_wsl_distro`), or escape double quotes in the string.

**Verify**: Pass a tmux_session containing `"` and verify it's rejected or escaped.

---

### F-08: Daemon PathParams Accepts Any Filesystem Path
**Severity**: LOW
**Location**: `src-tauri/src/daemon/handlers.rs:85-98` (and similar handlers)
**Reachability**: Any daemon client with a valid auth token.

**Description**: Daemon handlers accept `PathParams.path` (the project root) as any absolute filesystem path. While `read_file` and `read_asset` have path traversal protection for the *relative* path within the project, the project root itself is unconstrained. This means an authenticated daemon client can point `path` to any directory and use `file_tree` to enumerate it or `git_status` to probe if it's a git repository.

**Mitigations**: Daemon requires auth token (32 bytes, 0600 permissions) + binds to 127.0.0.1 only. The Tauri frontend resolves project paths from the database, not from user input.

**Fix Effort**: Moderate
**Fix**: Consider maintaining a list of registered project paths in the daemon and rejecting requests for paths not in the list. This would require the Tauri app to "register" project paths with the daemon.

**Verify**: Daemon rejects `file_tree` requests for paths not in the registered list.

---

### F-09: Daemon Auth Token File Readable by User Processes
**Severity**: INFO
**Location**: `src-tauri/src/daemon/auth.rs`
**Reachability**: Any process running as the same user.

**Description**: The daemon auth token is stored at `~/.local/share/taurhaus/daemon.token` with 0600 permissions. Any process running as the same user can read this file and gain full daemon access. Combined with F-08, this means any local process can read arbitrary files through the daemon.

This is expected behavior for a desktop app (same-user access is the standard trust boundary), but worth noting as a defense-in-depth consideration. The daemon already mitigates this with localhost-only binding.

**Fix Effort**: N/A (defense-in-depth awareness)

---

## Positive Findings

### P-01: Minimal Tauri Capabilities
`capabilities/default.json` grants only `core:default`, window controls, and `opener:allow-open-url` (restricted to `https://` and `http://`). No `fs`, `shell`, or `http` permissions. This is an exemplary minimal-privilege configuration.

### P-02: Triple-Layer Path Traversal Protection
`fs/reader.rs:13-50` implements three independent checks:
1. Rejects absolute paths
2. Rejects `..` path components (using `Component::ParentDir`)
3. Canonicalization check catches symlink escapes

All three must be bypassed for traversal to succeed. Well-tested with 9 unit tests including symlink escape.

### P-03: DOMPurify Sanitization on All `{@html}` Instances
All three `{@html}` usages are safe:
- `MarkdownRenderer.svelte:104` — DOMPurify.sanitize() applied
- `CodeViewer.svelte:60` — Shiki syntax highlighter output (escapes input)
- `ContextMenu.svelte:154` — Hardcoded SVG icons from code constants

### P-04: Parameterized SQL Queries
All database queries use rusqlite's `query!`/`query_as!`/`params![]` macros. The single `format!` in `queries.rs:116` constructs column names from hardcoded strings — values are bound via `params![]`.

### P-05: Daemon Authentication
32-byte random token (OsRng), hex-encoded to 64 chars, file permissions 0600, constant-time XOR comparison in `validate_token()`. `--no-auth` properly gated to debug builds via `#[cfg(debug_assertions)]`.

### P-06: Localhost-Only Daemon Binding
`daemon/server.rs` binds to `127.0.0.1` exclusively. Combined with auth token, daemon is not network-accessible.

### P-07: 1MB Request Size Limit
`daemon/server.rs`: `MAX_REQUEST_LINE_LEN = 1MB` prevents unbounded memory allocation from oversized requests.

### P-08: Command Injection Prevention
`control.rs:validate_command_override()` validates CLI binary allowlist + blocks shell metacharacters (`;|&$\`(){}<!>\n\r`). All process execution uses `Command::new()` (no shell interpretation).

### P-09: CSP Configuration
`tauri.conf.json` sets `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'` — blocks inline scripts and external resources. `connect-src` restricted to `ipc:` and `http://ipc.localhost`.

### P-10: SQLite WAL + Foreign Keys
`db/mod.rs:21-24` properly configures WAL mode and enforces foreign key constraints. Tested with `foreign_keys_are_enforced` test.

### P-11: WSL Distro Validation
`daemon/launcher.rs:39-52` validates distro names against `[a-zA-Z0-9._-]` allowlist — prevents command injection in WSL commands.

### P-12: Tree Walker Symlink Safety
`fs/tree.rs:70` sets `follow_links(false)` — prevents directory traversal via symlinks in file tree enumeration.

### P-13: Clean Dependency Audit
Zero known vulnerabilities across 606 Rust dependencies and all npm dependencies.

### P-14: Zero Secrets in Git History
Gitleaks scan across 425 commits found zero secrets.

### P-15: IPC Commands Database-Scoped
All file/git IPC commands resolve project paths from the database via `resolve_project_path()` rather than accepting raw filesystem paths from the frontend.

---

## Risk Summary

| ID | Title | Severity | Fix Effort |
|----|-------|----------|------------|
| F-01 | API key exposure via tmux + unrestricted agent defaults | HIGH | Moderate |
| F-02 | Broken deny.toml disables dependency scanning | MEDIUM | Trivial |
| F-03 | Team name `..` path traversal | MEDIUM | Trivial |
| F-04 | Inconsistent error sanitization | MEDIUM | Trivial |
| F-05 | `deny` vs `forbid` unsafe + git validation bypass | LOW | Trivial |
| F-06 | Missing SQLite busy_timeout | LOW | Trivial |
| F-07 | macOS AppleScript injection via tmux_session | LOW | Trivial |
| F-08 | Daemon accepts any path as project root | LOW | Moderate |
| F-09 | Daemon token readable by same-user processes | INFO | N/A |

**Recommended fix priority**:
1. F-01 (HIGH) — API key exposure chain. Highest risk to user assets.
2. F-02, F-03, F-04 (MEDIUM, Trivial) — Quick wins with immediate benefit.
3. F-05, F-06, F-07 (LOW, Trivial) — Low-effort improvements.
4. F-08 (LOW, Moderate) — Defense-in-depth enhancement.
