# Taurhaus Security Audit (Task #56)

Date: 2026-03-04  
Auditor: taurhaus-security-expert

## Scope
- Full-project security audit of `taurhaus` (Tauri desktop app, Rust backend + Svelte frontend).
- Reviewed IPC surface, daemon auth/transport, filesystem boundaries, command execution paths, coordination runtime, and markdown/rendering pipeline.

## Automated Checks
- `cargo audit` (in `src-tauri`): no vulnerable Rust advisories reported.
- `cargo deny check` (in `src-tauri`): passed (warnings only: duplicate crate versions / unencountered license).
- `cargo clippy -- -W clippy::all` (in `src-tauri`): passed.
- `gitleaks detect --source .`: no leaks found.
- `npm audit --audit-level=high` (repo root): 0 vulnerabilities.
- Unsafe code grep:
  - crate has `#![deny(unsafe_code)]` with one scoped exception.
  - one `unsafe` block found in `src-tauri/src/lib.rs` for libgit2 owner validation toggle.
- `cargo geiger --forbid-only`: tool instability in this environment (hung with registry parse errors); replaced with manual unsafe review + `#![deny(unsafe_code)]` verification.

## Findings

### F-01: Insecure-by-default autonomous agent launch flags
**Severity**: HIGH  
**Location**: `src-tauri/src/models/mod.rs:221`, `src-tauri/src/session_scanner/control.rs:406`  
**Reachability**: UI session launch (`launch_claude_session`) -> default command resolution -> tmux launch command execution.  
**Description**: Default CLI command templates launch Claude/Codex/Gemini with non-interactive high-trust flags (`--dangerously-skip-permissions`, `--yolo`). This removes user approval checkpoints by default and materially increases impact from prompt-injection or malicious repository content.  
**Evidence**:
- `claude --dangerously-skip-permissions ...` defaulted in `CliCommandSettings::default()`.
- `codex --yolo` / `gemini --yolo` defaulted similarly.
**Fix Effort**: Moderate  
**Fix**: Change defaults to safe/interactive modes; require explicit user opt-in for dangerous flags in settings or first-run flow, with clear risk warning and rollback path.  
**Verify**: Fresh install should show safe commands in settings and launched sessions should prompt/confirm privileged operations unless user explicitly opts in.

### F-02: Git ownership safety check is globally disabled
**Severity**: MEDIUM  
**Location**: `src-tauri/src/lib.rs:80`  
**Reachability**: App startup -> `disable_git_owner_validation()` -> all libgit2 repository operations for app lifetime.  
**Description**: The app unconditionally disables libgit2 owner validation, removing a defense meant to prevent use of untrusted/dubious-ownership repositories. This broadens trust to any opened repo and increases exposure on shared/multi-user filesystems.  
**Evidence**:
- `git2::opts::set_verify_owner_validation(false);` called at startup.
**Fix Effort**: Moderate  
**Fix**: Keep owner validation enabled by default; implement explicit trust/allowlist (for known WSL/UNC paths) instead of global disable.  
**Verify**: Repos with mismatched ownership should fail by default; trusted paths should work only after explicit allowlisting.

### F-03: External URL opener allows insecure `http://` destinations
**Severity**: LOW  
**Location**: `src-tauri/capabilities/default.json:13`, `src/lib/MarkdownRenderer.svelte:161`  
**Reachability**: README/markdown link click -> `openExternalUrl()` -> opener plugin with capability allowlist including `http://**`.  
**Description**: The opener capability permits all HTTP links. For untrusted project documentation, this allows downgrade to plaintext transport and increases phishing/MITM risk compared with HTTPS-only handling.  
**Evidence**:
- Capability includes `{ "url": "http://**" }`.
- Markdown click handler opens `http(s)` links via opener.
**Fix Effort**: Trivial  
**Fix**: Restrict allowlist to `https://**` by default. If HTTP is needed, require explicit warning/confirmation per click.  
**Verify**: HTTP links should be blocked or prompt a warning; HTTPS links should open normally.

## Residual Risk / Notes
- No SQL injection paths observed; DB access uses parameterized rusqlite queries.
- Filesystem read paths include traversal/symlink escape checks and 5 MB size guardrails.
- Daemon traffic is localhost-bound with per-request auth token validation and bounded line reads.
- CSP and DOM sanitization are present for rendered markdown; no direct XSS bypass confirmed in this audit.

