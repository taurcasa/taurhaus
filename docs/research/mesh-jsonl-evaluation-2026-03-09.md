# Mesh JSONL Evaluation — 2026-03-09

Task: `#829`

## Executive Recommendation

Do **not** convert the entire mesh file protocol to JSONL.

Recommended split:

- **Inbox messages:** move toward `JSONL`
- **Task canonical files:** keep `JSON`
- **Team config:** keep `JSON`
- **Daemon/runtime state snapshots:** keep `JSON`
- **Append-only journals, telemetry, protocol indexes:** continue using `JSONL`

The right rule is simple:

- use `JSONL` for append-heavy event streams and journals
- use `JSON` for canonical documents that are read or rewritten as a whole

## 1. JSON vs JSONL: structural difference

### JSON

JSON is one parsed unit.

Typical shapes:

- one object
- one array of objects

Implications:

- the file has one top-level structure
- if you append one element to a JSON array, you usually need to parse and rewrite the whole file
- it is good when the consumer wants the whole document as one coherent state snapshot

### JSONL

JSONL is newline-delimited JSON: one independent JSON value per line.

Implications:

- each line is a self-contained record
- you can append a new record by writing one line
- readers can process line-by-line instead of parsing a giant array first
- partial corruption is often more local: one bad line can be skipped while the rest survives

