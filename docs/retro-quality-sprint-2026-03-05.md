# Quality Sprint Retrospective — 2026-03-05

## Sprint Summary

**Duration:** Single session (~3 hours wall-clock)
**Team:** 1 Claude team-lead + 4 Codex agents (architect, developer1, developer2, developer3)
**Tasks completed:** 14 (4 P1, 5 P2, 3 P3, 2 infrastructure)
**Test suite:** 1922 tests green (1022 Rust + 902 frontend)

## Survey Results (4/4 agents responded)

### 1. Communication

**Consensus:** Team-lead communication was effective and fast. Task assignments had low latency. Blockers were resolved quickly when reported.

**Friction points:**
- Too many heartbeat/status pings while agents were working (dev1: "event-driven, not heartbeat")
- Mid-sprint policy changes (check-quick adoption) caused brief churn before stabilizing
- Message volume overhead from repeated "read unread" style pings

**Recommendations from agents:**
- Bundle assignment + process change + verification policy into one message (dev3)
- Pin verification policy in one canonical message, reference in every assignment (dev2)
- Batch small process changes into a single daily broadcast with effective timestamp (architect)
- Fewer heartbeat pings, more event-driven updates: assignment, unblock, priority change (dev1)

### 2. Task Clarity

**Consensus:** Task descriptions were strong when they included file pointers, acceptance criteria, and scope guidance. Actionable without guessing in most cases.

**Gaps identified:**
- Completion criteria ambiguous when global check is red for unrelated reasons (dev1, dev2)
- Scope boundaries unclear in cross-cutting changes — "how far to migrate adjacent surfaces?" (dev3)
- No default rule for handling unrelated test failures encountered during work (dev2)

**Recommendations from agents:**
- Add explicit "in-scope / out-of-scope" line per task (dev3)
- Include default rule for unrelated red tests in task template (dev2)
- Explicit guidance on completion when global gate is red from other agents' work (dev1)

### 3. Quality Gates

**Consensus:** `just check` caused major contention in shared worktree. `just check-quick` was an immediate and material improvement.

**Time lost to check contention (self-reported):**

| Agent | Estimated wait time |
|-------|-------------------|
| architect | 30-50 min |
| developer1 | 60-90 min |
| developer2 | 20-30 min |
| developer3 | 30-45 min |
| **Total** | **~140-215 min** (~2.5-3.5 hours of agent-time) |

**Key issues:**
- Cargo build directory lock contention when multiple agents run checks
- Full test suite (~5 min) serializes across agents = N×5 min wall-clock
- Unrelated formatting/lint failures from other agents' in-progress work
- False-red results undermined confidence in check results

**Mid-sprint fix:** Adopted check-quick-only for agents, team-lead owns serialized full gate. All agents confirmed this was a clear improvement.

### 4. Shared Worktree

**Consensus (4/4):** Universal pain point. All agents experienced builds broken by other agents' uncommitted changes.

**Patterns observed:**
- Template command removals broke compile for agents not touching templates
- IPC contract changes (project_path → project_id) broke tests across multiple files
- Formatting drift accumulated and caused false fmt failures
- Each agent handled it by: scoping tight, running targeted tests, reporting to lead

**No disagreement** — this was the sprint's biggest friction source after quality gate timing.

### 5. Compaction

**Consensus (4/4):** All agents experienced at least one compaction event. All recovered successfully, but with overhead and risk of stalling.

**Recovery strategies used:**
- Reconstructed from task status + local diffs + command output (architect)
- Recovered from saved notes/history, risk of drift (dev1)
- Explicit handoff summary recovery (dev2)
- Task status + rerun verification + resume flow (dev3)

**Universal request:** Auto-generated compacted-context handoff template containing:
- Active task ID and description
- Files changed so far
- Last validation status
- Pending next steps / next command

### 6. Top Improvement (Single Most Impactful Change)

| Agent | Recommendation |
|-------|---------------|
| architect | Sprint-start preflight protocol: canonical verification lane, full-gate owner, out-of-scope failure rules |
| developer1 | Per-agent isolated worktrees/branches |
| developer2 | Per-agent isolated verification lanes or task-scoped CI checks |
| developer3 | Per-agent isolated worktrees/branches with mandatory sync/merge gate |

**3/4 agents independently recommended worktree isolation.** Architect's preflight protocol is complementary (process rules that work regardless of isolation strategy).

---

## Decisions

### D1: Check-quick-only for agents (DECIDED mid-sprint)
- Agents use `just check-quick` exclusively
- Team-lead owns `just check` as serialized gate
- Documented in AGENTS.md and CLAUDE.md

### D2: Task template improvements (DECIDED)
- All tasks include explicit "in-scope / out-of-scope" line
- Default rule for unrelated failures: "If check-quick fails on files you didn't touch, report to team-lead and mark your task complete if your scoped tests pass"
- Completion criteria: check-quick green on your files = done. Full gate is lead's responsibility.

