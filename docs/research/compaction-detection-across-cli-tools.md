# Compaction / Context-Reset Detection Across Claude Code, Codex CLI, and Gemini CLI

Date: March 8, 2026

## Executive Summary

If Taurhaus wants reliable post-compaction role-card re-injection or context re-seeding, the three CLIs are not equally capable:

| Tool | Explicit compaction primitive | Detectable external signal | Can inject context after compaction | Resume behavior | Practical feasibility |
| --- | --- | --- | --- | --- | --- |
| Claude Code | Yes | Yes, first-party hooks | Yes, first-party hooks and memory files | Strong | High |
| Gemini CLI | Yes | Yes, first-party hooks | Yes, but less targeted than Claude | Strong | Medium-high |
| Codex CLI | No stable first-party hook found | Only indirect artifacts or terminal/UI heuristics | No clean post-compaction callback found | Moderate | Low |

Recommendation:

- Build the first production version around Claude Code.
- Add Gemini support second.
- Treat Codex support as heuristic-only until OpenAI exposes a first-party compaction/session hook.

## What "reliable" means here

For Taurhaus, a reliable solution needs all of the following:

1. A detectable event when context is compressed, reset, or resumed.
2. A supported place to inject fresh context after that event.
3. A session identity that survives resume.
4. A signal that is external to the terminal UI so backend supervision can observe it safely.

Claude Code meets all four. Gemini CLI meets three cleanly and the fourth partially. Codex CLI currently misses the key hook/callback piece.

## Claude Code

### How compaction works

Claude Code has an explicit compact lifecycle:

- `PreCompact` fires before a compact operation.
- `PreCompact` exposes whether the trigger was manual (`/compact`) or automatic (context full).
- `SessionStart` also fires after compaction with `source: "compact"`.

This is the strongest first-party compaction contract of the three tools.

### Detectable external signals

Claude Code exposes direct, machine-readable signals:

- `PreCompact` hook event
- `SessionStart` with matcher/source `compact`
- `transcript_path` and `session_id` in hook payloads
- `InstructionsLoaded` events when instruction files are loaded into context

This means Taurhaus does not need to scrape terminal output or infer compaction from opaque file churn.

### Can Taurhaus inject content after compaction?

Yes, cleanly.

The best hook point is `SessionStart` with `source: "compact"`:

- Claude docs state SessionStart runs when a session starts or resumes, including after compaction.
- SessionStart supports `additionalContext`.
- stdout from the hook is also added as context.

That gives Taurhaus a supported way to re-inject role state, project guardrails, or a short role-card summary after compaction.

### What survives resume?

Claude Code explicitly documents strong resume semantics:

- `claude --continue` and `claude --resume` restore the prior conversation.
- conversation history is stored locally
- message history is restored
- tool state/results are preserved
- resumed sessions keep prior context

Separate from session resume, Claude Code also has persistent memory files:

- project instructions via `./CLAUDE.md` or `./.claude/CLAUDE.md`
- user instructions via `~/.claude/CLAUDE.md`

So Claude has both:

- persistent static memory, and
- explicit dynamic reinjection hooks.

### Practical assessment

Claude Code is the only tool here with a clearly supported post-compaction reinjection path. This is the best base for Taurhaus role persistence.

## Gemini CLI

### How compaction works

Gemini CLI exposes an explicit compression lifecycle:

- `PreCompress` fires before the CLI summarizes history to save tokens.
- trigger is `auto` or `manual`.

This is materially weaker than Claude because the hook is advisory-only, but it is still a first-party compaction signal.

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

Gemini hook capabilities relevant here:

- `SessionStart` supports `hookSpecificOutput.additionalContext`
- in interactive mode, that additional context is injected as the first turn in history
- `BeforeAgent` can append per-turn context before planning

The limitation is that Gemini's `PreCompress` hook is advisory only and cannot directly alter the compression flow. The likely Taurhaus strategy would be:

1. Observe `PreCompress` for telemetry and state save.
2. Re-inject on the next `SessionStart` or `BeforeAgent`.

That is workable, but slightly less direct than Claude's explicit `SessionStart(source=compact)` path.

### What survives resume?

Gemini documents strong resume/storage behavior:

- sessions are stored in `~/.gemini/tmp/<project_hash>/chats/`
- complete conversation history is saved
- tool executions are saved
- token statistics are saved
- reasoning summaries are saved when available
- `--resume` restores prior session context

Gemini also supports chat save/resume and `/rewind`, and the docs state rewind works across chat compression points by reconstructing history from stored session data.

That is useful for Taurhaus because it implies compression does not fully destroy recoverable structure.

### Practical assessment

Gemini is viable for Taurhaus role reinjection. It has first-party hooks, durable session artifacts, and injectable startup/turn context. The main weakness is that the compaction callback is advisory rather than a crisp "compact finished, now inject" lifecycle like Claude.

## Codex CLI

