# Compaction Testing

Operational guide for validating post-compaction context delivery through native harness hooks. These lanes use real managed sessions; the Codex E2E lane spends subscription turns and is never part of a normal test suite.

## Current support

| Harness | Trigger that reaches taurhaus | Delivery | Minimum/version note |
|---|---|---|---|
| Claude Code | `SessionStart(source=compact)` after `/compact` or automatic compaction | `hookSpecificOutput.additionalContext` on hook stdout | Managed hook is always reconciled |
| Codex CLI | `SessionStart(source=compact)` after **automatic** compaction | `hookSpecificOutput.additionalContext` on hook stdout | Managed hook is default at Codex >= 0.147; older versions log `compaction.codex_hook.unsupported` once and receive no reinjection |
| Antigravity CLI (`agy`) | None | None | Registry declares `compaction_hook: false` |
| Grok CLI (`grok`) | `PostCompact`; imported Claude `SessionStart(compact)` is deduplicated | Member mesh inbox | Grok ignores passive-hook stdout |

The Codex transcript extractor, signal log/watcher/processor, owner election and `harness.codex_compaction` setting are retired. Do not look for `compaction.detected`, `compaction.signal_emitted`, `compaction.extractor.*`, or transcript-mode delivery.

## Hook installation and platform behavior

`coordination/compact_hook.rs` owns one bridge for Claude, Codex and Grok. Managed-hook reconciliation preserves foreign registrations and is idempotent. It repairs the wrapper when the taurhaus executable moves.

- Linux and WSL runtimes receive a `.sh` wrapper.
- Native Windows runtimes receive a `.cmd` wrapper.
- Codex hooks live under the resolved managed `CODEX_HOME`, not an assumed host home.
- Startup and terminal-settings reconciliation install the Codex hook whenever a supported managed Codex member exists.
- A supported managed Codex launch carries the hook-trust bypass required for the managed registration.

The tool is inferred from grok's reserved hook environment and otherwise from the transcript path. Member resolution uses runtime `session_id` first and normalized `cwd` second, including Windows, WSL UNC and Linux path forms.

## Trigger behavior

### Claude Code

The manual lane sends a filler prompt followed by `/compact`. A successful lifecycle is:

1. `PreCompact`
2. compaction
3. `SessionStart(source=compact)`
4. `compaction.claude_hook.received` -> `resolved` -> `delivered`

### Codex CLI

Only **automatic** Codex compaction validates taurhaus's hook. Before launching the target member, set a low enough `model_auto_compact_token_limit` in that managed Codex home to reach automatic compaction in a bounded number of turns. The manual script deliberately does not mutate Codex configuration.

Measured on Codex 0.149.0 and 0.150.1:

| Trigger | `PreCompact` | `PostCompact` | `SessionStart(source=compact)` | Reaches taurhaus |
|---|---|---|---|---|
| automatic | fires | fires | **fires** | yes |
| manual `/compact` | fires | fires | **does not fire** | no |

A manual `/compact` can create a transcript boundary, but it cannot validate delivery because Codex does not invoke the registered `SessionStart(compact)` hook. This is why the scripted Codex lane uses filler turns and never sends `/compact`.

### Grok CLI

There is no `just` lane. In a real managed grok pane, trigger compaction and verify:

- `compaction.grok_hook.received` -> `resolved` -> `delivered` in `taurhaus.log.jsonl`.
- Exactly one card in the member's mesh inbox, observable with `mesh read`.
- A second invocation through grok's imported Claude hook is skipped as `duplicate_compat_import`.

A `post_compact_signal_only` skip means the registry incorrectly routed grok as a stdout-answered harness.

## Preconditions

Before a delivery test:

1. The target member is live and attached to the expected managed harness.
2. The operational snapshot contains a resumable task.
3. The app and daemon are the paired build; protocol 14 rejects a mismatched peer.
4. For Codex, the installed CLI is >= 0.147 and any lower automatic-compaction threshold was configured before member launch.

No resumable task is an intentional skip, not a transport failure.

## Scripted recipes

### Claude

```bash
just test-compaction-claude taurhaus-team team-lead
```

The recipe resolves the managed member, checks runtime health and resumable context, writes manual-run metadata, sends filler plus `/compact`, and waits for Claude debug evidence and the native-hook event trail.

Dry run:

```bash
just test-compaction-claude taurhaus-team team-lead --dry-run
```

### Codex

```bash
just test-compaction-codex taurhaus-team architect
```

