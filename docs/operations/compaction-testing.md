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

1. Operator-triggered `/compact` in a real managed Codex pane via tmux/operator input — **for the transcript mode only**, see the version note below
2. Codex's own automatic compaction, reached by lowering `model_auto_compact_token_limit` — the only trigger that exercises the hook mode

Not supported / not reliable:

- natural-language delegation as a reliable test control path

Why:

- By default Codex compaction is transcript-detectable. Taurhaus reads the session JSONL boundaries:
  - `type="compacted"`
  - `event_msg.payload.type="context_compacted"`
- Codex *does* have first-party hooks, and taurhaus can use them, but that path is **opt-in**: `terminal.harness.codex_compaction` defaults to `transcript`, and `hooks` additionally requires Codex ≥ 0.147 (`CliVersions.codex_compaction_hooks_supported`) plus an installed managed `hooks.json`. Managed launches in hooks mode carry `--dangerously-bypass-hook-trust`. Test whichever mode the setting is actually in.

#### Which hook a Codex compaction fires (measured, 0.149.0)

taurhaus registers exactly one Codex hook: `SessionStart` with matcher `compact`
(`compact_hook.rs`, `ensure_settings_hook_entry`). Whether a compaction reaches it
depends on the trigger. Measured on Linux with Codex 0.149.0, using a probe home that
registered `PreCompact`, `PostCompact` and `SessionStart(compact)` together:

| Trigger | `PreCompact` | `PostCompact` | `SessionStart(source=compact)` | Reaches the taurhaus bridge |
|---------|--------------|---------------|--------------------------------|------------------------------|
| automatic (`trigger: auto`) | fires | fires | **fires** | yes |
| manual `/compact` (`trigger: manual`) | fires | fires | **does not fire** | no |

Both `PreCompact` and `PostCompact` carry `session_id`, `turn_id`, `transcript_path`,
`cwd`, `model` and `trigger`; the `SessionStart` payload carries `source: "compact"` and
`permission_mode` instead of a trigger, which is why the manual and automatic cases are
told apart by what the test did rather than by a field.

Consequences for testing:

- A manual `/compact` **cannot** validate the hook mode on 0.149. It compacts — the
  transcript boundary appears — but the bridge is never invoked, so there is no
  `compaction.codex_hook.received` to wait for. `just test-compaction-codex` remains
  valid for the transcript mode, which is what it asserts.
- Automatic compaction is the trigger that proves the hook path, and it is what
  `e2e/specs/compaction-codex-hooks.js` uses for its delivery case.

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

### Codex hooks, end to end

The two recipes above drive a team you already have. `e2e/specs/compaction-codex-hooks.js`
builds one instead: it initializes a Claude-led team with a single managed Codex member
under `terminal.harness.codex_compaction = hooks`, drives it to compaction twice, and
asserts that the card came back through the hook bridge rather than the transcript
tailer.

```bash
E2E_INSTALL_DAEMON=1 just test-e2e-spec compaction-codex-hooks
```

It runs on Linux only and spends **real Codex and Claude subscription turns**, so it is
excluded from both `just test-e2e` and `just test-e2e-full` and never runs as part of a
suite. It skips itself, with the reason printed, when `codex` is missing or older than
0.147, when `~/.codex/auth.json` is absent, when `claude` is not on `PATH`, or when mesh
or tmux are unavailable.

What it isolates:

- `TAURHAUS_DATA_DIR` and `TAURHAUS_CLAUDE_DIR` are the wdio session's temp roots, so the
  team, its inboxes and the JSONL log the assertions read are all throwaway.
- `CODEX_HOME` is a scratch copy holding **only** `auth.json` and `config.toml`
  (`e2e/helpers/codexScratchHome.js`). Sessions, history and the Codex databases are
  never copied, and nothing is written back to `~/.codex` — the managed `hooks.json` and
  every rollout transcript land in the scratch home.
- The hook is a separate process Codex spawns, so it resolves the teams dir and the log
  sink from the pane's environment. The lane sets those roots on the shared `taurhaus`
  tmux session only for the length of team initialization and restores them immediately
  after.

The two cases follow the version note above:

- **manual** — `/compact` typed into the member's pane over tmux. This case does **not**
  assert a delivery, because on 0.149 a manual compaction never fires
  `SessionStart(compact)`. It waits for Codex's own transcript boundary and then asserts
  that no `compaction.*_hook.*` event was produced, pinning that gap. If it starts
  failing, Codex changed and the manual trigger became usable again.
- **automatic** — Codex's own auto-compaction, and the case that proves the bridge. It is
  bounded rather than paid for in full: the scratch `config.toml` gets
  `model_auto_compact_token_limit = 20000`, the member is restarted so it reads that, and
  the lane feeds it one ~130 KB filler file per turn, capped at **6 turns**. A probe on
  this host crossed the threshold on the first turn. If the cap is reached without a
  compaction the case fails saying so — and nothing else in the lane proves the hook path.

The automatic case asserts the acceptance trail in `taurhaus.log.jsonl`:

- `compaction.codex_hook.received` → `resolved` → `delivered` for that member, with
  `additional_context_bytes` greater than zero,
- `compaction.injected` for the member with `tool = codex`,
- and **no** `compaction.signal_emitted`, `compaction.detected` or
  `compaction.extractor.*` for it — in hooks mode the tailer owns nothing.

Codex's `SessionStart` payload is printed, so a run shows exactly what the harness put on
the wire.

The card itself is **not** in the member's mesh inbox: Codex's registry entry is
`compaction_delivery: HookStdout`, so the bridge hands the card back on the hook's stdout
as `hookSpecificOutput.additionalContext` and there is nothing queued to read with
`mesh read`. `additional_context_bytes` on the `delivered` event is the size of what was
returned.

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
  - transcript boundary seen (the analyzer reads only the transcript boundary; in `hooks` mode inspect the `compaction.codex_hook.*` events in `taurhaus.log.jsonl` directly)

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
