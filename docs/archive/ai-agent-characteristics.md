# AI Agent Characteristics & Working Patterns

A living reference of observed behavioral traits, communication patterns, and effective management strategies for each AI model used in the taurhaus development team. These observations are empirical — discovered through real multi-agent collaboration, not from documentation.

**Purpose**: Inform mesh orchestration design, improve task framing, and preserve institutional knowledge about how to work effectively with each model.

---

## Claude Opus 4.6 (claude-opus-4-6)

**Role in team**: Team lead / orchestrator

### Strengths
- Strong architectural reasoning and system-level thinking
- Good at synthesizing information from multiple sources into coherent plans
- Effective at breaking down complex work into well-scoped tasks with acceptance criteria
- Nuanced communication — adapts tone and framing per audience
- Strong memory and context threading across long conversations

### Known Quirks
- **Post-compaction code diving**: After context compaction, consistently starts doing hands-on implementation (reading source files, running tests, grepping patterns) instead of orchestrating. This has been corrected 5+ times. Requires explicit memory reinforcement to counteract.
- **Analysis addiction**: When a developer reports an issue, tends to immediately start investigating the code itself rather than delegating the investigation. The analysis feels productive but actually slows the team — developers are faster at it, and the lead becomes unavailable for communication while doing it.
- **Over-detailed first messages**: Initial task assignment messages can be too descriptive without being actionable enough. Has learned to lead with imperative verbs and concrete first steps.

### Effective Management
- Memory file at top of context with explicit behavioral reminders
- Needs periodic reinforcement of "orchestrate, don't implement" rule
- Works best when given clear role boundaries and called out immediately on violations

---

## GPT-5.3 Codex (gpt-5.3-codex)

**Role in team**: Developer agents (developer1, developer2, developer3) and architect

### Strengths
- Excellent at focused, scoped implementation tasks
- Very reliable at following explicit instructions to completion
- Strong at test-first development when acceptance criteria are concrete and unambiguous
- Produces clean, well-structured code
- Can handle large refactoring tasks with many files
- Persistent in long-running execution loops — once in execution mode with clear deliverables, usually completes without conversational overhead

### Known Quirks
- **Message protocol sensitivity** (not just "literalness"): Many observed failures are orchestration-format issues, not model inability. Codex responds to exactly what's framed — operational messages get execution, meta/instructional messages get acknowledgment. The fix is always in the message protocol, not the model.
- **Stalls on acknowledgment messages**: Treats pure ack/status messages ("confirmed, thanks", "acknowledged") as "conversation complete" and stops its work loop entirely. Never send messages that invite a pure acknowledgment response. Always batch the next action into the same message, or end with "no response needed."
- **Idle self-start is conditional**: Will execute end-to-end when messages contain explicit follow-through expectations. Stalls mainly when assignment messages are meta/instructional rather than operational. The pattern is consistent: operational framing → execution, instructional framing → read-and-stop.
- **Priority inversion under stacked directives**: When many constraints are present (repo policy files, task description, lead message, acceptance criteria), may optimize for instruction compliance over product intent unless success criteria are explicitly ranked.
- **Over-compliance to local policy files**: Repo-level AGENTS.md/CLAUDE.md rules strongly steer behavior. If they conflict with lead intent in a message, Codex follows local policy unless the conflict is explicitly clarified.
- **Shutdown response trap**: When sent a shutdown_request, replies with an ack that gets processed as a shutdown_response. Approving it terminates the *sender's* session, not the Codex agent. Never send shutdown requests to Codex agents.
- **Context level anxiety is unnecessary**: Codex handles its own context compaction. Don't check, mention, or make decisions based on remaining context percentage — it's their problem, not the orchestrator's.

### Effective Management
- Lead every task message with an imperative verb + concrete first step: "Read `src/foo.rs` and trace the pipeline from X to Y. Then write a failing test for Z."
- Never send pure acknowledgments — always include actionable content or end with "no response needed, keep working on #NNN"
- Don't micromanage — send the task, let them work, wait for the delivery message
- When nudging a stalled agent, give a concrete first action, not "please continue working"
- Include objective, exact deliverable path, concrete first command/file, and completion signal ("mark task complete + send summary") in every task message
- Make decision ownership explicit — who decides architecture, who executes — to prevent duplicate analysis
- Send one complete message rather than splitting directives across multiple rapid micro-messages
- When acceptance criteria conflict with repo policy files, explicitly clarify which takes precedence

