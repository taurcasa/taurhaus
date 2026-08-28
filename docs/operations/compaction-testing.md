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

Not supported / not reliable:

- natural-language delegation as a reliable test control path

Why:

- By default Codex compaction is transcript-detectable. Taurhaus reads the session JSONL boundaries:
  - `type="compacted"`
  - `event_msg.payload.type="context_compacted"`
- Codex *does* have first-party hooks, and taurhaus can use them, but that path is **opt-in**: `terminal.harness.codex_compaction` defaults to `transcript`, and `hooks` additionally requires Codex ≥ 0.147 (`CliVersions.codex_compaction_hooks_supported`) plus an installed managed `hooks.json`. Managed launches in hooks mode carry `--dangerously-bypass-hook-trust`. Test whichever mode the setting is actually in.

### Antigravity CLI (`agy`)

Not covered. The registry declares `compaction_hook: false` and no transcript parser for `agy`, so an Antigravity compaction is not observed at all and there is nothing for a test lane to verify.

### Grok CLI (`grok`)

Detected, but with no `just` lane — trigger it by hand in a real managed grok pane.

- The hook fires on grok's own `PostCompact` event; grok's session-start source never reports `compact`.
- grok's personal hook directory (`<GROK_HOME>/hooks`) is always trusted, so there is no bypass flag to pass.
- The card is **not** returned on the hook's stdout: grok documents passive-hook stdout as ignored, so the registry routes delivery to the member's mesh inbox (`compaction_delivery: MeshInbox`). Verify with `mesh read`, not with the hook's response.
- grok also loads `~/.claude/settings.json` hooks, so one compaction can invoke the bridge twice. Exactly one reinjection is expected; a second is a bug, not a duplicate test signal.

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

`just test-compaction` accepts `claude` and `codex` only; any other tool exits with `Unsupported tool` (`justfile`, recipe `test-compaction`). Grok has no scripted lane yet.

## Manual-run diagnostics

Only the two scripted lanes write run metadata — `scripts/test-compaction-claude.py`
and `scripts/test-compaction-codex.py` both call `write_manual_run`
(`scripts/compaction_test_lib.py:199`). A grok run has no script and therefore no
metadata file; check it against the log and the inbox instead (see below).

The scripted lanes write metadata under:

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
  - transcript boundary seen (or, in `hooks` mode, the hook events)

Those two arms are the only tool-specific report logic the analyzer has
(`scripts/analyze-compaction.py:1130-1142`). For either scripted lane it also prints:

- transport delivery outcome
- wake stage
- whether the compaction card was surfaced by `mesh read`

### Grok, without a script

Grok has no `--manual-run-id` to pass, so verify it by hand:

- `taurhaus.log.jsonl`: `compaction.grok_hook.received` → `resolved` → `delivered`.
  `received` names the tool inferred from grok's reserved `GROK_*` hook env
  (`compact_hook.rs:90-96`); a `compaction.compact_hook.received` there means the
  inference failed and the run is not testing grok's path.
  A `skipped` with `post_compact_signal_only` means the delivery was routed as
  stdout, and one with `duplicate_compat_import` is the expected suppression of
  the second invocation grok makes through `~/.claude/settings.json`.
- The member's mesh inbox: the card is queued, never returned on stdout, so
  confirm it with `mesh read` — exactly one card per compaction.

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
