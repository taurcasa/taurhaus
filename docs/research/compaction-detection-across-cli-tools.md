# Compaction / Context-Reset Detection Across Claude Code, Codex CLI, and Gemini CLI

Date: March 9, 2026

Updated after the March 9, 2026 Taurhaus event-driven compaction rollout.

## Executive Summary

The three CLIs are still not equally capable, but the practical Taurhaus answer changed after the event-driven Codex rollout.

| Tool | Explicit compaction primitive | Detectable external signal | Can inject context after compaction | Resume behavior | Practical feasibility in Taurhaus |
| --- | --- | --- | --- | --- | --- |
| Claude Code | Yes | Yes, first-party hooks | Yes, first-party hooks and memory files | Strong | High |
| Gemini CLI | Yes | Yes, first-party hooks | Yes, but less targeted than Claude | Strong | Medium-high |
| Codex CLI | No public first-party hook | Yes, via stable on-disk session events | Yes, via Taurhaus-owned post-detect delivery | Moderate to strong | Medium |

Recommendation:

- Keep Claude Code as the strongest first-party integration path.
- Add Gemini support next.
- Treat Codex as implementation-coupled rather than hook-native: it is now viable in Taurhaus, but it still depends on Codex session artifact semantics instead of a published callback contract.

## What "reliable" means here

For Taurhaus, a reliable solution needs all of the following:

1. A detectable event when context is compressed, reset, or resumed.
2. A supported or product-owned place to inject fresh context after that event.
3. A session identity that survives resume.
4. A signal that is external to the terminal UI so backend supervision can observe it safely.

Claude Code meets all four with first-party support. Gemini CLI meets them with a weaker but still official hook surface. Codex CLI still lacks the first-party hook piece, but Taurhaus now satisfies the product requirement by normalizing Codex JSONL compaction records into its own signal log, watcher, processor, and reinjection pipeline.

## Claude Code

### How compaction works

Claude Code has an explicit compact lifecycle:

- `PreCompact` fires before a compact operation.
- `PreCompact` exposes whether the trigger was manual (`/compact`) or automatic (context full).
- `SessionStart` also fires after compaction with `source: "compact"`.

This remains the strongest first-party compaction contract of the three tools.

### Detectable external signals

Claude Code exposes direct, machine-readable signals:

- `PreCompact` hook event
- `SessionStart` with matcher/source `compact`
- `transcript_path` and `session_id` in hook payloads
- `InstructionsLoaded` events when instruction files are loaded into context

Taurhaus does not need to scrape terminal output or infer compaction from opaque file churn.

### Can Taurhaus inject content after compaction?

Yes, cleanly.

The best hook point is `SessionStart` with `source: "compact"`:

- Claude docs state SessionStart runs when a session starts or resumes, including after compaction.
- SessionStart supports `additionalContext`.
- stdout from the hook is also added as context.

That gives Taurhaus a supported way to re-inject role state, project guardrails, or a short operational resume card after compaction.

### What survives resume?

Claude Code explicitly documents strong resume semantics:

- `claude --continue` and `claude --resume` restore the prior conversation
- conversation history is stored locally
- message history is restored
- tool state/results are preserved
- resumed sessions keep prior context

Separate from session resume, Claude Code also has persistent memory files:

- project instructions via `./CLAUDE.md` or `./.claude/CLAUDE.md`
- user instructions via `~/.claude/CLAUDE.md`

### Practical assessment

Claude Code is still the cleanest and most portable base for role persistence.

## Gemini CLI

### How compaction works

Gemini CLI exposes an explicit compression lifecycle:

- `PreCompress` fires before the CLI summarizes history to save tokens
- trigger is `auto` or `manual`

This is weaker than Claude because the hook is advisory-only, but it is still a first-party compaction signal.

### Detectable external signals

Gemini gives several useful signals:

- `PreCompress`
- `SessionStart` with `source: "resume"` or startup/clear variants
- stable on-disk session storage under `~/.gemini/tmp/<project_hash>/chats/`
- checkpoint data under `~/.gemini/tmp/<project_hash>/checkpoints`
- shadow Git snapshots under `~/.gemini/history/<project_hash>`

Locally, this machine also shows Gemini project/session state under `~/.gemini/`, and `gemini --list-sessions` returns resumable project-scoped sessions.

### Can Taurhaus inject content after compaction?

Yes, but not as precisely as Claude.

Relevant hook capabilities:

- `SessionStart` supports `hookSpecificOutput.additionalContext`
- in interactive mode, that additional context is injected as the first turn in history
- `BeforeAgent` can append per-turn context before planning

The likely Taurhaus strategy remains:

1. Observe `PreCompress` for telemetry and state save.
2. Re-inject on the next `SessionStart` or `BeforeAgent`.

### What survives resume?

Gemini documents strong resume/storage behavior:

- sessions are stored in `~/.gemini/tmp/<project_hash>/chats/`
- complete conversation history is saved
- tool executions are saved
- token statistics are saved
- reasoning summaries are saved when available
- `--resume` restores prior session context

Gemini also supports chat save/resume and `/rewind`, and the docs state rewind works across chat compression points by reconstructing history from stored session data.

### Practical assessment

Gemini is viable for Taurhaus role reinjection. The main weakness is not observability; it is the softer "observe now, inject on next lifecycle point" model.

## Codex CLI

### How compaction works

I still found no first-party documented compaction hook or lifecycle callback in the current Codex CLI surface.

Local CLI evidence on this machine:

