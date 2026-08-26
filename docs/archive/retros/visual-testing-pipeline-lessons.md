# Visual Testing Pipeline — Lessons Learned

> Archived and stale: the visual lane now has 10 spec files, not 5. Superseded by [`docs/operations/visual-testing-guide.md`](../../operations/visual-testing-guide.md).

Date: 2026-03-06
Tasks: #404–#410

## Summary

7-task visual testing pipeline completed in one session. 32 browser-mode screenshot tests across 5 component specs (MeshCanvas, HoverCard, MeshNodeDetail, Sidebar, smoke), plus a Vite fixture host and testing guide. All green in 7.6s.

## Finding 1: Mesh send reactivation bug (critical)

All Codex agents were unable to send messages via `mesh send` for the entire session. They could receive messages and update tasks, but outbound communication was broken.

Root cause: `mesh join` on an existing member after a daemon restart only logged "updating presence" but never set `isActive=true`. The `mesh send` command gates on active membership, so sends were rejected with a misleading "agent not found (no inbox)" error even though inbox files existed.

Impact: Agents couldn't ask questions, report blockers, or communicate back. This caused multiple stalls where agents appeared confused or unresponsive — they were actually working but muted.

Fix: mesh 0.2.1 — added config-level member reactivation on rejoin. Regression test: `rejoin_reactivates_member_and_allows_send`.

Prevention: The regression test covers this path permanently. Future daemon restarts should be followed by a send verification from each agent.

## Finding 2: Codex message framing matters

Codex agents treat reading explanatory messages as completing the task. Context-heavy messages with background information cause agents to summarize the message internally and stop.

What doesn't work:
- "You're assigned task #X — here's the context, the background, the deliverables..."
- "ACTION REQUIRED:" (still too ambiguous about who)
- Broadcast FYI messages (stall every agent simultaneously)

What works:
- Lead with "YOU MUST:" + concrete file operation as first sentence
- Keep messages under 5 sentences
- End with completion criteria ("When done, mark #NNN completed")
- Never send messages that invite a pure ack response

## Pipeline execution stats

- Phase 1 (infra + pilot): developer2, serial, fast
- Phase 2 (component specs): developer1 + developer2 + developer3, parallel
- Phase 3 (fixture host): developer2, parallel with Phase 2
- Phase 4 (docs): architect, after Phase 2

developer2 completed 4 of 7 tasks. developer1 completed 2. developer3 completed 1 (hampered by send bug). architect completed 1.

## Action items for next session

1. Verify `mesh --version` shows 0.2.1+ before starting work
2. Test agent send capability early in session with a probe message
3. Use "YOU MUST:" prefix for all Codex task assignments
4. Defer formal retro survey to a session with working bidirectional comms
