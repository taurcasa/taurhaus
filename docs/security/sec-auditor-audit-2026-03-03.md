# Security Audit Report

**Project**: taurhaus
**Audit Type**: Deep Audit (Rust/Tauri desktop app, frontend, SQLite, daemon, coordination)
**Date**: 2026-03-03
**Auditor**: taurhaus-sec-auditor
**Scope**: `/home/mstie/projects/taurhaus/src-tauri/src`, `/home/mstie/projects/taurhaus/src`, Tauri config/capabilities, dependency/tooling checks
**Status**: Final

---

## Executive Summary

Completed a full directed audit across Tauri IPC surfaces, file/daemon paths, search/session ingestion flows, frontend rendering, and dependency controls. I identified **3 actionable findings**: **1 MEDIUM** and **2 LOW**. The highest risk issue is a symlink-based project-boundary bypass in incremental indexing that can ingest out-of-project file contents into the searchable index.

## Summary Table

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| F-01 | MEDIUM | Symlink escape in incremental indexing can ingest out-of-project file content | Open |
| F-02 | LOW | Frontend log bridge allows log-forging via unsanitized newline/control characters | Open |
| F-03 | LOW | `cargo deny` policy is misconfigured and currently non-functional | Open |

## Findings

### F-01: Symlink Escape In Incremental Indexing Can Ingest Out-of-Project File Content

**Severity**: MEDIUM

**Location**: `src-tauri/src/search/indexer.rs:423`, `src-tauri/src/search/indexer.rs:445`

**Reachability**: Filesystem event under watched project -> `process_watch_events` -> `search::indexer::update_file` -> `std::fs::read_to_string(absolute_path)`.

Call chain:
- `src-tauri/src/fs/watcher.rs:257` classifies regular file events without rejecting symlinks.
- `src-tauri/src/event_processor.rs:293` forwards each changed path into `search::indexer::update_file`.
- `src-tauri/src/search/indexer.rs:423-446` only checks `strip_prefix(project_root)` on the symlink path string, then reads file contents directly.

**Description**:
Incremental indexing trusts watcher-provided absolute paths and does not canonicalize before reading. If a file under the project root is a symlink to an external file, `update_file` will read the symlink target and index its contents under the project document path. This bypasses the stronger canonicalization guard used by normal file reads (`fs/reader.rs`), enabling out-of-project content ingestion and exposure through search snippets.

**Evidence**:
```rust
// src-tauri/src/search/indexer.rs
let relative = match absolute_path.strip_prefix(project_root) {
    Ok(r) => r.to_string_lossy().to_string(),
    Err(_) => return Ok(false),
};
...
let content = match std::fs::read_to_string(absolute_path) {
    Ok(c) => c,
    Err(_) => { ... }
};
```

```rust
// src-tauri/src/fs/reader.rs (existing stronger guard in a different code path)
let canonical = full_path.canonicalize()?;
let canonical_root = project_root.canonicalize()?;
if !canonical.starts_with(&canonical_root) {
    return Err(AppError::InvalidPath("Path resolves outside project directory".to_string()));
}
```

**Fix Effort**: Moderate

**Fix**:
```text
In `search::indexer::update_file`:
1. Resolve `canonical_root = project_root.canonicalize()` and `canonical_file = absolute_path.canonicalize()`.
2. Reject when `!canonical_file.starts_with(canonical_root)`.
3. Reject symlinks explicitly (`symlink_metadata(...).file_type().is_symlink()`) or only index canonical non-symlink files under root.
4. Normalize `relative` from canonical path.
```

**Verify**:
- Add regression test: create project dir + symlink `notes.md -> /tmp/outside.md`, call `update_file`, assert it returns `Ok(false)` (or equivalent rejection) and index doc count remains unchanged.
- Re-run search tests and watcher-driven incremental index update tests.

---

### F-02: Frontend Log Bridge Allows Log-Forging Via Unsanitized Newline/Control Characters

**Severity**: LOW

**Location**: `src-tauri/src/commands/logging.rs:17`

**Reachability**: Frontend `console.*` calls -> `src/lib/logger.js` -> Tauri command `frontend_log` -> structured JSONL sink (`taurhaus.log.jsonl`).

