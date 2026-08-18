# taurhaus Retrospective

taurhaus is an experimental desktop system for supervising projects and persistent AI coding teams. It is not currently under active development. This retrospective explains why I built it, what the experiment taught me, and which parts now look different as agent tooling has evolved.

This is intentionally not another architecture guide. For implementation details, start with [ARCHITECTURE.md](../ARCHITECTURE.md) and the [coordination architecture](coordination-architecture.md).

## Why I Built taurhaus

Running Claude Code, Codex, and Gemini CLI across several repositories created a coordination problem that ordinary terminal management did not solve.

Each individual session could be productive. The difficulty appeared one level above the session: remembering which project had live work, which agent owned a task, what had changed, which process had silently failed, and how to resume after a context reset or machine restart. With more parallel execution, the operator also had to keep Git state, reviews, decisions, and follow-up work aligned across tools that did not share one runtime.

The interesting problem therefore shifted from code generation to orchestration. I wanted one local control plane that could observe the real development environment instead of replacing it: existing repositories, tmux sessions, CLI tools, task files, Git history, and local filesystems.

## What I Wanted to Explore

The project was built around a set of questions rather than a claim that one orchestration model was already correct:

- Can coding agents behave as persistent team members rather than disposable calls?
- Can different CLI tools coordinate while remaining inside their native environments?
- What should be durable: the role, the agent identity, the process, the session, or the task?
- How can useful work survive process crashes, context compaction, and machine restarts?
- What evidence distinguishes a healthy worker from a merely running process?
- As execution becomes faster and more parallel, where does the real bottleneck move?

Those questions led to a desktop app, a companion daemon, and Mesh: a local coordination layer built around explicit team state, task state, inboxes, tmux processes, and recovery records.

## What Worked

### Persistent roles

Separating a durable team member from its current process or model session was the right abstraction.

A role such as architect or reviewer has an operating contract and a place in the team. A tmux pane, process ID, CLI session, or transcript is only its current attachment. That distinction made it possible to replace a crashed process or start a fresh model context without pretending that a new conceptual teammate had appeared. The implementation records durable team composition separately from rebuildable runtime attachment state; the [data architecture](architecture/data-architecture.md) documents those authority boundaries.

### Cross-agent coordination

Mesh demonstrated that Claude Code, Codex, and Gemini CLI could participate in one workflow without being forced behind a new hosted execution API. File-backed tasks and inboxes made coordination inspectable, while tmux preserved each tool's normal interactive environment.

That approach was not frictionless, but it was practical. It let agents send messages, claim and complete work, and remain individually addressable while the human operator could still attach to the underlying sessions.

### Context recovery

Context loss had to be treated as a normal lifecycle event rather than an exceptional failure. taurhaus detects compaction signals, resolves them back to a managed team member, and conditionally re-delivers bounded working context. The design includes freshness, membership, liveness, and resumable-task guards so recovery does not blindly inject stale work.

The important lesson was broader than the implementation: recovery needs durable facts outside the model's context window. A compressed conversation is helpful, but it is not a substitute for explicit task ownership, last-known validation state, and the next safe action.

### Worker health and recovery

The project became more reliable once it stopped treating “process exists” as “work is progressing.” Process state, pane state, recent activity, task lifecycle, messaging, and validation evidence are different signals. None is sufficient alone.

The later health and stall work moved toward composite classification: healthy, busy, uncertain, stalled, or broken. The [team retrospective decisions](retro/retro-2026-03-08-decisions.md) also replaced repeated timer-based nudges with state-aware reminders and bounded human escalation. That was a meaningful improvement because noisy false alarms consume the same attention needed for real failures.

### Operational visibility

Bringing project context, live sessions, task state, handoffs, Git history, search, and recovery into one view was useful even before considering autonomous teams. The UI made the system's operational state inspectable, and the structured JSONL logging pipeline made cross-layer failures traceable through frontend actions, Tauri IPC, daemon RPC, and coordination events.

This mattered because multi-agent failures are usually boundary failures: the task says one thing, the process reports another, the filesystem contains a third state, and the UI has cached a fourth. Visibility does not remove those disagreements, but it makes them diagnosable.

## What Surprised Me

The biggest constraint eventually became human direction and attention.

Faster implementation did not remove the need for product choices, architectural judgment, review, prioritization, clarification, or acceptance. It increased the rate at which those decisions arrived. More parallel agents could produce more candidate work, but they also created more branches to reconcile, more findings to triage, and more moments where an agent needed a decision before useful work could continue.

