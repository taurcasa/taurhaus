# Layout Engine Pipeline — Retro

Date: 2026-03-06
Tasks: #413–#418
Participants: architect, developer1, developer2, developer3

## Summary

5-task pipeline completed: architecture concept (#413), pure layout engine with TDD (#414), MeshConnection refactor (#415), MeshCanvas integration (#416), visual verification (#417). Result: 34 visual tests passing, all connection routing bugs structurally resolved by replacing scalar `bend` with explicit cubic control points.

This was also the first pipeline with working bidirectional mesh comms (0.2.1).

## What went well (consensus)

- **Scoped pipeline**: Each task had a clear deliverable. The split between pure engine, component refactor, integration, and verification was logical.
- **TDD worked**: Pure layout invariant tests caught geometry issues before they reached the renderer. Visual specs provided regression confidence.
- **Existing infrastructure paid off**: The visual testing lane built in the previous pipeline (#404–#410) made verification fast and reliable.
- **Architecture-first**: Having the concept doc (#413) before implementation gave everyone a shared contract to work against.

## What needs improvement (consensus themes)

### 1. Task overlap and handoff ambiguity
developer2 did much of the integration work during #414, making #415 and #416 partially redundant verification tasks. Agents weren't sure if they should author new code or verify existing changes.

**Action**: Each task assignment should explicitly state: "author net-new code" vs "verify and clean up existing changes."

### 2. Stale dependency metadata
`blockedBy` flags weren't automatically cleared when upstream tasks completed, causing confusion about what was actually unblocked.

**Action**: Send an explicit "blocker cleared" message when completing a task that blocks others, not just a task status update.

### 3. Over-prescribed mechanics vs acceptance criteria
Some task messages prescribed implementation steps instead of outcomes. This created noise and sometimes conflicted with what the code actually needed.

**Action**: Lead with acceptance criteria. Only prescribe mechanics when the agent lacks context about the approach.

### 4. Idle monitor noise during active work
Repeated nudge/reminder messages arrived while agents were actively working, creating distraction.

**Action**: This is a mesh-level tuning issue — idle monitor should respect active task status.

## Collaboration & communication

- **Mesh 0.2.1 validated**: All 4 agents sent retro responses successfully. Bidirectional comms confirmed working.
- **"YOU MUST:" framing worked**: Agents consistently acted on messages with this prefix. No stalls from message-as-task confusion.
- **Previous session's send outage was the biggest collaboration blocker**: Multiple agents noted the pre-fix period caused uncertainty and stalls. The fix resolved it completely.

## Ideas for future pipelines

1. **Publish phase map up front** with owner, artifact path, and dependency edge per task (architect)
2. **Designate contract owner** for shared fixtures/schemas before parallel slices start (developer2)
3. **Shared "known red tests" board** to reduce duplicate investigation of unrelated failures (developer3)
4. **Merge verification-only tasks** into the implementation task when the work is small (developer2)
5. **Standardize on spec-generated screenshots** as primary review artifact; visual host is manual support, not required automation (developer3)

## Mesh 0.2.1 validation result

All 4 agents sent survey responses via `mesh send`. No send failures. The rejoin reactivation fix is confirmed working in production use.
