# Taurhaus Security Risk Register

Last updated: 2026-03-04

## Open Findings (Task #56 Audit)

| ID | Severity | Risk | Current Status | Planned Mitigation |
|---|---|---|---|---|
| F-01 | HIGH | Insecure-by-default autonomous launch flags (`--dangerously-skip-permissions`, `--yolo`) increase prompt-injection blast radius. | Open | Change defaults to safe/interactive launch commands and require explicit opt-in for dangerous flags with user warning. |
| F-02 | MEDIUM | libgit2 owner validation is globally disabled, broadening trust to dubious-ownership repositories. | Open | Re-enable owner validation by default and add explicit trust/allowlist for known path classes as needed. |
| F-03 | LOW | External URL opener capability allows `http://**`, enabling insecure transport for untrusted markdown links. | Open | Restrict default allowlist to HTTPS and/or require explicit warning/confirmation for HTTP links. |

## Accepted Risks

| ID | Risk | Decision | Rationale | Revisit Trigger |
|---|---|---|---|---|
| RR-001 | AI prompt-injection could exfiltrate API keys in permissive tmux-based workflows | Accepted (by design) | Target users are power users already running CLI agents in tmux with permissive flags. This is a baseline workflow risk, not a taurhaus-specific risk class. Added complexity to mitigate this now is not justified for current audience. | Revisit if audience expands beyond tmux-first power users, if non-permissive modes become a product default, or if credential-scoping architecture is introduced. |

## Notes

- Accepted risk does **not** mean ignored forever; it is explicitly tracked with a trigger.
- Current implementation priority is low-complexity/high-signal fixes first (filesystem boundaries, validation hardening, dependency policy gating).
