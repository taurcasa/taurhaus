# Accounts UX overhaul — functional brief

Commissioned by the operator (2026-09-02): account selection is scattered across the settings base command, the sidebar's silent last-used resolution, and the overview's shift-click chooser; usage has no central home; adding an account has no flow at all; and managed Claude teams offer no account choice while the mesh view shows only negative signals. This brief fixes the *what*; the visual and interaction design comes from a design panel with creative freedom, and the operator approves the direction before implementation.

Grounded by the account-selection research pass (managed-launch pin trace, breakage table for a moved `CLAUDE_CONFIG_DIR`, reusable app-launch shapes). Key facts the design must respect:

- The scatter is three mechanisms from three eras: base-command aliases (pre-accounts; the operator's `claude2` alias exists because first-class selection was missing), the detection/pin/last-used resolution core (good machinery, invisible), and the launch-time usage-aware chooser (only some surfaces can summon it).
- A managed **Claude** member cannot run on a different account than its team's root: Claude Code derives teams polling, task lists, hooks, and transcripts from one `CLAUDE_CONFIG_DIR`. Mixed-account Claude teams are impossible today (upstream feature request noted); a whole team CAN move roots only if the teams-dir authority is threaded per-team through mesh `--claude-dir`, the daemon passes, hook install, and the task scanner.
- **Codex and Grok members are mesh-bridged**: their selector env moves the account without moving any team state — per-member accounts for them are safe plumbing-wise today.
- The app-launch machinery is reusable as-is: `resolve_launch_account` with `AccountOrigin` (says *why*), the preview IPC (says *which account would launch* before launching), `project_tool_accounts` persistence, the chooser with usage meters, and `LaunchAccountResult` honesty for opaque wrappers.

## Functional requirements

**FR1 — One home: an Accounts surface.** Every detected account across every tool in one place: identity (email/label), usage windows, health (signed out, exhausted, unauthorized), which is the global default, which projects pin it, and which teams run on it. This is where defaults and pins are managed — settings stops being where account behavior hides.

**FR2 — Central usage.** All accounts' usage meters visible together (the poller already polls every detected account of every tool); compact usage stays available where launches happen, but the full picture has one address.

**FR3 — Add-account flow.** A guided, registry-driven flow per selector-capable tool: create the sibling config dir, open a managed terminal running the tool's login inside it, and let detection pick it up. Adding a Claude, Codex, or future tool's account must be the same gesture. No credential handling beyond what the CLIs themselves do (tokens stay read-at-request, never stored by taurhaus).

**FR4 — Choice offered, never forced, everywhere a session starts or resumes.** One shared picker component consumed by every launch surface (sidebar, overview, command center, resume paths, team builder). Every surface can show — before launching — which account *will* be used and why (the preview IPC + `AccountOrigin`). The settled-launch contract stays: interrupt only on exhausted/unauthorized usage; explicit choice always one gesture away, mandatory never.

**FR5 — Managed teams get account truth and account choice.**
- *Codex/Grok members*: per-member account selection (builder roster row beside the existing `ModelSelect`; stored on the member record like `model` is; rendered through the existing selector machinery).
- *Claude teams*: per-**team** account — the whole team on one chosen root, with the per-team teams-dir authority threaded through mesh `--claude-dir`, the daemon, the compact-hook installer, and the task scanner. Core scope, not stretch: it is the mechanism FR5b rides on. Per-member mixed-account Claude teams stay out of scope (CC constraint, documented). Until the slice lands, the UI tells the truth about which account a team runs on, replacing today's one-line disclaimer.
- *Mesh view*: positive account display on member nodes/detail (label + usage state + applied/not-guaranteed), not just the opaque-wrapper warning.

**FR5b — Switch a team to a different account (the mid-wave usage-out journey).** Field reality (operator, 2026-09-02): usage runs out while a team is working; today the operator kills the team, rebuilds it by hand hoping the roles match, and tells the new lead "we are on a different account now" with no continuity guarantees. This becomes a first-class operation:
- *The operation*: stop team → migrate/re-point its state for the new account (Claude: team config, inboxes, runtime, mesh task records move with the per-team root; Codex/Grok: only the member selector env changes — team state never lived in the account dir) → resume team under the new account, through the existing daemon resume pipeline.
- *Continuity story, stated honestly*: transcripts do not cross accounts — every member comes back as a fresh conversation. What DOES survive, because it is account-independent team state, is everything the task ledger holds: task records with rulings, artifacts, and restart cursors, the inboxes, and the operational snapshots. The resume onboarding says so explicitly: which account the team now runs on, which account the previous run used, where that run's transcripts live (path pointer for the lead), and the standing instruction to rebuild working context from `mesh task get` — the switch journey is precisely what the W-B cursor and compaction-card machinery were built to serve.
- *Offered where the pain hits*: when a team member's account is exhausted/unauthorized, the team surface proposes the switch (same interrupt contract as FR4 — proposed, never forced).

**FR6 — Choices are keyed by account id, resolved to a dir at render.** Ids survive dir renames; resolution goes through detection with the existing degraded-detection and `fallback_from` semantics (a vanished or signed-out account falls back loudly, never silently).

**FR7 — The alias becomes an escape hatch, honestly surfaced.** Base-carried selectors keep working for app launches (existing precedence) but stop being the *recommended* way to express account intent. Where a team launch must override a base-carried selector (the Claude inbox contract), the UI says so — the current behavior (silent rewrite + one log line) is exactly the scatter this overhaul removes.

## Non-goals

- Mid-task *silent* account switching of a single live member (FR5b switches at the team resume boundary, with fresh conversations and explicit onboarding — a live conversation never hops accounts).
- Shared history across accounts.
- Any storage of credentials or refresh flows.
- Role templates carrying machine-local account ids as hard requirements (portable templates may carry a *soft* account preference at most).

## Process

1. This brief (approved scope: 0.9.0, operator-confirmed).
2. **Design panel**: independent design concepts for the Accounts surface + picker + team/mesh integration, judged, synthesized; the winning direction goes to the operator for approval before any implementation.
3. Implementation: backend slices (member account field, per-team root authority as the stretch slice) via the standard lanes; frontend via the design-led loop with the visual dual review.
