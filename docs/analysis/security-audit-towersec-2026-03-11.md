# Taurhaus Security Audit - TowerSec - 2026-03-11

**Project**: taurhaus  
**Audit Type**: Deep Audit - full application security review  
**Date**: 2026-03-11  
**Auditor**: security-auditor  
**Status**: Final  

## Executive Summary

I audited the current Taurhaus desktop application across the Tauri capability layer, Rust IPC surface, frontend IPC callers, daemon transport/auth, tmux and terminal automation, filesystem boundaries, logging, and coordination runtime edges.

Confirmed findings: 4 total.

- HIGH: 1
- MEDIUM: 1
- LOW: 2

Overall risk is driven less by classic Tauri IPC breakout paths and more by trust-composition failures around AI tool launch defaults and platform automation. I did not confirm any current arbitrary file-read or arbitrary shell-execution IPC path from the WebView, and two previously reported issues are now closed: coordination team-name traversal and incremental-index symlink escape.

## Scope

- `AGENTS.md`
- `ARCHITECTURE.md`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/`
- `src/lib/ipc/`
- `src/lib/logger.js`
- `src/lib/MarkdownRenderer.svelte`
- `src/lib/Settings.svelte`

## Threat Model Snapshot

**Crown jewels**

- Registered project contents and repository metadata
- CLI agent sessions that inherit user API keys and filesystem access
- Coordination runtime state under `~/.claude/teams/...`
- Daemon-mediated access to WSL-side projects and watchers

**Shortest realistic compromise path**

1. User opens or launches work inside an untrusted repository or accepts an untrusted prompt/context payload.
2. Taurhaus starts Claude/Codex/Gemini in an unsafe default mode (`--dangerously-skip-permissions`, `--yolo`).
3. The agent runs with materially reduced approval barriers and can act on attacker-controlled instructions with the user’s local privileges and project/API-key context.

## Confirmed Findings

### F-01: Unsafe Agent Launch Flags Are Shipped As The Default

**Severity**: HIGH

**Location**: `src-tauri/src/models/mod.rs:232`, `src-tauri/src/session_scanner/control.rs:433`, `src/lib/Settings.svelte:134`

**Reachability**: UI launch action -> `launch_cli_session` -> `commands/command_center/launching.rs` -> `resolve_configured_tool_command()` / `build_launch_command()` -> tmux launch command.

**Description**:
Taurhaus defaults Claude, Codex, and Gemini launches into high-trust non-interactive modes. That turns untrusted repository content, README instructions, coordination payloads, or prompt-injection into a much shorter path to filesystem/process actions under the user’s account. This is the highest-risk confirmed issue because it affects the default behavior on fresh installs and normal operator workflows.

**Evidence**:

```text
claude --dangerously-skip-permissions --continue
claude --dangerously-skip-permissions
codex --yolo
gemini --yolo
```

The defaults exist in both backend and frontend fallback settings. I also ran `cargo test build_claude_`, which passed and confirms the shipped Claude defaults still contain `--dangerously-skip-permissions`.

**Fix Effort**: Moderate

**Fix**:
Change all default tool commands to safe interactive modes. Keep dangerous flags behind explicit user opt-in in Settings with a clear warning, and avoid pre-populating them for fresh installs.

**Verify**:

- Fresh settings on a clean profile must not contain `--dangerously-skip-permissions` or `--yolo`.
- Launching each tool should require its normal approval/sandbox flow unless the user explicitly enables the dangerous mode.

### F-02: Git Dubious-Ownership Protection Is Disabled For The Entire Process

**Severity**: MEDIUM

**Location**: `src-tauri/src/lib.rs:97`, `src-tauri/src/lib.rs:302`

**Reachability**: App startup -> `disable_git_owner_validation()` -> every subsequent libgit2-backed git operation for the lifetime of the process.

**Description**:
Taurhaus globally disables libgit2 owner validation to support WSL UNC repositories. That removes Git’s dubious-ownership defense for all repositories, not just the UNC/WSL cases that motivated the workaround. On shared or mis-owned filesystems this broadens trust to repositories Git would normally reject.

**Evidence**:

```rust
unsafe {
    let _ = git2::opts::set_verify_owner_validation(false);
}
```

The option is applied unconditionally during startup.

**Fix Effort**: Moderate

**Fix**:
Keep owner validation enabled by default. Apply a narrower trust mechanism only for explicit WSL/UNC cases, for example a per-path allowlist or a conditional provider-specific bypass rather than a global process-wide setting.

**Verify**:

- A mismatched-owner repository should fail by default.
- Explicitly trusted WSL/UNC repositories should still work through the intended exception path.

### F-03: Project Content Can Open Plaintext HTTP URLs

**Severity**: LOW

**Location**: `src-tauri/capabilities/default.json:15`, `src/lib/MarkdownRenderer.svelte:215`

**Reachability**: Untrusted project README/markdown link click -> `openExternalUrl()` -> opener capability allows `http://**`.

**Description**:
The main-window capability explicitly allows plaintext HTTP destinations. Taurhaus renders project-controlled markdown and opens external links from that content. This is not code execution, but it weakens transport guarantees and increases phishing / MITM exposure for links surfaced from untrusted repositories.

**Evidence**:

```json
{
  "identifier": "opener:allow-open-url",
  "allow": [{ "url": "https://**" }, { "url": "http://**" }]
}
```

`MarkdownRenderer.svelte` forwards `http://` and `https://` links from rendered content into `openExternalUrl(...)`.

**Fix Effort**: Trivial

