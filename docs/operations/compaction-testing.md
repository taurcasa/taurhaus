# Compaction Testing

This document defines the supported ways to trigger compaction on demand for reinjection testing.

## Supported trigger methods

### Claude Code

Supported:

1. Operator-triggered `/compact` inside a real managed Claude pane
2. Claude Agent SDK slash-command harness using `prompt: "/compact"`

Not supported / not reliable:

- telling a Claude teammate in natural language to "run /compact"
- assuming a plain model response will execute a built-in slash command on your behalf

Why:

- `/compact` is a built-in Claude command
- built-in commands are distinct from skills/tools
- Taurhaus reinjection depends on the documented Claude lifecycle:
  - `PreCompact`
  - compaction
  - `SessionStart(source=compact)`

### Codex CLI

Supported:

1. Operator-triggered `/compact` in a real managed Codex pane via tmux/operator input

Not supported / not available:

- first-party compaction hooks
- natural-language delegation as a reliable test control path

Why:

- Codex compaction is transcript-detectable, not hook-driven in our current design
- Taurhaus detects Codex compaction from session JSONL boundaries:
  - `type="compacted"`
  - `event_msg.payload.type="context_compacted"`

## Preconditions

Before running a compaction delivery test:

1. The target member must be live and attached.
2. The target member must have a resumable operational task context.
3. The target pane must already be running the intended CLI tool.

If there is no resumable task context, Taurhaus can intentionally skip delivery. That is correct behavior, not a harness failure.

## Recipes

### Claude

```bash
just test-compaction-claude taurhaus-team team-lead
```

This does the following:

1. resolves the managed Claude member
2. checks runtime health + resumable task context
3. writes a manual-run metadata file
4. sends a short filler prompt into the pane
5. sends `/compact`
6. waits for:
   - `PreCompact` evidence in Claude debug logs
   - `SessionStart(source=compact)` evidence in Claude debug logs
   - Taurhaus hook receipt + delivered event in the app log

Dry-run only:

```bash
just test-compaction-claude taurhaus-team team-lead --dry-run
```

### Codex

```bash
just test-compaction-codex taurhaus-team architect
```

This does the following:

1. resolves the managed Codex member
2. checks runtime health + resumable task context + session JSONL binding
3. writes a manual-run metadata file
4. sends a short filler prompt into the pane
5. sends `/compact`
6. waits for:
   - a real transcript compaction boundary
   - `compaction.detected`
   - terminal transport outcome (`compaction.injected` expected for a healthy positive case)
   - `wake_delivery` stage `tmux_injected`

Dry-run only:

```bash
just test-compaction-codex taurhaus-team architect --dry-run
```

### Generic entry point

```bash
just test-compaction claude taurhaus-team team-lead
just test-compaction codex taurhaus-team architect
```

## Manual-run diagnostics

Each harness writes metadata under:

```text
~/.claude/teams/<team>/state/compaction/manual-runs/<run_id>.json
```

Use the analyzer against a specific run:

```bash
python3 scripts/analyze-compaction.py --team taurhaus-team --manual-run-id <run_id>
```

The analyzer will then print targeted run diagnostics, including:

- Claude:
  - `PreCompact` seen
  - `SessionStart(compact)` seen
  - hook success seen
- Codex:
  - transcript boundary seen
- Both:
  - transport delivery outcome
  - wake stage
  - whether the compaction card was surfaced by `mesh read`

## What success means

### Claude success

A healthy positive Claude run should show:

- Claude debug log:
  - `Getting matching hook commands for PreCompact with query: manual`
  - `Getting matching hook commands for SessionStart with query: compact`
  - `Hook SessionStart:compact (SessionStart) success`
- Taurhaus log:
  - `compaction.claude_hook.received`
  - `compaction.claude_hook.resolved`
  - `compaction.claude_hook.delivered`

### Codex success

A healthy positive Codex run should show:

- session JSONL boundary:
  - `type="compacted"` or `payload.type="context_compacted"`
- Taurhaus log:
  - `compaction.detected`
  - `compaction.injected`
- mesh protocol telemetry:
  - `wake_delivery` with stage `tmux_injected`

## Limitations

1. These harnesses prove transport and surfacing boundaries, not perfect model uptake.
2. Claude testing depends on available debug logs and a real managed session.
3. Codex testing depends on a valid runtime `session_id` and `jsonl_path` binding.
4. If the member has no resumable task context, skip behavior is expected and the harness will fail early instead of producing a misleading result.
