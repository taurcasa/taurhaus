# Communication Analysis Framework

Observed context for this framework:

- Message data lives primarily in `~/.claude/teams/<team>/inboxes/*.json`
- Task state lives in `~/.claude/tasks/<namespace>/*.json`
- Task change audit lives in `~/.claude/teams/<team>/state/task_mutations.jsonl`
- Version slices are defined in [communication-version-timeline.md](/home/mstie/projects/taurhaus/docs/analysis/communication-version-timeline.md)

This framework is designed to evaluate communication quality in mesh-based multi-agent teams using the data shapes currently available on disk.

## Goal

Produce a repeatable assessment of how well a team communicates within a given version slice:

- How much communication happened
- Who talked to whom
- Which messages created progress versus noise
- How efficiently tasks moved from assignment to completion
- Which communication habits caused stalls or prevented them

## Analysis Unit

The default unit of analysis is one version slice for one team, for example:

- Team: `taurhaus-team`
- Version slice: `mesh 0.2.1`
- Time range: from the version transition timestamp until the next version transition or present

Within a slice, analyze three connected objects:

1. Messages
2. Tasks
3. Task mutation events

## Data Model And Extractable Signals

| Source | Directly measurable | Heuristic / inferred |
| --- | --- | --- |
| Inbox JSON | sender, recipient inbox, timestamp, read state, raw text, optional summary | message type, whether it was actionable, whether it was redundant, implied reply chains |
| Task JSON | task id, owner, status, subject, description, dependency fields | likely assignment time from file mtime if explicit creation time is absent |
| Task mutations JSONL | who changed a task, when, which fields changed | whether a communication exchange caused the mutation |

Use direct measurements for counts and timing whenever possible. Use heuristics only when the raw files do not explicitly encode the relationship.

## Message Taxonomy

Every message in a slice should be tagged with one primary class.

### Primary classes

| Class | Definition | Examples |
| --- | --- | --- |
| Assignment | Directs work with objective, deliverable, first step, completion signal, response expectation | team-lead task kickoff, task assignment payload |
| Status | Reports progress, current state, partial findings, or blockers | “starting task”, “inventory complete”, “blocked on X” |
| Completion | Declares work finished and provides artifacts/results | “task 505 completed”, doc path, test status |
| Coordination | Clarifies ownership, dependencies, sequencing, or unblocking | “pick up #506”, “this blocks #507” |
| Informational | Context that does not require a response | `INFO ONLY:` messages |
| Reminder / Nudge | Prompts resumed activity without adding new task content | idle-monitor reminder, follow-up nudge |
| Acknowledgment | Pure receipt/confirmation with minimal new information | “understood”, “acknowledged”, “will do” |
| Redundant | Repeats already available instruction or information without materially changing next action | duplicate assignment text, repeated broadcast |

### Direction tags

Each message should also be tagged by direction:

- lead -> agent
- agent -> lead
- agent -> agent
- lead -> broadcast / unassigned
- system / monitor -> agent

## Dimensions To Analyze

Score each dimension on a `1-5` scale.

- `5`: strong pattern, low ambiguity, clearly supports progress
- `4`: good overall, minor waste or ambiguity
- `3`: mixed quality, acceptable but noticeably inefficient
- `2`: weak, repeated friction or unclear coordination
- `1`: poor, communication frequently blocks progress

## Dimension 1: Message Pattern Quality

Measures whether the team used the right amount and direction of communication.

### Metrics

- Total messages in slice
- Messages per active task
- Direction mix: lead -> agent, agent -> lead, agent -> agent, broadcasts
- Messages per actor
- Broadcast share: broadcasts / total messages
- Reminder share: reminder/nudge messages / total messages

### Heuristics

- Healthy pattern: most traffic is lead -> agent assignment/status routing and agent -> lead progress/completion.
- Watch for over-broadcasting: high unassigned or multi-recipient traffic without corresponding unblocking value.
- Watch for silent execution risk: too few agent -> lead status/completion reports relative to task completions.

### Scoring guide

| Score | Indicators |
| --- | --- |
| 5 | Direction mix is purposeful, broadcast use is rare and justified, low reminder dependency |
| 4 | Mostly clean flow, some extra nudges or repeated routing |
| 3 | Noticeable imbalance, such as too many lead follow-ups or too little peer coordination |
| 2 | Heavy broadcast/retry traffic or uneven participation from key actors |
| 1 | Communication is chaotic, missing, or dominated by reminders and repeats |

