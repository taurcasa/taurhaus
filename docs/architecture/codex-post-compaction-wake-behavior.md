# Codex Post-Compaction Wake Behavior

## Decision

Keep Codex post-compaction wake behavior as a next-turn steer, not an interruptive forced-control mechanism.

Taurhaus should continue to:
- deliver the compaction card into the member inbox
- rely on the mesh member daemon to inject a wake prompt into the tmux pane
- let Codex consume that prompt as the next user turn

Taurhaus should not escalate to more assertive pane-control behavior such as:
- force-injecting the full compaction card directly into the active turn
- repeatedly re-sending wake prompts while the same turn is still running
- synthesizing extra terminal input that tries to interrupt or override the current model output

## Why

The audit and follow-up validation establish these facts:

1. Transport is working.
   - Taurhaus detects compaction and appends the compaction card.
   - `mesh` member daemons detect the inbox mutation.
   - `tmux` wake injection works.
   - Codex records the wake prompt as a real user turn.

2. The weak point is not transport.
   - The weak point is timing and model behavior while Codex is already busy.
   - An injected wake prompt becomes queued context, not an out-of-band interrupt.

3. More assertive behavior would be brittle.
   - It would be highly tool-specific.
   - It would risk corrupting active terminal state.
   - It would create duplicate or overlapping user-turn input.
   - It would be hard to reason about and harder to debug than the current inbox + wake model.

4. The current architecture is maintainable if observability is strong enough.
   - Once wake delivery and surfacing are separately visible, we can distinguish:
     - signal detection
     - transport delivery
     - wake injection
     - card surfacing
   - That is enough to debug real failures without introducing aggressive pane hacks.

## Operational Model

Treat the Codex compaction path as a staged delivery chain:

1. Transcript boundary detected.
2. Taurhaus resolves the managed member.
3. Taurhaus appends a compaction card to the member inbox.
4. `mesh daemon` emits wake-delivery telemetry and injects the wake prompt.
5. Codex sees the prompt as the next user message.
6. `mesh read` surfaces the card contents.

Steps 1-5 are transport/reachability.
Step 6 is the first durable evidence that the compaction card was actually surfaced to the agent session.

## Guardrails

Keep these constraints:
- only one wake prompt per inbox change
- no compaction resume card for completed or deleted tasks
- no stronger pane control without explicit new evidence that transport is failing
- treat `compaction.injected` as transport evidence only, not consumption evidence
- treat `compaction_read_surfaced` as surfacing evidence, not proof of model-quality uptake

## What To Improve Instead Of Becoming More Assertive

1. Keep mesh wake telemetry.
   - `observed`
   - `tmux_injected`
   - `tmux_failed`

2. Keep analyzer wording precise.
   - transport delivered
   - wake injected
   - card surfaced
   - no claim of guaranteed consumption beyond what is observed

3. Let idle-monitor/escalation handle true non-response.
   - if Codex remains silent after delivery, that is an orchestration problem, not a reason to immediately add stronger terminal control.

## Revisit Conditions

Revisit this decision only if telemetry shows one of these repeatedly:
- wake prompts are not reaching panes
- wake prompts reach panes but are systematically never surfaced even after subsequent turns
- the queued next-turn model causes unacceptable practical task loss that idle-monitor cannot correct

Until then, the maintainable choice is:
- keep the current wake model
- improve evidence and diagnostics
- avoid interruptive hacks