- `codex --help` exposes `resume` and `fork`, but no hook command
- `codex resume --help` exposes resume behavior, but no post-resume callback or hook integration
- `codex debug --help` does not expose compaction/session hooks

Upstream community evidence in the official `openai/codex` repo still suggests hooks are a requested feature, not an established one.

### Detectable external signals

Codex is no longer "terminal heuristics only" for Taurhaus.

The useful signal is the active session JSONL itself:

- live sessions append to `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
- compaction appends `type:"compacted"` followed by an `event_msg` carrying `payload.type:"context_compacted"`
- Taurhaus now normalizes those records into a team-scoped append-only signal log and consumes them through an event-driven watcher/processor chain

Important limitation:

- this is still an implementation-level contract, not a published Codex hook
- Taurhaus should continue to fail closed on ambiguous parse or pairing state
- terminal scraping remains a fallback, not the primary path

### Can Taurhaus inject content after compaction?

Yes, with Taurhaus-owned delivery rather than a Codex-owned callback.

There is still no supported Codex equivalent of:

- Claude `SessionStart(source=compact)` plus `additionalContext`, or
- Gemini `SessionStart`/`BeforeAgent` hook system

But Taurhaus now has a workable product path:

1. Detect the append-only compaction boundary in the Codex session JSONL.
2. Normalize it into Taurhaus's signal log.
3. Deliver a bounded post-compaction resume card through the existing team messaging channel.

That is weaker than a first-party hook, but it is strong enough for production Taurhaus behavior if the parser remains strict and observable.

### What survives resume?

Codex resume is real and first-party:

- `codex resume` resumes a previous interactive session
- local session files exist under `~/.codex/sessions`
- upstream issue discussion confirms the CLI can fork/backtrack from prior history and references rollout JSON/JSONL internals

Local evidence on this machine also shows structured session artifacts with persisted summaries and history continuity.

### Practical assessment

Codex support is now medium-feasibility for Taurhaus:

- detection is good enough through event-driven JSONL monitoring
- reinjection is good enough through Taurhaus-owned delivery
- pre-compaction save and long-term portability remain weaker than Claude/Gemini because there is still no official Codex hook surface

## Comparison by Taurhaus use case

### 1. Detect that compaction happened

- Claude Code: strong yes
- Gemini CLI: yes
- Codex CLI: yes, via event-driven session artifact monitoring

### 2. Save state before compaction

- Claude Code: yes, `PreCompact`
- Gemini CLI: yes, `PreCompress`
- Codex CLI: no clean pre-compaction hook

### 3. Inject state immediately after compaction

- Claude Code: yes, best path
- Gemini CLI: yes, but less exact
- Codex CLI: yes, via Taurhaus post-detect delivery rather than a first-party callback

### 4. Resume with prior context

- Claude Code: strong
- Gemini CLI: strong
- Codex CLI: moderate to strong

### 5. Safe backend observability

- Claude Code: strong
- Gemini CLI: strong
- Codex CLI: medium after Taurhaus normalization

## Recommended product strategy

### Phase 1: shipped in current Taurhaus

- Claude Code: first-party compaction lifecycle support
- Codex: event-driven JSONL extraction, watcher/processor delivery, bounded resume card, and compaction audit surface

### Phase 2: next product value

- Gemini integration on the same bounded operational-card model
- compaction-aware runtime classification and mesh idle-monitor suppression

### Phase 3: future simplification

Upgrade Codex support if OpenAI adds one of:

- a session lifecycle hook system
- a documented compaction event
- a stable transcript/session event stream
- a documented resume callback with context injection support

## Bottom Line

If the goal is reliable role persistence across context resets:

- Claude Code is still the cleanest option.
- Gemini CLI is still a good second integration.
- Codex CLI is now practically supportable in Taurhaus, but by product-owned event normalization rather than a first-party hook contract.

That is the key distinction:

- Codex is still weaker at the CLI contract level.
- It is no longer "not ready" at the Taurhaus product level.

## Sources

- Anthropic Claude Code hooks: https://code.claude.com/docs/en/hooks
- Anthropic Claude Code tutorials / resume docs: https://code.claude.com/docs/en/tutorials
- Anthropic Claude Code memory docs: https://code.claude.com/docs/en/memory
- Gemini CLI hooks overview: https://geminicli.com/docs/hooks/
- Gemini CLI hooks reference: https://geminicli.com/docs/hooks/reference/
- Gemini CLI session management: https://geminicli.com/docs/cli/session-management/
- Gemini CLI checkpointing: https://geminicli.com/docs/cli/checkpointing/
- Gemini CLI command reference: https://geminicli.com/docs/cli/commands
- OpenAI Codex official repo discussions index: https://github.com/openai/codex/discussions
- OpenAI Codex discussion: "Hook would be a great feature" #2150: https://github.com/openai/codex/discussions/2150
- OpenAI Codex issue: fork/backtrack API request #4972: https://github.com/openai/codex/issues/4972
- [CHANGELOG.md](/home/mstie/projects/taurhaus/CHANGELOG.md)
- [compaction_watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs)
- [compaction_events.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/compaction_events.rs)
- [reinjection.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/reinjection.rs)

## Local Evidence Used

Local inspection on this machine, March 8-9, 2026:

- `codex --help`
- `codex resume --help`
- `gemini --help`
- `gemini --list-sessions`
- inspection of `~/.codex/`
- inspection of `~/.gemini/`
- current Taurhaus compaction implementation and changelog
