# Security Policy

## Security model

taurhaus is a local desktop application that manages AI CLI tool sessions and project metadata. It has access to the local filesystem, git repositories, running processes, and a TCP connection to its companion daemon.

### Trust boundaries

```
┌─────────────────────────────────────────────────────┐
│  Frontend (WebView)                                 │
│  - Renders project data, git diffs, search results  │
│  - No direct filesystem access                      │
│  - All data flows through IPC                       │
├─────────────────── IPC ────────────────────────────┤
│  Backend (Rust / Tauri)                             │
│  - Validates all inputs server-side                 │
│  - SQLite, tantivy, libgit2                         │
│  - Platform abstraction for process inspection      │
├─────────────────── TCP ────────────────────────────┤
│  Daemon (127.0.0.1:17233 by default)                │
│  - Session scanning, tmux, Windows/WSL watch bridge │
│  - Token-authenticated, localhost-only              │
├────────────────────────────────────────────────────┤
│  tmux / CLI tools (Claude, Codex, Antigravity, Grok)│
│  - User-controlled, permissive flags                │
│  - taurhaus launches but doesn't sandbox them       │
└─────────────────────────────────────────────────────┘
```

### Security boundaries

**IPC (frontend ↔ backend)**:
- All commands validated server-side — the frontend is untrusted
- Path traversal protection on file operations
- Project paths verified against registered projects

**Daemon (app ↔ daemon TCP)**:
- Localhost-only binding (no network exposure)
- The default endpoint is `127.0.0.1:17233`, defined by [`daemon::server::DEFAULT_PORT`](src-tauri/src/daemon/server.rs); the daemon CLI can override the port
- Token authentication: daemon generates a 32-byte random token on startup, written to a file with `0600` permissions. The app reads this token and includes it in every request.
- WSL distro names validated against a safe character set (alphanumeric, hyphens, underscores, dots)
- Protocol version checking on connect

**Search index**:
- Symlink escape protection in incremental indexing — prevents indexed content from escaping project boundaries via symlinks

**Frontend rendering**:
- DOMPurify sanitization for all rendered markdown
- Console log sanitization to prevent log-forging attacks

**File access**:
- .gitignore-filtered file watching — only watches relevant project files
- Directory listing uses lazy expansion (no recursive scan of protected macOS TCC directories)

### Known risks

Accepted risks are tracked in the [risk register](docs/security/risk-register.md).

**RR-001**: AI prompt injection could exfiltrate API keys in permissive tmux-based workflows. Accepted by design — target users are power users already running CLI agents with permissive flags. This is a baseline workflow risk, not a taurhaus-specific vulnerability. See risk register for revisit triggers.

### Dependency auditing

The repository includes `cargo-deny` supply-chain policy and a `cargo audit` configuration. Known advisories for transitive dependencies (primarily from Tauri's GTK3 stack) are documented with rationale in `src-tauri/.cargo/audit.toml`.

Direct dependencies are chosen conservatively:
- `rusqlite` with bundled SQLite (no system library dependency)
- `git2` with vendored OpenSSL
- `tantivy` for search (Rust-native, no C bindings)

### Audit history

| Date | Scope | Findings | Report |
|------|-------|----------|--------|
| 2026-02-27 | Full audit v0.3.2 | 0 Critical, 0 High, 2 Medium, 8 Low | [Report](docs/archive/security/audit-2026-02-27.md) |
| 2026-03-03 | Directed (IPC, files, search) | 1 Medium, 2 Low | [Report](docs/archive/security/sec-auditor-audit-2026-03-03.md) |
| 2026-03-03 | TaurSec framework audit | 0 Critical, 1 High, 4 Medium, 4 Low | [Report](docs/archive/security/team-lead-audit-2026-03-03.md) |

## Reporting a Vulnerability

If you discover a security vulnerability in taurhaus, please report it responsibly.

**Do not** open a public GitHub issue for security vulnerabilities.

Instead, please email: **dev@taurcasa.dev**

taurhaus is experimental and is not currently under active development. Reports are welcome, but monitoring, response, remediation, and release timelines cannot be guaranteed.

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if you have one)

## Supported Versions

No release currently receives guaranteed security updates. Existing releases remain available as experimental artifacts, not as supported production software. If development resumes, this section should be revised before making any support commitment.

## Scope

The following are in scope for security reports:

- Command injection via IPC commands or daemon protocol
- Path traversal in file operations
- Unauthorized access to project data
- WSL distro parameter injection
- Vulnerabilities in the authenticated [localhost daemon protocol](docs/architecture/daemon-protocol.md)

The following are generally out of scope:

- Vulnerabilities in upstream dependencies (report to the upstream project)
- Denial of service on the local machine (taurhaus is a local desktop app)
- Issues requiring physical access to the machine

## Known Advisories

Accepted dependency advisories are documented in `src-tauri/.cargo/audit.toml`.
