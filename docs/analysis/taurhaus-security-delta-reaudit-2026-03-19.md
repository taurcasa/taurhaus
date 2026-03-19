# Security Re-Audit Delta Report

**Project**: taurhaus  
**Audit Type**: Delta re-audit after quality phase  
**Date**: 2026-03-19  
**Auditor**: security-auditor  
**Baseline**: `docs/analysis/taurhaus-security-full-audit-2026-03-19.md`  
**Status**: Final

---

## Executive Summary

I completed the final security re-audit requested for quality-phase task `#1348`, comparing the current working tree against the 2026-03-19 full audit and the current risk register.

Result:

- `F-03` is **resolved**
- `F-01` and `F-02` remain **accepted risks** for this quality phase and are **not re-flagged**
- I found **no new actionable security findings** in the reviewed quality-phase changes

Overall posture improved modestly from the baseline audit because the only dependency finding from the initial report was fixed and I did not identify any new auth, injection, path-boundary, or opener regressions in the newly added code paths.

---

## Delta Verdict

| Item | Outcome | Notes |
|---|---|---|
| `F-01` unsafe launch defaults | Accepted risk, unchanged | Still documented in `docs/security/risk-register.md`; not re-flagged per task instruction |
| `F-02` shared tmux session credential exposure | Accepted risk, unchanged | Still documented in `docs/security/risk-register.md`; not re-flagged per task instruction |
| `F-03` `lz4_flex` via `tantivy` | Resolved | `cargo audit` and `cargo deny check` no longer report the advisory |
| Quality-phase code changes | No new findings | Manual review plus targeted tests did not surface a new exploitable issue |

---

## Scope Reviewed

Focused review areas requested by the lead:

- `src-tauri/src/services/scan_policy.rs`
- terminal contract IPC and settings reconciliation paths
- `src-tauri/src/tmux_layout.rs`
- `src/lib/errorCopy.js`
- `src/lib/a11y.js`
- runtime split under `src-tauri/src/coordination/runtime/`
- startup split under `src-tauri/src/startup/`
- stall detector split under `src-tauri/src/coordination/stall_detector/`
- structured logging additions in startup and command-center flows

I also re-checked surrounding call chains where those changes are reached:

- `src-tauri/src/commands/command_center/*`
- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/commands/search.rs`
- `src-tauri/src/search/indexer.rs`
- `src-tauri/src/services/scanner.rs`

---

## Automated Verification

| Tool / command | Result | Delta interpretation |
|---|---|---|
| `cargo audit` | Clean | Confirms prior `F-03` advisory is gone |
| `cargo deny check` | Clean for advisories/bans/licenses/sources | Confirms prior `F-03` advisory is gone |
| `SQLX_OFFLINE=true cargo clippy -- -W clippy::all` | Passed with warnings only | No new blocking correctness/security signal |
| `rg 'unsafe ' --type rust ...` | No active unsafe blocks | Prior `#![deny(unsafe_code)]` posture preserved |
| `gitleaks detect --source .` | Clean | No new secret leak in repo |
| `osv-scanner scan source -r .` | Mixed, unchanged ecosystem/dev-tool noise | No reappearance of `lz4_flex`; remaining results match prior transitive/dev-path noise |
| `bun audit` | Mixed, unchanged dev/test tooling advisories | No new production-path delta from quality changes |

Targeted regression checks:

- `cargo test matcher_ignores_matching_paths_and_descendants --lib`
- `cargo test scan_directory_honors_saved_ignore_patterns --lib`
- `cargo test rebuild_index_honors_saved_ignore_patterns --lib`
- `cargo test per_project_policy_reuses_matching_window_name --lib`
- `bun test src/lib/errorCopy.test.js`

All targeted checks passed.

---

## Manual Review Notes

### 1. Scan policy and indexing changes

The new saved ignore-pattern flow is security-positive and did not regress the earlier project-boundary fix.

- `ScanIndexPolicy` normalizes user patterns and merges them with a conservative default ignore set.
- Directory scanning and full-index rebuild now honor saved ignore patterns.
- The earlier symlink escape fix remains intact because incremental indexing in `search/indexer.rs` still canonicalizes both `project_root` and `absolute_path` before indexing or removal.

Assessment: no new traversal or indexing-boundary issue found.

### 2. Terminal contract IPC and tmux layout changes

The terminal contract work is mainly validation and UI/backend consistency hardening rather than new privilege surface.

- Runtime contract data constrains the frontend to supported emulator choices for the current platform.
- The reviewed launch/navigation/stop paths still pass tmux and daemon arguments as structured argv arrays rather than shell-concatenated strings.
- `tmux_layout.rs` resolves split targets deterministically from parsed tmux window metadata; I did not find a new shell-injection path in the allocation logic itself.

Assessment: no new IPC trust-boundary or tmux-target injection issue found in the reviewed delta.

### 3. Runtime/startup/stall-detector refactors

The runtime/startup/stall-detector changes are primarily code-motion and decomposition with behavior preserved.

- Startup fast-path daemon validation still checks protocol version and binary version before trusting an existing daemon.
- Runtime process helpers still validate pid files against expected process identity before trusting them.
- Stall-detector split logic uses bounded subprocess timeouts and parses Mesh JSON defensively.

Assessment: no new daemon-auth bypass, stale-pid trust, or monitor-spawn regression found in the refactored paths.

### 4. Structured logging additions

The reviewed logging additions increase operational visibility but did not cross the threshold into a new finding.

- I did not find API keys, daemon auth tokens, or command-override strings being written by the newly reviewed startup and command-center log events.
- New log fields do include operational metadata such as project paths, tmux identifiers, and local filesystem paths.

Assessment: this remains worth watching for future redaction discipline, but in the current form I do not consider it a new actionable security finding.

### 5. Frontend helper additions

- `errorCopy.js` only maps backend/system failures to fixed user-facing strings; no HTML rendering or opener expansion was introduced.
- `a11y.js` manages focus and modal isolation only; I did not find an XSS, opener, or privilege-boundary implication in the reviewed code.

Assessment: no security finding.

---

## New Findings

None.

I did not identify a new issue that met the reporting gate of being externally reachable today, unmitigated by current controls, realistically exploitable end-to-end, and actionable within current constraints.

---

## Resolved Since Baseline

### F-03: `lz4_flex` via `tantivy`
**Status**: Resolved

Verification:

- `cargo audit` no longer reports `RUSTSEC-2026-0041`
- `cargo deny check` now completes with advisories clean

Impact on posture:

- The only dependency finding from the initial audit is gone
- The quality-phase tree is therefore strictly better than the baseline on dependency posture

---

## Accepted Risks Not Re-Flagged

Per `docs/security/risk-register.md` and the task instruction for this re-audit:

- `F-01` unsafe-by-default launch flags remain accepted risk for this phase
- `F-02` shared-session tmux credential exposure remains accepted risk for this phase

These risks still exist technically, but this report does not re-open them as delta findings.

---

## Overall Posture Assessment

Security posture improved versus the initial 2026-03-19 audit.

Why:

1. One prior finding (`F-03`) is now fixed.
2. The requested quality-phase code paths did not introduce a new actionable regression.
3. Previously verified fixes remain intact, including the incremental-indexing symlink boundary and the restricted external-opener posture.

Current residual risk is still dominated by the already-accepted design choices tracked as `F-01` and `F-02`, not by newly introduced quality-phase defects.
