# Taurhaus Security Risk Register

Last updated: 2026-03-03

## Accepted Risks

| ID | Risk | Decision | Rationale | Revisit Trigger |
|---|---|---|---|---|
| RR-001 | AI prompt-injection could exfiltrate API keys in permissive tmux-based workflows | Accepted (by design) | Target users are power users already running CLI agents in tmux with permissive flags. This is a baseline workflow risk, not a taurhaus-specific risk class. Added complexity to mitigate this now is not justified for current audience. | Revisit if audience expands beyond tmux-first power users, if non-permissive modes become a product default, or if credential-scoping architecture is introduced. |

## Notes

- Accepted risk does **not** mean ignored forever; it is explicitly tracked with a trigger.
- Current implementation priority is low-complexity/high-signal fixes first (filesystem boundaries, validation hardening, dependency policy gating).