The operator increasingly became an interrupt handler for machine-speed workers.

This showed up in mundane ways: ambiguous audit-versus-implementation assignments, overlapping file ownership, repeated idle notifications, and agents blocked on small cross-stream failures. The internal [survey findings](retro/retro-2026-03-08-survey-findings.md) consistently favored bounded tasks with exact deliverables and completion signals. Later process rules introduced explicit execution modes, ownership boundaries, validation expectations, escalation rules, and WIP limits. Those were not administrative polish; they were throughput controls.

I also learned that activity and progress are easy to confuse. Tool calls, file writes, process uptime, and message traffic are observable. Useful forward motion is a judgment about the task. Any system that collapses those concepts will either interrupt healthy workers or overlook stalled ones.

## What I Would Do Differently Today

I would start with human attention as an explicit architectural resource.

- Batch decisions. Agents should return compact decision briefs with options, evidence, reversibility, and a recommendation instead of emitting a stream of small questions.
- Limit WIP by review and decision capacity, not by available model concurrency. A queue of nearly finished work can be worse than fewer completed changes.
- Define autonomy and escalation policy before scaling the team. Reversible local choices can proceed; irreversible, externally visible, or architecture-changing choices need a clear owner.
- Make confidence part of escalation. Low confidence alone should not always interrupt the operator; low confidence combined with high impact should.
- Separate evidence from conclusions. Runtime observations, task state, and messages should remain inspectable inputs to health and recovery decisions.
- Prefer a smaller tool-agnostic coordination core. Tool-specific adapters are necessary, but identity, assignment, recovery, attention, and audit concepts should not inherit one CLI's storage model more deeply than required.
- Test lifecycle state machines early. Restart, partial success, stale state, duplicate delivery, and version drift deserve first-class scenarios, not late integration cleanup.
- Treat documentation as a maintained view of code authority. Volatile counts and endpoints should point back to their source instead of being copied into diagrams and prose without a verification path.

I would also be more selective about building low-level coordination infrastructure. Some of it was necessary when taurhaus began; some now exists in the tools themselves.

## What Became Obsolete or Was Absorbed by the Ecosystem

As of August 2026, mainstream agent harnesses expose more of the primitives taurhaus explored:

- OpenAI's Codex materials describe reusable [skills and durable goals](https://developers.openai.com/codex/use-cases) for repeatable and long-running work.
- Claude Code now documents experimental [agent teams](https://code.claude.com/docs/en/agent-teams) with a lead, independent teammates, a shared task list, direct inter-agent messaging, and centralized management. Its documentation also explicitly notes remaining limitations around resumption, coordination, and shutdown.
- Gemini CLI documents both isolated [subagents](https://geminicli.com/docs/core/subagents/) and reusable [Agent Skills](https://geminicli.com/docs/cli/using-agent-skills/).

These are overlapping capabilities, not proof that taurhaus's implementation should be preserved unchanged or that every tool has converged on the same model. taurhaus also combined cross-vendor CLI processes, a desktop operational view, local project history, Windows/WSL bridging, and explicit recovery diagnostics. But the direction of travel is clear: delegation, reusable roles, persistent task context, and multi-agent coordination have moved closer to the harness layer.

I see that convergence as validation of the problem, not as a reason to portray this repository as current product infrastructure. The value of taurhaus now is the engineering record: which boundaries were hard, which failure modes appeared under sustained use, and where orchestration shifted costs rather than eliminating them.

## What Still Seems Unsolved

The durable open questions are less about spawning more agents and more about absorbing their output:

- How should a system measure useful product throughput rather than generated code or agent activity?
- What is a sustainable parallel-agent WIP limit for one human decision-maker?
- Which decisions can be safely batched, delegated, or reversed?
- How should confidence, impact, urgency, and reversibility combine into an escalation policy?
- How can an operator see one truthful attention queue without creating another notification stream?
- How should responsibility be assigned when an agent produces work faster than it can be reviewed?
- What organizational shape works when implementation latency falls faster than decision latency?

Better models reduce some execution failures. They do not automatically resolve ownership, product judgment, conflicting goals, or finite reviewer attention.

## Broader Engineering Takeaway

Increasing implementation throughput does not automatically increase product throughput.

Once execution becomes cheap, bottlenecks move upstream into problem selection, decisions, coordination, and validation. If those stages do not change, more parallel execution mostly increases inventory and interruption pressure.

The interesting engineering challenge is therefore not only how to make agents faster. It is how to design software systems, operating policies, and human organizations that can absorb machine-speed implementation without losing judgment, coherence, or control.