## Dimension 2: Responsiveness And Loop Closure

Measures whether requests receive timely, useful responses and whether threads close cleanly.

### Metrics

- Median response time from assignment/instruction to first substantive reply
- Median response time from blocker report to next coordinating response
- Completion report lag: task completion event minus last completion message
- Open-loop count: assignment-like messages with no meaningful follow-up in window

### Heuristics

- A substantive reply is a status, blocker, or completion message, not a pure acknowledgment.
- If explicit threading is unavailable, pair messages by task id, sender/recipient pair, and proximity in time.
- Count an exchange as “closed loop” when the request is followed by progress/completion or explicit “no response needed”.

### Scoring guide

| Score | Indicators |
| --- | --- |
| 5 | Fast substantive replies, low open-loop count, blockers answered quickly |
| 4 | Most loops close cleanly, occasional slow follow-up |
| 3 | Moderate lag or ambiguous closure on several threads |
| 2 | Frequent delays, many reminder-driven resumptions |
| 1 | Requests routinely hang or require repeated prompting |

## Dimension 3: Signal-To-Noise Ratio

Measures how much communication creates actionable information versus overhead.

### Metrics

- Actionable message share: assignment + status + completion + coordination / total messages
- Acknowledgment share
- Redundant message share
- Reminder/nudge share
- Average words per actionable message versus acknowledgment message

### Heuristics

- Treat `INFO ONLY:` with concrete context as useful signal, not noise.
- Treat pure acknowledgment as noise unless it carries new execution intent, constraint, or ETA.
- Repeated restatement of the same assignment counts as redundant unless it changes scope, deadline, or owner.

### Scoring guide

| Score | Indicators |
| --- | --- |
| 5 | Most messages change what someone should do or know; low acknowledgment/redundancy share |
| 4 | Small amount of ceremony, but signal still dominates |
| 3 | Noticeable clutter from acknowledgments, reminders, or repeated instructions |
| 2 | High communication volume with modest net progress signal |
| 1 | Noise dominates; extracting real task state requires manual reconstruction |

## Dimension 4: Task Lifecycle Efficiency

Measures whether communication supports fast, low-friction task movement.

### Metrics

- Messages per completed task
- Task mutation events per completed task
- Blocked/unblocked cycles per task
- Time from assignment to first execution signal
- Time from first execution signal to completion
- Reassignment count per task

### Heuristics

- Efficient lifecycle: one clear assignment, a small number of progress updates, one completion report.
- Expensive lifecycle: repeated restarts, repeated clarifications, many owner/status flips.
- Use task mutation history as the system-of-record for status transitions, then map nearby communication around those events.

### Scoring guide

| Score | Indicators |
| --- | --- |
| 5 | Tasks move with few messages and few mutation reversals |
| 4 | Mostly efficient, with isolated clarification overhead |
| 3 | Several tasks need extra prompting or status churn |
| 2 | Repeated blocked/unblocked loops or many messages per task |
| 1 | Communication overhead is a major part of task execution cost |

## Dimension 5: Anti-Pattern Detection

Measures whether the slice contains known bad communication behaviors.

### Anti-patterns to flag

| Anti-pattern | Detection heuristic | Why it matters |
| --- | --- | --- |
| Duplicate assignment | Same sender repeats materially identical assignment within a short window | Wastes attention, often signals lack of trust in loop closure |
| Unnecessary broadcast | Broadcast/unassigned message where only one owner is relevant | Creates context load for uninvolved agents |
| Pure acknowledgment loop | Assignment -> ack -> ack-like follow-up without progress | Adds traffic without advancing task state |
| Reminder-driven execution | Real progress appears only after idle-monitor or repeated nudge | Suggests weak self-reporting or poor loop closure |
| Stall-causing ambiguity | Messages missing objective, deliverable, or next step, followed by clarification or inactivity | Turns communication defects into execution stalls |
| Wasted round-trip | Clarification that could have been avoided by including artifact path, task id, or completion signal up front | Inflates latency and message count |

### Scoring guide