**Fix**:
Restrict the capability to `https://**` by default. If HTTP support is still needed for edge cases, require an explicit warning/confirmation dialog before opening it.

**Verify**:

- `https://` links still open normally.
- `http://` links are blocked or require a warning-confirmed opt-in path.

### F-04: macOS Terminal Automation Interpolates tmux Session Names Into AppleScript

**Severity**: LOW

**Location**: `src-tauri/src/commands/command_center/navigation.rs:76`, `src-tauri/src/terminal.rs:423`

**Reachability**: Session navigation/open-terminal action -> `navigate_to_session` -> `handle_terminal(EnsureOpen)` -> `MacEmulator::launch_with_tmux()` -> `osascript -e <formatted script>`.

**Description**:
On macOS, Taurhaus builds AppleScript source with `tmux_session` inserted directly into string literals. Taurhaus-launched sessions use the fixed `taurhaus` session name, but the scanner also surfaces existing tmux-backed tool sessions. A crafted local tmux session name containing AppleScript-breaking characters could therefore turn a user click into script injection.

**Evidence**:

```applescript
write text "tmux attach-session -t {tmux_session}"
do script "tmux attach-session -t {tmux_session}"
```

The session name is passed through from navigation into `TerminalIntent::EnsureOpen` without AppleScript-specific escaping.

**Fix Effort**: Moderate

**Fix**:
Escape `tmux_session` for AppleScript string context before interpolation, or stop constructing AppleScript source via `format!` and pass the target through a safer helper boundary.

**Verify**:

- Add a unit/integration test for tmux session names containing quotes and control characters.
- On macOS, navigation to such names must not alter script structure or execute unintended commands.

## Needs Validation / Audit Noise

### V-01: `osv-scanner` surfaced dependency advisories that did not clear the reachability gate

`osv-scanner` reported 21 Rust advisory entries from `src-tauri/Cargo.lock`, mostly GTK 0.18 stack crates inherited transitively by the Linux desktop stack plus one direct `git2` advisory:

- `git2 0.19.0` -> `RUSTSEC-2026-0008` / `GHSA-j39j-6gw9-jw6h`
- multiple GTK/GDK/ATK crates with no fixed version listed

This did **not** become a confirmed finding in this report for two reasons:

1. `cargo audit` was clean in the same tree.
2. I did not establish an end-to-end exploit path in Taurhaus today beyond generic lockfile presence.

Action: track `git2` upgrade planning separately because Taurhaus depends on `git2` directly, but do not treat the current `osv-scanner` output as a confirmed runtime vulnerability without further triage.

### V-02: Daemon auth token creation uses write-then-chmod on Unix

`src-tauri/src/daemon/auth.rs:23` writes the token with `fs::write(...)` and only then applies `0600`. On a broadly traversable app-data directory this could create a brief local-read race. On this host, the current parent directory permissions (`~/.local/share` at `700`) materially reduce exposure, so I did not treat it as a confirmed finding for the shipped default posture.

Action: if Taurhaus is expected to run on multi-user systems with looser home/app-data permissions, tighten token creation to use an atomic `0600` create path.

## Previously Reported Issues Rechecked

- `coordination::validation::validate_team_name("..")` is now fixed. `cargo test team_name_rejects_parent_component` passed.
- Search incremental indexing now rejects symlink escapes outside the project root. `cargo test update_file_rejects_symlink_target_outside_project` passed.
- `cargo deny check` is now functional and completed successfully.

## Scan Results

| Tool | Result | Notes |
| --- | --- | --- |
| `cargo audit` | Clean | No RustSec vulnerabilities reported |
| `cargo deny check` | Clean with warnings | Duplicate crate versions only; policy executed successfully |
| `cargo clippy -- -W clippy::all` | Clean | No clippy findings surfaced |
| `rg 'unsafe '` + crate policy check | 1 scoped unsafe block | `#![deny(unsafe_code)]`; only the libgit2 owner-validation bypass remains |
| `cargo geiger --forbid-only` | Tool failure | Parser/panic errors in dependency tree; replaced with manual unsafe review |
| `gitleaks detect --source .` | Clean | No secrets detected |
| `bun audit` | Clean | No JS vulnerabilities reported |
| `osv-scanner scan source -r .` | 21 advisory entries | Treated as triage noise / dependency follow-up, not confirmed findings |

## Validation Performed

- Reviewed `AGENTS.md` and `ARCHITECTURE.md` first, per task instructions.
- Mapped command registration from `src-tauri/src/lib.rs` and frontend IPC wrappers under `src/lib/ipc/`.
- Inspected daemon auth/protocol, file reading, search indexing, terminal automation, logging, settings, and markdown/open-url paths manually.
- Ran:
  - `cargo deny check`
  - `cargo clippy -- -W clippy::all`
  - `cargo audit`
  - `gitleaks detect --source .`
  - `bun audit`
  - `osv-scanner scan source -r .`
- Ran targeted tests:
  - `cargo test build_claude_`
  - `cargo test update_file_rejects_symlink_target_outside_project`
  - `cargo test team_name_rejects_parent_component`

## Recommended Remediation Order

1. Fix F-01 first. It is default-on, user-facing, and materially shortens the path from untrusted project content to high-impact agent actions.
2. Narrow F-02 next. The current workaround is broader than the WSL problem it was added to solve.
3. Close F-03 and F-04 as small hardening changes in the same sprint. Both are localized and inexpensive.
4. Track V-01 and V-02 as follow-up triage, not immediate remediation commitments.
