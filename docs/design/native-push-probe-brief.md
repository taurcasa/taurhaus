# Probe brief: native push channels for Claude Code and Codex, as installed

Commissioned by the operator, 2026-09-07. One Astra lane at high effort.
Deliverable: `docs/design/native-push-probe-report.md` — upgrades the
overhaul study's U-labels to S/verified or NO-GO-for-now for THIS
machine's installed harness versions. Probe, not implementation.

## Why

Operator decision: ~80% of future teams are expected to run Claude Code
+ Codex only. Native push for those two is first-class; agy/grok keep
the tmux fallback. The overhaul study (mesh-messaging-overhaul-
research.md) left the decisive claims unverified for installed
versions. Close exactly those gaps.

## Questions, in priority order

1. **Claude Code channels**: does the installed `claude` support the
   channels surface the docs describe (session flags, the
   `experimental['claude/channel']` capability, `notifications/claude/
   channel`)? Discover from `claude --version`, `--help`, config/docs
   surfaces first. If the surface exists, ONE bounded end-to-end trial:
   a minimal local MCP channel server + a scratch-project session, one
   distinctive benign payload, verify it reaches the model's context in
   that session's own output. If org/preview-gated, report the exact
   gate and stop — do not work around eligibility.
2. **Codex app-server**: does installed Codex (0.153.x) ship the
   app-server surface (`codex app-server` or equivalent)? Handshake it;
   if cheap, ONE `turn/start` on a scratch thread with a trivial prompt
   (smallest available codex model), and check `turn/steer` semantics
   against an active turn if practical. Report acceptance vs
   comprehension separately, per the study's receipt vocabulary.
3. **Codex hooks surface**: which hook events does the installed
   version's config actually accept (beyond the compaction contract we
   already run)? Config-surface verification; a live boundary trial only
   if free.
4. For each verified channel: the minimal adapter shape it implies for
   the study's delivery-service design (what the claim/present/record
   steps map to), in a few sentences each — not a design document.

## Constraints (a nine-seat wave is LIVE)

- NEVER touch: the live daemon (17233), the operator's tmux server,
  ~/.claude/teams, ~/.claude-account2 (ANYTHING under it — it is the
  live wave's root), ~/projects/taurjob, live team state of any kind.
- Claude probing: read-only capability discovery may use the default
  installed setup; any live session trial runs in a fresh scratch
  project directory, at most TWO short turns total, nothing written
  outside scratch. Codex probing: scratch CODEX_HOME per the e2e
  paid-lane pattern if isolation is needed; at most two short turns.
- No credentials read, logged, or copied beyond what the harnesses do
  themselves. If a probe would exceed these bounds, record NO-GO with
  the reason instead.
- Label every claim S (verified here, with version), D (docs only), or
  U — same vocabulary as the overhaul study. Negative results are
  first-class results.