The recipe:

1. resolves a healthy managed Codex member with resumable context;
2. creates a temporary `.taurhaus-compaction-filler-<run-id>.md` in the member project;
3. sends bounded filler turns to trigger automatic compaction;
4. waits for `compaction.codex_hook.received` and a terminal `delivered`, `skipped`, or `failed` event;
5. requires `compaction.codex_hook.delivered` and reports `additional_context_bytes`; and
6. removes the filler file in cleanup if it created it.

If it exhausts the turn limit, confirm `model_auto_compact_token_limit` was configured before this member launched. The script never edits `CODEX_HOME` or sends `/compact`.

Dry run:

```bash
just test-compaction-codex taurhaus-team architect --dry-run
```

Generic entry points:

```bash
just test-compaction claude taurhaus-team team-lead
just test-compaction codex taurhaus-team architect
```

`just test-compaction` accepts only `claude` and `codex`.

## Native-hook analyzer

The scripted lanes write metadata below `teams/<team>/state/compaction/manual-runs/`. Analyze a run with:

```bash
python3 scripts/analyze-compaction.py \
  --team taurhaus-team \
  --member architect \
  --manual-run-id <run-id>
```

Or inspect a time window directly:

```bash
python3 scripts/analyze-compaction.py --last 30m
```

The analyzer reads only native-hook events:

- `compaction.claude_hook.*`
- `compaction.codex_hook.*`
- `compaction.grok_hook.*`
- `compaction.compact_hook.*` when tool inference failed

It reports counts by tool/action, terminal outcomes, skip/failure reasons and recent events. It does not parse transcripts or old signal files.

## Paid Codex E2E lane

`e2e/specs/compaction-codex-hooks.js` builds a Claude-led team with one managed Codex member and proves default hook delivery. It is Linux-only, spends real Claude and Codex subscription turns, is excluded from `just test-e2e` and `just test-e2e-full`, and must be invoked explicitly:

```bash
E2E_INSTALL_DAEMON=0 just test-e2e-spec compaction-codex-hooks
```

Keep `E2E_INSTALL_DAEMON` at its safe default of `0`. The worker launches the checkout-local `src-tauri/target/debug/taurhaus-daemon` on its own private port, so setting it to `1` only rebuilds and restarts the *operator's* installed daemon and contributes nothing to the run.

The lane asserts the retired `codexCompaction` and `codex_compaction` settings are absent, lowers `model_auto_compact_token_limit` in a scratch Codex config **before launch**, triggers bounded automatic compaction, and requires:

- `compaction.codex_hook.received` -> `resolved` -> `delivered`;
- `compaction.injected` with `tool = codex`;
- no hook failure event; and
- the card's unique marker in Codex's own rollout transcript, proving Codex consumed the hook response.

A second case sends manual `/compact`, requires Codex's own transcript boundary, and asserts that no native-hook event appeared. It pins the measured 0.149/0.150 contract; if Codex later emits `SessionStart(compact)` for manual compaction, the test and this runbook must change together.

Isolation requirements are strict, and the worker owns every writable root: `HOME`, `TAURHAUS_DATA_DIR`, `TAURHAUS_CLAUDE_DIR`, `CODEX_HOME`, `GROK_HOME` and the taurhaus-only `TAURHAUS_AGY_DIR` all point inside the session temp directory, alongside a private daemon port and a private tmux server (`e2e/helpers/workerEnv.js`, `e2e/helpers/laneTmux.js`). The scratch Codex home contains only `auth.json` copied from the source home plus a *generated* `config.toml` — the operator's own config can register things Codex executes, and a configured `notify` in particular would displace the notifier taurhaus installs, which is this lane's only turn signal. Operator config, sessions, history and databases are never copied or written back. Unit and integration tests use generated temporary directories and must never read or write `~/.codex` or `~/.claude*`.

## Success and failure interpretation

A healthy stdout-delivery run has `received` -> `resolved` -> `delivered` and a positive `additional_context_bytes`. `compaction.injected` records the completed delivery. The card is not in the Claude or Codex member inbox.

Expected skips include missing resumable context and duplicate grok compatibility imports. Failures include payload parsing, member resolution, card composition, response serialization and managed-hook execution errors; use the event's `failure_stage` or `skip_reason` rather than inferring from absence.

These lanes prove trigger, transport and surfacing boundaries. The paid Codex lane additionally proves the returned marker reached Codex's transcript; none can prove how a model semantically uses every restored detail.
