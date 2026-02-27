# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in taurhaus, please report it responsibly.

**Do not** open a public GitHub issue for security vulnerabilities.

Instead, please email: **stierms@gmail.com**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if you have one)

## Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 1 week
- **Fix or mitigation**: Depends on severity, but we aim for critical fixes within 2 weeks

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest  | Yes       |
| Older   | No        |

Only the latest release receives security updates.

## Scope

The following are in scope for security reports:

- Command injection via IPC commands or daemon protocol
- Path traversal in file operations
- Unauthorized access to project data
- WSL distro parameter injection
- Daemon protocol vulnerabilities (TCP localhost:9000)

The following are generally out of scope:

- Vulnerabilities in upstream dependencies (report to the upstream project)
- Denial of service on the local machine (taurhaus is a local desktop app)
- Issues requiring physical access to the machine

## Known Advisories

See [docs/known-advisories.md](docs/known-advisories.md) for documented and accepted dependency advisories.