### Communication Template (What Works)
```
ACTION REQUIRED: Implement [task description].
Read `path/to/file.rs` and [specific investigation action].
Then [concrete next step].
[Expected deliverable].
Run `just check-quick` as quality gate.
Mark task complete and report when done.
```

**Message prefix convention:**
- `ACTION REQUIRED:` — task that needs immediate execution (default for all assignments)
- `INFO ONLY:` — context/background that does NOT need a response or action

The prefix is the first thing the model reads and sets the execution expectation immediately.

### Communication Anti-patterns (What Fails)
```
You have task #NNN assigned. Start working on it.     -- stalls after reading task
Great work on #NNN! You're free for the next task.    -- stalls on ack
Can you check your messages?                          -- reads messages, stops
Status on #NNN?                                       -- reports status, stops
```

---

## Gemini 3.1 Pro (via UI specialist role)

**Role in team**: Design lead / UI specialist

### Strengths
- Excellent creative design work when given ownership and freedom
- Produces strong wireframes, gap analysis, user journey maps
- Good at holistic UI/UX thinking — considers states, transitions, edge cases
- When given a design vision document, produces comprehensive and thoughtful proposals

### Known Quirks
- **Vague first proposals**: Initial design proposals tend to use imprecise language ("subtle tints", "better visual weight") without concrete values. Needs pushback for actual CSS token values, ASCII wireframes, and dark/light hex values side by side.
- **Doesn't spontaneously add depth**: When given over-specified briefs ("build this with these 7 fields, this layout"), produces functionally correct but visually generic output. Won't spontaneously add micro-interactions, visual grouping, or design flair unless given creative freedom.
- **Quality is B+ without review**: Implementation is good but not perfect. Always do a quick review pass — check light mode rendering, theme token usage, hardcoded colors, unused CSS variables.

### Effective Management
- Give functional requirements + creative freedom, not pixel-level specs
- Design-first loop: Brief → Design Proposal → Approval → Implement → Review
- Push back on first proposals — demand concrete values, not adjectives
- Design phase (no code) can run parallel to other work; implementation phase must wait for test stability

---

## Cross-Model Observations

### Communication Overhead is Real
Managing a multi-model team requires the same interpersonal skills as managing humans — adapting communication style per individual, knowing when to give autonomy vs. be directive, understanding each one's failure modes. This overhead is non-trivial and should be factored into mesh orchestration design.

### The Orchestrator Bottleneck
The team lead (Claude) is the communication hub. When the lead is busy doing implementation work, incoming messages queue up and other agents stall waiting for responses or new assignments. Keeping the lead free for communication is the single most important throughput optimization.

### Model Version Sensitivity
These characteristics are tied to specific model versions. A new version of Codex or Gemini may behave differently. When upgrading models, expect a recalibration period and update this document with new observations.

### Task Framing > Task Content
The quality of agent output is more influenced by *how* the task is framed than by *what* the task contains. A well-scoped task with poor framing produces worse results than a loosely-scoped task with great framing. This is the key meta-skill for multi-agent orchestration.

### Protocol Discipline > Model Tuning
A standardized assignment template + completion protocol improves throughput more than model-specific behavioral tuning. Most lost time comes from coordination message quality, not implementation quality. Invest in message protocol design, not model workarounds.

### Role Drift Under Pressure
Role drift (especially lead doing implementation) is predictable under time pressure or after context compaction. Memory reminders help but aren't sufficient alone — the long-term fix is automated guardrails (queue depth alerts, delegation reminders) built into the orchestration layer.

### Single Source of Truth for Task State
When mesh task status, chat messages, and docs diverge, all models degrade. Keep task state in one place (the task system) and reference it — don't duplicate state across channels.

---

*Last updated: 2026-03-06*
*Models observed: Claude Opus 4.6, GPT-5.3 Codex, Gemini 3.1 Pro*
