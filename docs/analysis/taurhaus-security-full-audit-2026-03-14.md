# Security Audit Report

**Project**: taurhaus
**Audit Type**: Deep Audit
**Date**: 2026-03-14
**Auditor**: security-auditor
**Scope**: `src-tauri/src/startup/`, `src-tauri/src/daemon/`, `src-tauri/src/provider/`, `src-tauri/src/commands/`, `src-tauri/src/coordination/`, `src-tauri/src/session_scanner/`, frontend IPC/rendering paths under `src/lib/`, `.github/workflows/quality-gate.yml`, dependency manifests and audit config
**Status**: Final

---

## Executive Summary

I audited Taurhaus end to end as a Tauri desktop application with special focus on startup, daemon transport, provider/path enforcement, command execution, coordination flows, frontend rendering, CI, and dependency posture. I confirmed three actionable findings: one HIGH application-level issue, one MEDIUM CI supply-chain hardening gap, and one LOW direct-dependency risk that is currently being suppressed in `cargo audit`.

The strongest issue is Taurhaus launching Claude, Codex, and Gemini with unsafe no-approval flags by default. The remaining automated findings were triaged: the GTK/Tauri Linux stack advisories match known false-positive patterns for this ecosystem, and the npm advisories land in dev/test tooling paths rather than the shipped desktop runtime.

## Summary Table

| ID | Severity | Title | Status |
|----|----------|-------|--------|
| F-01 | HIGH | CLI sessions launch AI agents with unsafe no-approval flags by default | Open |
| F-02 | MEDIUM | GitHub Actions workflow uses unpinned actions with default token permissions | Open |
| F-03 | LOW | Direct `git2` advisory is suppressed while Taurhaus parses user-chosen repositories | Open |

## Findings

### F-01: CLI sessions launch AI agents with unsafe no-approval flags by default

**Severity**: HIGH

**Location**: `src-tauri/src/models/mod.rs:232`

**Reachability**: User launches a CLI session from the Taurhaus UI -> `launch_cli_session_impl()` loads terminal settings -> `resolve_configured_tool_command()` / `build_launch_command()` selects the default command -> Taurhaus launches the tool in tmux locally or via the daemon. This is a normal user path today for fresh, continue, and resume launches.

**Description**: Taurhaus ships default launch commands that disable agent permission gates for all supported tools: Claude uses `--dangerously-skip-permissions`, while Codex and Gemini use `--yolo`. That turns Taurhaus into a one-click launcher for fully trusted agent execution inside the project working directory. If a repository prompt file, task text, or other model-visible content is malicious, the agent can immediately perform destructive filesystem or command actions without the user ever seeing the tool's normal approval flow.

**Evidence**:
```text
src-tauri/src/models/mod.rs
236 continue_cmd: "claude --dangerously-skip-permissions --continue"
237 fresh: "claude --dangerously-skip-permissions"
241 continue_cmd: "codex --yolo"
242 fresh: "codex --yolo"
246 continue_cmd: "gemini --yolo --resume"
247 fresh: "gemini --yolo"

src/lib/Settings.svelte
137 continue_cmd: 'claude --dangerously-skip-permissions --continue'
142 continue_cmd: 'codex --yolo'
147 continue_cmd: 'gemini --yolo --resume'

src-tauri/src/commands/command_center/launching.rs
93  let ts = load_terminal_settings(db);
94  let tool_cmd = crate::session_scanner::control::resolve_configured_tool_command(...)
107 command_override: Some(tool_cmd),
188 let (session, window, pane) = crate::session_scanner::control::launch_in_tmux_with_layout(...)

Validation:
- cargo test build_claude_fresh_command --lib -> ok
- cargo test build_codex_fresh_command --lib -> ok
- cargo test build_gemini_fresh_command --lib -> ok
- cargo test terminal_settings_default_includes_cli_commands --lib -> ok
```

**Fix Effort**: Moderate