**Description**:
`frontend_log` writes user-provided `message` directly into line-oriented logs. Newlines and control characters are not escaped/filtered, so an attacker-controlled string can create forged additional log entries or corrupt log structure. This primarily impacts auditability and incident forensics.

**Evidence**:
```javascript
// src/lib/logger.js
invoke('frontend_log', { level, message: serialize(...args) })
```

```rust
// src-tauri/src/commands/logging.rs
let _ = writeln!(f, "[{timestamp}] [{tag}] [frontend] {message}");
```

**Fix Effort**: Trivial

**Fix**:
```text
Before writing, sanitize message:
- replace '\r' and '\n' with escaped sequences (e.g., "\\n").
- optionally strip other control chars except tab.
- optionally switch to JSON-structured logging to preserve field boundaries.
```

**Verify**:
- Add test calling `frontend_log("info", "a\n[FAKE] entry")` and assert output is a single physical line with escaped newline.

---

### F-03: `cargo deny` Policy Is Misconfigured And Currently Non-Functional

**Severity**: LOW

**Location**: `src-tauri/deny.toml:6`

**Reachability**: CI/local security gate execution (`cargo deny check`) currently fails before policy evaluation.

**Description**:
Dependency-policy enforcement is configured but broken due invalid `deny.toml` schema/value. This disables an intended supply-chain control and increases risk of missed advisory/license/source policy violations.

**Evidence**:
```toml
# src-tauri/deny.toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
```

```text
$ cargo deny check
error[unexpected-value]: expected '["all", "workspace", "transitive", "none"]'
  ┌─ /home/mstie/projects/taurhaus/src-tauri/deny.toml:6:17
6 │ unmaintained = "warn"
```

**Fix Effort**: Trivial

**Fix**:
```text
Update `deny.toml` to current cargo-deny schema for unmaintained dependency handling,
then re-run `cargo deny check` in CI and fail builds on config parse errors.
```

**Verify**:
- `cd src-tauri && cargo deny check` exits 0.
- CI pipeline includes this command and fails on non-zero status.

---

## Scan Results

| Tool | Findings | Fixed | Deferred | Clean |
|------|----------|-------|----------|-------|
| cargo audit | 0 vulnerabilities | N/A | N/A | Yes |
| cargo deny | Config parse failure (control unavailable) | 0 | 1 | No |
| cargo clippy (`-W clippy::all`) | 0 (for audited target) | N/A | N/A | Yes |
| unsafe code grep | 1 app-level unsafe block reviewed | N/A | N/A | Partial |
| gitleaks | 0 leaks | N/A | N/A | Yes |
| npm audit (`--audit-level=high`) | 0 vulnerabilities | N/A | N/A | Yes |

Notes:
- `cargo geiger --forbid-only` was attempted but output was non-actionable/noisy in this environment; manual unsafe review was performed instead.

## Threat Research Notes (Last 12 Months)

- Reviewed recent Tauri/GitHub advisories and RustSec ecosystem coverage relevant to this stack.
- No currently exploitable dependency CVE was identified for the pinned audited versions via `cargo audit`.
- Notable Tauri ecosystem advisory observed (`tauri-plugin-shell` command argument injection) is not directly used in this codebase path (`tauri-plugin-opener` is used instead).

## Recommendations

1. Fix F-01 first and add a regression test specifically for symlink escape on incremental indexing.
2. Apply structured/sanitized logging for frontend log bridge (F-02) to preserve forensic integrity.
3. Repair `cargo deny` config and gate in CI to restore supply-chain policy enforcement (F-03).
4. Add a dedicated symlink-abuse checklist item to filesystem watcher/indexing reviews to prevent regressions.

## Severity Definitions

- **CRITICAL** — Data breach, auth bypass, RCE. Stop everything, fix immediately
- **HIGH** — Privilege escalation, IDOR, crypto weakness. Fix this sprint
- **MEDIUM** — Missing hardening, info leak. Fix next sprint
- **LOW** — Best practice gap. Fix opportunistically
- **INFO** — Awareness note. No action required