### D3: Communication protocol (DECIDED)
- Event-driven messaging: assignment, unblock, priority change. No heartbeat pings.
- Process changes bundled into one broadcast with effective-immediately flag
- Verification policy referenced in every task assignment message

### D4: Post-compaction idle prevention (DECIDED)
- The problem is NOT lost context — compaction preserves it in compressed form
- The problem IS behavioral: agent compacts → reads messages → sees no new task → assumes idle → stops
- **Fix (taurhaus-side):** AGENTS.md already has the rule (line 128). Reinforce in task assignments.
- **Fix (mesh-side):** Add default instruction to agent onboarding: "If you have no new messages and are unsure of your current task, immediately reach out to team-lead. Do not assume you are done."
- Auto-snapshot before compaction is NOT the answer — that's what compaction already does.

### D5: Worktree isolation (DECIDED — not pursuing)
- 3/4 agents recommended this, but task cycle is too fast (~15-30 min per task)
- Worktree setup/teardown + merge overhead would likely exceed contention savings
- The check-quick-only model already eliminates most contention
- Remaining friction (compile errors from others' changes) better solved by:
  - Faster commit cadence (commit after each task, not batch)
  - Agents ignoring out-of-scope failures (already in AGENTS.md)
- **Revisit if team size grows or tasks become longer-lived**

---

## Action Items

### Taurhaus-side (we can fix now)

| # | Action | Owner | Status |
|---|--------|-------|--------|
| A1 | Update task template with in-scope/out-of-scope + default failure rule | team-lead | TODO |
| A2 | Add completion criteria to AGENTS.md: "check-quick green on your files = done" | team-lead | TODO |
| A3 | Stop heartbeat pings, switch to event-driven communication only | team-lead | TODO (process) |
| A4 | Bundle process changes into single broadcasts | team-lead | TODO (process) |
| A5 | Sprint-start preflight: establish verification lane + gate owner + failure rules | team-lead | TODO (process) |
| A6 | Per-task commit cadence: lead commits after each agent task completion | team-lead | TODO (process) |
| A7 | Commit message template: `<type>(<scope>): <summary> (#task-id)` | team-lead | TODO |

### Mesh-side (may need mesh project changes)

| # | Action | Scope | Status |
|---|--------|-------|--------|
| M1 | Post-compaction idle prevention: add "reach out to lead if unsure" to agent onboarding template | mesh onboarding | TODO |
| M2 | Stall detection: multi-signal composite with 10/18 min thresholds, 3-stage escalation | mesh feature | SCOPED (see D7) |

### D6: Per-task commit cadence (DECIDED)
- **Unanimous (4/4):** Commit after each completed task, not batched at sprint checkpoints
- Reduces cross-agent drift, makes breakage attributable to narrow change sets
- Commit message template: `<type>(<scope>): <summary> (#task-id)`
- Optional squash at release boundaries if history noise is a concern
- **Lead commits on behalf of agents** (agents report done → lead commits → agents work against fresh baseline)

### D7: Stall detection spec (DECIDED — for mesh implementation)
- **Unanimous (4/4):** Multi-signal composite, not single timer
- **Signals:** (a) no tool calls/commands, (b) no file writes/git diff, (c) no mesh send/read activity, (d) task state still in_progress
- **Detection rule:** Trigger only when 2+ signals are quiet simultaneously
- **Thresholds:** Soft nudge at 10 min, hard escalate at 18 min (compromise across agent suggestions)
- **Suppress when:** Long-running command active (tests/check running), agent explicitly marked "blocked" or "investigating"
- **Three-stage escalation:**
  1. Stage A (10 min): Mesh sends soft nudge to agent ("Are you still working on #N?")
  2. Stage B (18 min): Mesh alerts team-lead ("Agent X may be stalled on #N")
  3. Stage C (after nudge + no response): Lead intervenes manually
- **False-positive mitigations:** Hysteresis (2-window confirmation), whitelist active processes, allow bounded "focused analysis" status

---

## Team Discussion Results

### Q1: Commit cadence — per-task commits
**Vote: 4/4 in favor.** All agents independently preferred commit-after-each-task over batching.
Key arguments: reduces drift, makes breakage attributable, keeps baseline fresh.
Trade-off (noisier history) managed via commit template + optional release squash.

### Q2: Stall detection — multi-signal composite
**Vote: 4/4 in favor of multi-signal approach.** No agent recommended single-timer detection.
Threshold range: soft 8-12 min, hard 15-20 min. Compromise: 10/18 min.
All agents flagged same false-positive risks (long reads, running tests, lock contention).
All agents recommended suppressing alerts during active long-running commands.

---

### Parking Lot (future discussion)

- Per-agent worktree isolation (revisit if team size grows or tasks become longer-lived)
- Build-server-like check queue for multi-agent teams
- Per-stack check variants (backend-only, frontend-only)