**Fix**:
```text
Change the shipped defaults to the tools' safe/approval-preserving modes.

- Remove `--dangerously-skip-permissions` from Claude defaults.
- Remove `--yolo` from Codex and Gemini defaults.
- If operators still need these modes, gate them behind an explicit settings opt-in
  with a clear warning instead of making them the default.
- Migrate existing persisted settings carefully so legacy installs do not silently
  keep unsafe defaults forever.
```

**Verify**: Update unit tests so default command assertions no longer include unsafe flags. Launch each tool from a clean settings profile and confirm the spawned process arguments omit `--dangerously-skip-permissions` / `--yolo`.

---

### F-02: GitHub Actions workflow uses unpinned actions with default token permissions

**Severity**: MEDIUM

**Location**: `.github/workflows/quality-gate.yml:15`

**Reachability**: Any CI run on `pull_request`, `push`, or `workflow_dispatch` executes this workflow. A compromised moving action tag or over-broad default `GITHUB_TOKEN` on the runner can affect the repository's trusted build environment.

**Description**: The Quality Gate workflow uses mutable action tags instead of commit SHAs, does not declare explicit token permissions, and leaves `actions/checkout` credential persistence enabled. This is a classic GitHub Actions supply-chain hardening gap: the workflow trusts externally maintained action refs at execution time and inherits broader token behavior than necessary.

**Evidence**:
```text
.github/workflows/quality-gate.yml
20 uses: actions/checkout@v4
39 uses: oven-sh/setup-bun@v2
44 uses: dtolnay/rust-toolchain@stable
47 uses: taiki-e/install-action@just
50 uses: Swatinem/rust-cache@v2

No top-level or job-level `permissions:` block is present.
`actions/checkout` does not set `persist-credentials: false`.

zizmor .github/workflows/quality-gate.yml
- warning[artipacked]: actions/checkout@v4 does not set persist-credentials: false
- warning[excessive-permissions]: default permissions used due to no permissions: block
- error[unpinned-uses]: actions/checkout@v4
- error[unpinned-uses]: oven-sh/setup-bun@v2
- error[unpinned-uses]: dtolnay/rust-toolchain@stable
- error[unpinned-uses]: taiki-e/install-action@just
- error[unpinned-uses]: Swatinem/rust-cache@v2
```

**Fix Effort**: Trivial

**Fix**:
```yaml
permissions:
  contents: read

jobs:
  quality-gate:
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@<full-sha>
        with:
          persist-credentials: false
      - uses: oven-sh/setup-bun@<full-sha>
      - uses: dtolnay/rust-toolchain@<full-sha>
      - uses: taiki-e/install-action@<full-sha>
      - uses: Swatinem/rust-cache@<full-sha>
```

**Verify**: Re-run `zizmor .github/workflows/quality-gate.yml` and `actionlint .github/workflows/quality-gate.yml`. `zizmor` should no longer report `unpinned-uses`, `excessive-permissions`, or `artipacked`.

---

### F-03: Direct `git2` advisory is suppressed while Taurhaus parses user-chosen repositories

**Severity**: LOW

**Location**: `src-tauri/.cargo/audit.toml:4`

**Reachability**: User registers or selects a project -> Taurhaus opens and walks that repository through `git2` for status, commit history, and remote inspection -> vulnerable `git2` code processes attacker-controlled repository data. This is part of normal project browsing and indexing behavior.

**Description**: Taurhaus depends directly on `git2 = 0.19` and suppresses `RUSTSEC-2026-0008` in `cargo audit` with the rationale `read-only usage, low risk`. That rationale is too weak for a desktop application that directly opens user-selected repositories on a large git surface. Even though the advisory is low severity and no exploit chain was reproduced here, the current suppression hides a real known issue on an exposed parsing path and prevents `cargo audit` from signaling when the project is ready to upgrade.

