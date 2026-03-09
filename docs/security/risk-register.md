# Taurhaus Security Risk Register

Last updated: 2026-03-09

## Current Posture Notes

Recent releases materially improved daemon and compaction posture:

| Change | Status | Security / integrity effect |
|---|---|---|
| Mesh daemon singleton locking (`create_new` lifetime-held lock files) | Shipped in mesh `0.2.4` / taurhaus `0.5.5` | Mitigates duplicate-daemon races and forged stale-pid startup collisions. |
| Stale daemon binary detection with automatic restart on mismatch | Shipped in taurhaus `0.5.7` | Reduces version-drift risk where an old daemon binary keeps running after an app upgrade. |
| Event-driven compaction pipeline (`extractor -> watcher -> processor`) | Shipped in taurhaus `0.5.7` | Removes the redundant `500ms` scan loop, reduces duplicated observation paths, and centralizes compaction handling behind a stricter signal pipeline. |
| Shared runtime-session cache | Shipped in taurhaus `0.5.7` | Replaces duplicate display/compaction scans with one canonical runtime source, reducing state divergence risk. |

These are posture improvements, not new open findings.

## Open Findings

| ID | Severity | Risk | Current Status | Planned Mitigation |
|---|---|---|---|---|
| F-01 | HIGH | Insecure-by-default autonomous launch flags (`--dangerously-skip-permissions`, `--yolo`) increase prompt-injection blast radius. | Open | Change defaults to safe/interactive launch commands and require explicit opt-in for dangerous flags with user warning. |
| F-02 | MEDIUM | libgit2 owner validation is globally disabled, broadening trust to dubious-ownership repositories. | Open | Re-enable owner validation by default and add explicit trust/allowlist for known path classes as needed. |
| F-03 | LOW | External URL opener capability allows `http://**`, enabling insecure transport for untrusted markdown links. | Open | Restrict default allowlist to HTTPS and/or require explicit warning/confirmation for HTTP links. |
| F-04 | MEDIUM | Post-compaction reinjection and audit artifacts persist task metadata, member identities, and operational context on disk. This is necessary for recovery and auditability, but it broadens local metadata exposure if the host or app-data directory is compromised. | Open | Keep reinjection payloads strictly operational and secret-free, document retention expectations, and add redaction/visibility rules before expanding the card contents further. |
| F-05 | MEDIUM | Codex compaction handling depends on undocumented session JSONL semantics. Upstream format drift could cause false-positive delivery, missed reinjection, or ambiguous state transitions if parsing is too permissive. | Open | Keep parser rules strict, preserve paired-record normalization, fail closed on ambiguity, and monitor watcher/extractor diagnostics for schema drift. |

## Accepted Risks

| ID | Risk | Decision | Rationale | Revisit Trigger |
|---|---|---|---|---|
| RR-001 | AI prompt-injection could exfiltrate API keys in permissive tmux-based workflows | Accepted (by design) | Target users are power users already running CLI agents in tmux with permissive flags. This is a baseline workflow risk, not a taurhaus-specific risk class. Added complexity to mitigate this now is not justified for current audience. | Revisit if audience expands beyond tmux-first power users, if non-permissive modes become a product default, or if credential-scoping architecture is introduced. |

## Notes

- Accepted risk does not mean ignored forever; it is explicitly tracked with a revisit trigger.
- The current daemon/compaction posture is materially better than the March 4, 2026 snapshot, but the remaining open risks are now more about blast radius, local metadata exposure, and parser integrity than about polling-loop architecture.
- Current implementation priority remains low-complexity, high-signal fixes first.