### How compaction works

I found no first-party documented compaction hook or lifecycle callback in the current Codex CLI surface.

Local CLI evidence on this machine:

- `codex --help` exposes `resume` and `fork`, but no hook command.
- `codex resume --help` exposes resume behavior, but no post-resume callback or hook integration.
- `codex debug --help` does not expose compaction/session hooks.

Upstream community evidence in the official `openai/codex` repo suggests hooks are still a requested feature, not an established one.

There is also evidence that Codex maintains internal summarization state:

- local rollout/session JSON files under `~/.codex/sessions/*.json` contain `summary` arrays with `summary_text` items
- official issue discussion around backtracking/forking references rollout JSON/JSONL internals

Inference: Codex clearly has internal summarization/history management, but it is not exposed through a stable public hook interface.

### Detectable external signals

Codex has only indirect signals today:

- local files under `~/.codex/sessions/`
- local history file `~/.codex/history.jsonl`
- possible terminal/UI behavior
- possible log mutations under `~/.codex/log/`

Those are observable, but fragile:

- file formats are implementation details
- there is no documented event meaning "compaction just happened"
- session file mutation may lag or batch changes
- terminal scraping is the weakest option and should be avoided for product logic

### Can Taurhaus inject content after compaction?

Not cleanly.

I found no supported Codex equivalent of:

- Claude `SessionStart(source=compact)` plus `additionalContext`, or
- Gemini `SessionStart`/`BeforeAgent` hook system.

What Taurhaus could still do:

- inject on resume by wrapping the Codex launch/resume command
- infer compaction via file changes or UI text and then push a new user turn

But those are heuristics, not first-party lifecycle integrations. They are good enough for experiments, not for a robust role-persistence feature.

### What survives resume?

Codex resume is real and first-party:

- `codex resume` resumes a previous interactive session
- local session files exist under `~/.codex/sessions`
- upstream issue discussion confirms the CLI can fork/backtrack from prior history and references rollout JSON/JSONL internals

Local evidence on this machine also shows structured session artifacts:

- `~/.codex/sessions/*.json`
- top-level `session` and `items`
- `summary` entries attached to some items

So Codex does preserve enough history for resume/fork workflows. The problem is not persistence. The problem is the lack of a public, deterministic hook around compaction/reset.

### Practical assessment

Codex support is possible only through heuristics:

- watch session/history files
- wrap `codex resume`
- optionally inspect terminal output

That is acceptable for diagnostics or best-effort UX hints. It is not strong enough for reliable automatic role-card re-injection.

## Comparison by Taurhaus use case

### 1. Detect that compaction happened

- Claude Code: strong yes
- Gemini CLI: yes
- Codex CLI: only inferred

### 2. Save state before compaction

- Claude Code: yes, `PreCompact`
- Gemini CLI: yes, `PreCompress`
- Codex CLI: only by polling/watching artifacts continuously

### 3. Inject state immediately after compaction

- Claude Code: yes, best path
- Gemini CLI: yes, but less exact
- Codex CLI: no clean first-party callback found

### 4. Resume with prior context

- Claude Code: strong
- Gemini CLI: strong
- Codex CLI: moderate to strong

### 5. Safe backend observability

- Claude Code: strong
- Gemini CLI: strong
- Codex CLI: weak to medium

## Recommended product strategy

### Phase 1

Ship support only for:

- Claude Code: full support
- Gemini CLI: partial/full support if we keep the implementation simple

Do not promise robust Codex post-compaction reinjection yet.

### Phase 2

For Codex, support only:

- resume-aware reinjection when Taurhaus launches or resumes the session itself
- heuristic warnings when session artifacts suggest a context reset or summarization event

Do not couple critical product behavior to Codex session file internals.

### Phase 3

Upgrade Codex support only if OpenAI adds one of:

- a session lifecycle hook system
- a documented compaction event
- a stable transcript/session event stream
- a documented "session resumed" callback with context injection support

## Bottom Line

If the goal is reliable role persistence across context resets:

- Claude Code is ready now.
- Gemini CLI is good enough to support next.
- Codex CLI is not ready for a robust implementation without heuristics.

For Taurhaus, the practical architecture is:

1. First-class Claude integration.
2. Gemini integration behind a simpler reinjection path.
3. Codex treated as best-effort until the product exposes a real hook/event contract.

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

## Local Evidence Used

Local inspection on this machine, March 8, 2026:

- `codex --help`
- `codex resume --help`
- `gemini --help`
- `gemini --list-sessions`
- inspection of `~/.codex/`
- inspection of `~/.gemini/`

Key local findings:

- Codex exposes resume/fork but no public hooks in CLI help.
- Codex stores structured session artifacts under `~/.codex/sessions/`, including embedded summaries.
- Gemini exposes hooks and resume directly in CLI help.
- Gemini stores project-scoped session artifacts locally and lists resumable sessions cleanly.