**Evidence**:
```text
src-tauri/Cargo.toml
42 git2 = { version = "0.19", features = ["vendored-openssl"] }

src-tauri/.cargo/audit.toml
9  # git2 0.19.0 unsound Buf deref — read-only usage, low risk
10 "RUSTSEC-2026-0008",

src-tauri/src/git/commits.rs
46 fn open_git_repo(repo_path: &Path) -> Result<Repository, AppError> {
47     Repository::open(repo_path)...

src-tauri/src/git/status.rs
10 let repo = Repository::open(repo_path)...

src-tauri/src/commands/git.rs
195 let repo = git2::Repository::open(&path)...

osv-scanner scan source -r .
- RUSTSEC-2026-0008 / GHSA-j39j-6gw9-jw6h
- package: git2 0.19.0
- fixed version: 0.20.4
```

**Fix Effort**: Moderate

**Fix**:
```text
Upgrade the direct `git2` dependency to a fixed release (0.20.4 or newer),
rebuild the lockfile, and remove the `RUSTSEC-2026-0008` ignore entry from
`src-tauri/.cargo/audit.toml`.

If an immediate upgrade is blocked, document the exact affected API surface and
why Taurhaus cannot reach it today. The current "read-only usage" note is not
sufficient because Taurhaus still opens and traverses untrusted repository data.
```

**Verify**: Run `cargo update -p git2`, then `cargo audit --json` and confirm `RUSTSEC-2026-0008` is no longer present or ignored. Re-run the git-related tests and smoke-test project registration, git status, commit history, and remote URL lookup.

---

## Scan Results

| Tool | Findings | Fixed | Deferred | Clean |
|------|----------|-------|----------|-------|
| `cargo audit --json` | 0 active vulns, but 1 direct advisory (`RUSTSEC-2026-0008`) is suppressed in config | 0 | 1 | No |
| `cargo deny check` | duplicates/license warnings only | 0 | duplicate-version warnings | Yes |
| `cargo clippy --all-targets -- -W clippy::all` | no captured lint findings | 0 | 0 | Yes |
| `gitleaks detect --source .` | 0 | 0 | 0 | Yes |
| `osv-scanner scan source -r .` | 35 advisories total | 0 | GTK/Tauri stack noise and dev-tooling npm issues triaged out; `git2` reported above | No |
| `bun audit` | 14 advisories | 0 | all findings triaged to dev/test/tooling paths (`@wdio`, `jsdom`, Svelte compiler path) | No |
| `zizmor .github/workflows/quality-gate.yml` | 7 actionable workflow hardening findings | 0 | reported as F-02 | No |
| `actionlint .github/workflows/quality-gate.yml` | 0 | 0 | 0 | Yes |

## Triage Notes

- The large GTK / GDK / ATK / `glib` batch from `osv-scanner` matches the known Tauri-on-Linux false-positive pattern already documented in Taursec. `cargo audit` remains clean for the same tree, and Taurhaus does not directly control those transitive GTK bindings.
- `bun audit` / `osv-scanner` npm findings land in dev or test toolchains:
  - `undici` is pulled by `jsdom` and WebdriverIO paths
  - `yauzl` is pulled by `@wdio/*` browser tooling
  - `devalue` is pulled through Svelte compiler/runtime packaging, not through a Taurhaus-owned direct runtime feature
- Daemon auth, WSL path translation, local file reads, and markdown link opening showed appropriate boundary checks during this audit and did not produce additional reportable findings.

## Recommendations

1. Change Taurhaus to safe-by-default agent launch commands and require an explicit opt-in for no-approval modes.
2. Pin every GitHub Action by full SHA, declare explicit `permissions`, and disable persisted checkout credentials.
3. Upgrade `git2`, remove the advisory suppression, and keep `cargo audit` honest for direct dependencies.
4. Keep treating OSV GTK findings and npm dev-tooling advisories as triage-required rather than auto-reportable, but document that reasoning in future audits as done here.

## Severity Definitions

- **CRITICAL** — Data breach, auth bypass, RCE. Stop everything, fix immediately
- **HIGH** — Privilege escalation, IDOR, crypto weakness. Fix this sprint
- **MEDIUM** — Missing hardening, info leak. Fix next sprint
- **LOW** — Best practice gap. Fix opportunistically
- **INFO** — Awareness note. No action required