Official JSON Lines guidance describes it as a format for structured data processed one record at a time, good for shell pipelines, logs, and cooperating processes. It explicitly recommends a trailing newline because concatenation and generation become easier. Source: [jsonlines.org](https://jsonlines.org/).

## 2. LLM readability: is JSONL better than JSON for Claude/Codex/Gemini?

### Short answer

There is **no strong vendor evidence** that LLMs inherently understand `JSONL` better than ordinary `JSON` arrays/objects when the file is read directly as context.

### What the evidence does show

Vendor precedent strongly supports `JSONL` for **batch and stream processing**, not for “model cognition”:

- OpenAI Batch API requires input files formatted as `JSONL`, with one request per line, and returns `.jsonl` output. Source: [OpenAI Batch guide](https://platform.openai.com/docs/guides/batch/2-uploading-your-batch-input-file%20.doc) and [Batch API reference](https://platform.openai.com/docs/api-reference/batch/object%20.doc).
- OpenAI fine-tuning docs also use `JSONL`, one training example per line. Source: [OpenAI fine-tuning guide](https://platform.openai.com/docs/guides/supervised-fine-tuning).
- Gemini Batch API supports inline JSON arrays for smaller jobs, but recommends `JSONL` input files for larger requests and returns `JSONL` output files. Source: [Gemini Batch API](https://ai.google.dev/gemini-api/docs/batch-api).
- Anthropic’s normal API is plain `application/json`, but Message Batch results are streamed as `.jsonl`, one result object per line. Sources: [Anthropic API overview](https://docs.anthropic.com/en/api/getting-started), [Anthropic message batch results](https://docs.anthropic.com/en/api/retrieving-message-batch-results).

### What this means for mesh

The vendor pattern is consistent:

- **whole request/response documents** -> JSON
- **large batch / append / stream / line-oriented artifacts** -> JSONL

That supports an engineering conclusion, not a cognition conclusion.

### Practical LLM-readability assessment

For direct agent consumption, the tradeoff is this:

- `JSON` object/array is often easier when the agent needs one coherent structured document and relationships across entries matter immediately.
- `JSONL` is often easier when the agent is meant to scan recent records, tail the last N entries, or process each line as an independent event.

My inference:

- `JSONL` does **not** give a meaningful general readability advantage for config-style files.
- `JSONL` **does** give a workflow advantage for inbox/event files because agents and tools can cheaply read the latest records without reparsing a monolithic array.
- `JSONL` also helps context-window efficiency operationally because recent tail reads are trivial and do not require loading the whole array.

That last point is an inference from the format, not a published Claude/Codex/Gemini claim.

## 3. Append-friendly writes: where it matters

### Inbox

This is where the write pattern matters most.

Current shape in mesh:

- inbox file per agent, currently `inboxes/{name}.json`
- stored as one JSON array of `InboxMessage`
- every append and every read/ack mutation parses and rewrites the full array

Current implementation evidence:

- [inbox.rs](/home/mstie/projects/mesh/src/inbox.rs) parses the whole array on read
- `append_message`, `mark_all_read`, `mark_messages_read`, `mark_read_since`, and `ack_message` all rewrite the whole file

This is exactly the workload where `JSONL` is attractive:

- frequent appends
- recent-message reads are common
- the file naturally represents a stream of message records

### Tasks

Tasks are different.

Current shape:

- one canonical JSON file per task: `~/.claude/tasks/{team}/{id}.json`
- updates rewrite that one task file, not a giant shared task array
- append-heavy history already exists separately as `task_mutations.jsonl`

This is already a good split.

The canonical state is document-shaped, not stream-shaped.

### Config

Team config is the opposite of inbox.

Current shape:

- one canonical `config.json`
- read as a whole document
- rewritten when membership/activity/status changes
- includes nested member state plus extension fields preserved via `serde(flatten)`

This wants atomic whole-document semantics, not append semantics.

### Daemon/runtime state

Most daemon/runtime state under mesh is snapshot-like or tiny-file metadata:

- pid files
- health/activity snapshots
- idle reminder markers
- current attachment/runtime facts

These are naturally current-state documents or markers, not append logs.

## 4. Per-file recommendation

## Inbox messages

Recommendation: **move to JSONL**, but not as a naive drop-in replacement for the current mutable array.

Reasoning:

- append-heavy
- recent-tail reads matter more than full historical rewrites
- line-oriented format matches the domain: message stream
- partial corruption handling becomes better localized

Important caveat:

The current inbox model stores mutable `read` and `acked_*` state inside each message record. That clashes with JSONL if you expect in-place mutation.

So the real recommendation is:

- message bodies/events -> `inbox/{agent}.jsonl`
- read/ack state -> separate small state document or append-only sidecar journal

Good options:

1. `inbox/{agent}.jsonl` for immutable messages + `inbox_state/{agent}.json` for per-message read/ack state
2. `inbox/{agent}.jsonl` for immutable messages + `inbox_reads/{agent}.jsonl` and `inbox_acks/{agent}.jsonl` as last-write-wins journals

I would prefer option 1 first because it keeps read-path logic simpler.

## Task canonical files

Recommendation: **keep JSON**.

Reasoning:

- each task is already isolated to its own file
- canonical task state is a whole document, not a stream
- append-friendly writes are already handled by the existing `task_mutations.jsonl` journal
- moving canonical task files to JSONL would complicate reads without buying much

If task scalability becomes a problem later, improve task indexing/journaling further. Do not replace per-task canonical JSON with JSONL just for consistency.

## Team config

Recommendation: **keep JSON**.

Reasoning:

- config is authoritative master data
- consumers read it as a whole coherent object
- it benefits from atomic rewrite semantics
- preserving extension fields is straightforward in a single JSON document
- line-oriented append semantics are the wrong fit for roster/config state

## Daemon/runtime state

Recommendation: **keep JSON** for snapshots, pidfiles/markers as they are, and use `JSONL` only for append-only runtime journals.

Reasoning:

- current daemon/runtime files mostly represent latest known state, not history
- readers want “the current snapshot,” not “replay every event line”
- JSONL is useful only where the file is truly an event stream

## Existing journals / telemetry / protocol index

Recommendation: **keep JSONL**.

Reasoning:

This is already the correct pattern in mesh:

- `task_mutations.jsonl`
- `protocol_index.jsonl`
- `protocol_telemetry.jsonl`

These files are append-only, line-oriented, and queried record-by-record. They are exactly the kind of artifacts JSONL is for.

## 5. Migration path

## Recommended scope

Do this incrementally.

Phase 1:

- migrate **inbox only**
- keep task canonical files and config unchanged
- keep existing JSONL journals unchanged

## Backwards compatibility

Best migration path:

1. Teach mesh to read both formats for inboxes:
   - legacy `inboxes/{name}.json`
   - new `inboxes/{name}.jsonl`
2. Prefer writing the new `jsonl` format once migration is enabled.
3. On first successful write, convert legacy array entries into line records and rename or archive the old `.json` file.
4. Keep dual-read support for at least one release window.

## Recommended migration design for inbox

Because read/ack state is mutable, do **not** merely replace the array file with one JSONL file containing mutable message records.

Instead:

- `inboxes/{name}.jsonl` -> immutable message append log
- `inboxes/{name}.state.json` -> current read/ack state keyed by message id

Migration steps:

1. Read legacy array file.
2. Ensure every message has a stable `id`.
3. Write one JSON object per message to the new `.jsonl` file.
4. Materialize read/ack state into the sidecar `.state.json`.
5. Rename old file to `.json.bak` or delete after successful verification.

## Why not migrate config/tasks the same way?

Because there is no equivalent payoff:

- config wants whole-document truth
- canonical task files are already one-file-per-task and not suffering from monolithic-array rewrites

## 6. Taurhaus JSONL precedent

Taurhaus already uses JSONL for the structured log sink:

- canonical sink: `taurhaus.log.jsonl`
- append-only writer thread
- line-oriented rotation and retention

Evidence:

- [logging.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/logging.rs)

Lessons that carry over:

- JSONL works very well for append-only event streams.
- It composes naturally with rotation, incremental consumers, and tail-style diagnostics.
- It is a poor fit for state that needs in-place mutation or atomic whole-document replacement.

That is exactly the distinction mesh should use.

## Final Recommendation Matrix

| File type | Current shape | Recommendation | Why |
| --- | --- | --- | --- |
| Inbox messages | One JSON array per agent inbox | **Move to JSONL** | Frequent appends; stream semantics; avoid parse-whole-file rewrite |
| Inbox read/ack state | Inline mutable fields inside message array | **Split out of message log** | Mutable state is awkward in pure JSONL append form |
| Task canonical files | One JSON file per task | **Keep JSON** | Already document-shaped and isolated; append history already lives in JSONL journal |
| Task mutation history | JSONL journal | **Keep JSONL** | Correct append-only fit |
| Team config | One `config.json` object | **Keep JSON** | Canonical whole-document state |
| Protocol index / telemetry | JSONL journals | **Keep JSONL** | Correct append-only fit |
| Daemon/runtime snapshots | Small JSON state files | **Keep JSON** | Snapshot semantics, not event-log semantics |

## Bottom Line

Mesh should not ask “JSON or JSONL for everything?”

It should ask:

- is this file a **document** or a **stream**?

For mesh today:

- inboxes are trending toward **stream** semantics and should move toward `JSONL`
- config, canonical task state, and runtime snapshots are still **document** semantics and should remain `JSON`

There is no strong evidence that Claude, Codex, or Gemini intrinsically read `JSONL` better than `JSON`.
The real advantage is operational:

- append-friendly writes
- cheap tail reads
- better record-local corruption handling
- easier scaling for message/event streams

That is enough to justify `JSONL` for inboxes, but not for the whole mesh protocol.

## Sources

- JSON Lines format: https://jsonlines.org/
- OpenAI Batch guide: https://platform.openai.com/docs/guides/batch/2-uploading-your-batch-input-file%20.doc
- OpenAI Batch API reference: https://platform.openai.com/docs/api-reference/batch/object%20.doc
- OpenAI supervised fine-tuning guide: https://platform.openai.com/docs/guides/supervised-fine-tuning
- Gemini Batch API: https://ai.google.dev/gemini-api/docs/batch-api
- Anthropic API overview: https://docs.anthropic.com/en/api/getting-started
- Anthropic batch processing: https://docs.anthropic.com/en/docs/build-with-claude/batch-processing
- Anthropic batch results: https://docs.anthropic.com/en/api/retrieving-message-batch-results
