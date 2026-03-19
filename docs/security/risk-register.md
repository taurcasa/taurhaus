# Taurhaus Security Risk Register

Last updated: 2026-03-19

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
| F-01 | HIGH | Insecure-by-default autonomous launch flags (`--dangerously-skip-permissions`, `--yolo`) increase prompt-injection blast radius. | Accepted risk for current quality phase | No behavior change in this phase; keep explicitly documented and revisit if product direction changes. |
| F-02 | MEDIUM | API keys remain visible within the shared `taurhaus` tmux session boundary, so one pane can read credentials available to the session. | Accepted risk for current quality phase | No mitigation task in this phase; keep explicitly documented and revisit if credential-isolation architecture becomes a goal. |
| F-03 | LOW | Search index path still ships with vulnerable `lz4_flex` via `tantivy`. | Open (cheap-only) | Attempt remediation only if it stays a clean dependency bump with low integration risk. |
| F-04 | MEDIUM | Post-compaction reinjection and audit artifacts persist task metadata, member identities, and operational context on disk. This is necessary for recovery and auditability, but it broadens local metadata exposure if the host or app-data directory is compromised. | Open | Keep reinjection payloads strictly operational and secret-free, document retention expectations, and add redaction/visibility rules before expanding the card contents further. |
| F-05 | MEDIUM | Codex compaction handling depends on undocumented session JSONL semantics. Upstream format drift could cause false-positive delivery, missed reinjection, or ambiguous state transitions if parsing is too permissive. | Open | Keep parser rules strict, preserve paired-record normalization, fail closed on ambiguity, and monitor watcher/extractor diagnostics for schema drift. |

## 2026-03-19 Quality Phase Decisions

These decisions are locked for the current quality phase and come from the 2026-03-19 security audit plus user triage:

| Finding | Decision | Execution impact | Revisit trigger |
|---|---|---|---|
| `F-01` unsafe launch flags | Accepted risk for this phase | Do not change launch behavior in this quality phase. Keep the risk explicit in planning and release notes. | Revisit if the target audience broadens beyond tmux-first power users, if approval-preserving defaults become product direction, or if credential/isolation architecture changes. |
| `F-02` tmux session API key exposure | Accepted risk for this phase | Do not schedule a mitigation task in this quality phase. Keep the risk explicit in planning and release notes. | Revisit if Taurhaus adopts stronger per-pane credential isolation, shared-session assumptions change, or the product direction shifts away from current power-user workflows. |
| `F-03` `tantivy` / `lz4_flex` dependency issue | Cheap-only remediation gate | Attempt remediation only if it stays a clean dependency bump with low integration risk. Skip if it expands into a broader compatibility or stabilization task. | Revisit if a clean bump becomes available later or if exploitability/impact changes materially. |

## Accepted Risks

| ID | Risk | Decision | Rationale | Revisit Trigger |
|---|---|---|---|---|
| RR-001 | AI prompt-injection could exfiltrate API keys in permissive tmux-based workflows | Accepted (by design) | Target users are power users already running CLI agents in tmux with permissive flags. This is a baseline workflow risk, not a taurhaus-specific risk class. Added complexity to mitigate this now is not justified for current audience. | Revisit if audience expands beyond tmux-first power users, if non-permissive modes become a product default, or if credential-scoping architecture is introduced. |
| RR-002 | Default unsafe launch flags remain enabled in Taurhaus (`F-01`) | Accepted for current quality phase | The current product direction explicitly prioritizes the power-user workflow these flags support. The team chose documentation and explicit tracking instead of behavior change in this phase. | Revisit if product direction changes toward safer defaults or mixed-experience users. |
| RR-003 | Shared `taurhaus` tmux session remains a credential-sharing boundary (`F-02`) | Accepted for current quality phase | The current architecture and user workflow treat the shared tmux session as an accepted trust boundary. The team chose not to take on the complexity of secret-isolation redesign in this phase. | Revisit if per-pane/per-process credential isolation becomes a product goal. |

## Notes

- Accepted risk does not mean ignored forever; it is explicitly tracked with a revisit trigger.
- The 2026-03-19 quality phase explicitly accepted `F-01` and `F-02` for this phase and gated `F-03` as cheap-only. Those are execution decisions, not proof that the underlying risks disappeared.
- The current daemon/compaction posture is materially better than the March 4, 2026 snapshot, but the remaining open risks are now more about blast radius, local metadata exposure, and parser integrity than about polling-loop architecture.
- Current implementation priority remains low-complexity, high-signal fixes first.