| Score | Indicators |
| --- | --- |
| 5 | Anti-patterns are rare and isolated |
| 4 | Small number of low-cost anti-patterns |
| 3 | Recurrent but manageable friction patterns |
| 2 | Anti-patterns meaningfully slow the team |
| 1 | Anti-patterns are common and structurally damaging |

## Dimension 6: Effective Pattern Detection

Measures the presence of communication habits that reliably produce good outcomes.

### Effective patterns to credit

| Pattern | Detection heuristic | Why it matters |
| --- | --- | --- |
| Complete assignment message | Objective, deliverable path, first action, completion signal, response expectation all present | Reduces clarification and startup delay |
| Progress-before-pause reporting | Agent sends concise progress summary before context switch or compaction risk | Preserves continuity |
| Artifact-bearing completion | Completion message includes file path, result summary, and test status | Makes verification immediate |
| Fast blocker escalation | Agent reports blocker early with concrete next need | Minimizes dead time |
| Minimal but sufficient updates | Agent sends short informative updates during longer work | Balances transparency with low traffic |
| Clean handoff / unblock | Completion message explicitly points to next task or newly unblocked work | Keeps pipeline moving |

### Scoring guide

| Score | Indicators |
| --- | --- |
| 5 | Strong presence of reusable good patterns across actors |
| 4 | Good patterns are common but not universal |
| 3 | Mixed usage; some actors communicate well, others do not |
| 2 | Effective patterns are occasional rather than default |
| 1 | Little evidence of disciplined coordination habits |

## Overall Scoring

Use weighted scoring so the final assessment emphasizes progress, not just message aesthetics.

| Dimension | Weight |
| --- | --- |
| Message pattern quality | 15% |
| Responsiveness and loop closure | 20% |
| Signal-to-noise ratio | 20% |
| Task lifecycle efficiency | 20% |
| Anti-pattern detection | 15% |
| Effective pattern detection | 10% |

### Overall classification

| Score | Classification |
| --- | --- |
| `4.5-5.0` | Excellent |
| `3.8-4.49` | Strong |
| `3.0-3.79` | Mixed |
| `2.0-2.99` | Weak |
| `<2.0` | Failing |

## Recommended Analysis Workflow

1. Define the version slice and date range.
2. Extract all inbox messages in range.
3. Extract all task mutations in range.
4. Identify the active tasks in range from task files, mutations, and message references.
5. Tag each message with direction and primary class.
6. Compute direct metrics.
7. Apply heuristics for response loops, redundancy, and anti-patterns.
8. Score each dimension.
9. Summarize strongest and weakest patterns.
10. Recommend changes specific to that version slice.

## Output Template For A Version Slice

Use the following structure when applying this framework to a concrete slice.

```md
# Communication Analysis: <team> / <mesh version>

## Slice Definition
- Team:
- Mesh version:
- Date range:
- Message count:
- Task count:
- Task mutation count:

## Executive Assessment
- Overall score:
- Classification:
- One-sentence summary:

## Dimension Scores
| Dimension | Score | Key evidence |
| --- | --- | --- |
| Message pattern quality |  |  |
| Responsiveness and loop closure |  |  |
| Signal-to-noise ratio |  |  |
| Task lifecycle efficiency |  |  |
| Anti-pattern detection |  |  |
| Effective pattern detection |  |  |

## Message Pattern Findings
- Direction mix:
- Broadcast usage:
- Response-time observations:

## Signal-To-Noise Findings
- Actionable share:
- Acknowledgment share:
- Redundancy observations:

## Task Lifecycle Findings
- Messages per completed task:
- Blocked/unblocked cycles:
- Fastest and slowest task examples:

## Anti-Patterns
- Pattern:
  Evidence:
  Impact:

## Effective Patterns
- Pattern:
  Evidence:
  Impact:

## Representative Examples
- Good example:
- Bad example:

## Recommendations
- Keep:
- Change:
- Investigate next:
```

## Practical Notes For This Repo

- The framework should treat `mesh 0.2.1` as the highest-priority slice because it is both current and heavily populated.
- Inbox data is the best source for message quality analysis.
- Task mutation JSONL is the best source for lifecycle transition analysis.
- Task file mtimes are useful for approximate volume, but weaker than explicit timestamps and should be labeled as approximate in any write-up.
