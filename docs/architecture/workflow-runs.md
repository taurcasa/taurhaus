# Workflow run scanning

taurhaus derives Claude Code workflow runs from the files Claude owns. W2a does not persist a second history, insert plan rows, watch the filesystem, or render workflow UI. The W2b UI can poll the IPC commands and treat the returned data as a snapshot.

## Session layout and sources

Given the parent transcript `<config>/projects/<cwd-slug>/<session-id>.jsonl`, the scanner reads the sibling `<session-id>/` directory:

| Path | Data used |
|---|---|
| `workflows/scripts/<name>-<run-id>.js` | The first `export const meta` literal: workflow name, description, and phase titles |
| `subagents/workflows/<run-id>/journal.jsonl` | Agent identity, started/result state, and the agent result preview |
| `subagents/workflows/<run-id>/agent-<agent-id>.jsonl` | First user prompt, assistant model and usage, tool calls, last tool, and file write time |
| `workflows/<run-id>.json` | The completed summary: authoritative status, per-agent `workflowProgress`, totals, timing, and result |

Only immediate run directories with path-safe IDs are considered. Malformed JSONL records and a final line caught mid-write are skipped. One bad record never makes the run or the other agents disappear.

## Live and completed snapshots

The completed summary does not exist during a live run. While it is absent, the scanner combines the script metadata, journal, and agent transcripts. The journal does not carry agent labels or phases, so live agents deliberately return `label: null` and `phase: null`; taurhaus does not infer either value from script call sites.

A journal `started` record can precede creation of its agent transcript. That short interval contributes zero tokens and tool calls; `null` remains reserved for a transcript that exists but is too large for an exact bounded total. A summary-less run remains `live` in run history because Claude supplies no durable abandoned state. The session activity hint separately excludes runs whose latest transcript write is older than its 60-second window, so abandoned directories do not inflate the active-run count.

Once `<run-id>.json` exists and parses as an object, its `workflowProgress` entries and aggregate totals are authoritative. `completed`/`success` map to `completed`, `failed`/`error` map to `failed`, and an unrecognised summary status remains `unknown` instead of being guessed. The persisted session copy of the script remains the preferred `script_path`.

## Bounded reads and cache

Run scans are bounded even when an agent transcript has grown large:

- script metadata reads stop at 64 KiB because the metadata literal is the first statement;
- the prompt comes from at most the first 16 KiB of a transcript;
- model, tool, and usage parsing reads at most the final 256 KiB;
- unchanged transcript facts are cached by path, modification time, and length;
- the process-local cache holds at most 256 transcript entries and clears before admitting a new entry beyond that bound.
- parsed completed summaries are cached by the same modification-time-and-length stamp, with at most 64 entries, because Claude writes each summary once;
- one run listing scans `workflows/scripts` once and reuses those paths for every run rather than reopening the directory per run.

For a transcript at or below the tail bound, token and tool totals cover the whole file. For a larger transcript the scanner still returns the prompt, latest model/tool, and last-write time, but returns `null` for token and tool-call totals rather than presenting a partial count as exact. Completed summaries carry Claude's exact totals regardless of transcript size.

## Session activity hint

Claude's session registry does not publish a busy edge for a headless workflow parent. On each normal session scan, a workflow-capable harness resolves the sibling session directory from its transcript path. If any summary-less run has an `agent-*.jsonl` write no more than 60 seconds old, both `RuntimeSession` and the frontend-safe session listing carry:

```json
{"workflow_activity":{"live_runs":1,"last_write_at":1787949436814}}
```

`live_runs` counts only summary-less runs with a transcript write inside that same window. The field is optional and serde-defaulted, so this is an additive daemon snapshot change and protocol 14 does not need a bump. The hub's change signature contains the optional live-run count, not the raw millisecond write time: run start/end and count changes wake a session-list poll, while each mid-run transcript append does not trigger a new long-poll version or per-member activity export. The payload still retains the exact `last_write_at` for the next real transition or periodic snapshot refresh.

The 500 ms activity path caches the run-directory and completed-summary indexes by directory stamp. Completed run IDs are remembered after first observation, so accumulated run history is not walked and re-statted on every session scan; current summary-less run transcripts still receive the file metadata checks needed to detect new writes.

## IPC and platform split

- `list_workflow_runs(session_id)` returns `WorkflowRunSummary[]`; summaries omit agents and the workflow result.
- `get_workflow_run(session_id, run_id)` returns the full `WorkflowRun`.
- `workflow_ledger_row(session_id, run_id)` returns one Markdown row or `null`.

Linux and macOS scan in-process. On Windows the app sends the additive `list_workflow_runs` and `get_workflow_run` daemon methods so paths are read inside WSL, matching account and transcript detection. Session IDs are resolved only inside detected config roots; tests install scratch config roots and never inspect a real tool home.

The ledger renderer accepts only the common procedure return shape documented in `.claude/workflows/README.md`. It escapes Markdown cells and renders `title`, `implementer`, joined reviewers, rounds, and majors, with the merge cell left as `tbd`. A plain string or any other result shape returns `null`; W2a never edits a plan document.
