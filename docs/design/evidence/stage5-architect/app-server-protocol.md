> **Compaction recovery / round-1 amendment (2026-09-10).** Attempt-8 step 5 completed without a hook.
> Owned-thread notifications now admit recovery, delivered under host exclusion on idle; busy recovery
> defers, background pending reads try once, and recovery failures never abort team liveness.
> Round 3: ID-less known opposite observers deduplicate within two seconds; blocked polls spend no attempt. This is fake-host coverage, not eligibility proof.
> See [Compaction boundary on 0.153.4](../../app-server-transport-amendment.md#compaction-boundary-on-01534).

> **Thread-state correction (2026-09-10).** The orchestrator's real Codex
> 0.153.4 probe supersedes the `includeTurns` and read-derived active-turn rules
> below. Neither client sends `includeTurns`; plain reads return empty turns and
> may transiently fail with -32603 before the first item persists. Such reads
> defer as `pending` under the bounded deadline, before any submission. The daemon
> tracks active IDs on its own notification connection and synthesizes bounded
> UI turns from events. A second client without an active ID waits for idle.
> Start-result turn IDs remain receipts; events do not become delivery receipts.
> See the [binding facts and archived probes](../../app-server-transport-amendment.md#thread-state-on-codex-01534-binding-from-the-orchestrators-real-host-probe-2026-09-10).
> No eligibility, protocol version, hot-switch or paid-proof scope changes.

> **TRANSPORT SUPERSEDED (2026-09-09).** The `unix_ndjson` framing this
> document assumes returns EOF on the installed Codex 0.153.4; the Unix
> endpoint requires an HTTP/1.1 Upgrade to WebSocket, then RFC 6455
> masked client text frames carrying the same `jsonrpc`-less objects.
> Binding correction: `docs/design/app-server-transport-amendment.md`
> (evidence `docs/design/evidence/native-eligibility/codex-0.153.4-attached-tui.md`).
> Every mention of NDJSON framing or "64 KiB per NDJSON frame" below
> reads as WebSocket text frames with the amendment's bounds.

> **Paired identity amendment (2026-09-10).** The JSON example now names the
> daemon's stable host/configuration/trust classes. The argument digest is additive
> launch evidence, not descriptor admission authority. Loaded project instruction
> sources are recorded and logged at info; they do not refuse hosting. Fresh seats
> may choose delivery at creation; hot switches still require the unbuilt packet.
> No native eligibility is enabled by this amendment.

# Native push / owned-host contract v1 (5a)

Software seam only, on stage-4a closing prerequisite
`adc9b831756de2fd4282198f4e63a72ba24fe928`. No production pin is enabled.
The supplied stage-5 brief names this prerequisite; this packet does not invent
an architect advance signature, stage-4b closing tip or a reviewed 5b candidate.
5b must record those prerequisites and pair its exact tip with this lane's closing
tip before activation. Stage-2 runtime-exclusion v1.1, stage-3 known-writer and
canonical activation restrictions, and stage-4 offer/switch rules still bind.
Only disposable Mesh-only canonical teams are admitted by this software seam.

## Ownership and runtime record (5b implements)

Taurhaus daemon owns the one Codex app-server child, its startup configuration,
thread creation/resume, lifecycle, liveness and operator event stream. Mesh never
spawns a process, owns stdio, starts/resumes a thread, selects an account or model,
or calls interrupt/approval/effort operations. It is a second Unix-socket client
with a separate connection and UUID request-id space on every attempt. The daemon
must keep accepting connections independently of slow model/RPC work; there are
only the two cooperative clients. Unix socket connect uses the OS connect call;
its backlog behavior is part of the unverified Unix hosting packet, not an SLA.

Runtime path: `teams/TEAM/runtime/MEMBER.json`, read under its short advisory lock
with the v1.1 displaced-record fallback. Required existing fields are
`terminalContract:1`, `memberName`, `harness:"codex"`, `health:"active"`,
`attachmentGeneration` (positive u64), `contextGeneration` (nonempty verified
string, never `unknown`), and all four `launchRoot` fields. The added object is:

```json
{
  "appServer": {
    "contract": 1,
    "socketPath": "/absolute/scratch/private/app.sock",
    "threadId": "exact-owned-codex-thread",
    "memberId": "config-agentId-incarnation",
    "accountRoot": "/absolute/scratch/codex-account",
    "processId": 123,
    "processStart": "linux-proc-start-ticks",
    "hostGeneration": "unique-host-incarnation",
    "build": "0.153.4",
    "host": "taurhaus-daemon-owned-thread/1",
    "configuration": "strict-config/1",
    "configurationDigest": "per-launch-argument-digest",
    "instructionSources": ["/absolute/scratch/project/AGENTS.md"],
    "trust": "daemon-owned/1",
    "transport": "unix-websocket"
  }
}
```

`socketPath` is the exact listening Unix socket created by Taurhaus, not a daemon
port or a guessed default path. Use a short private absolute pathname within the
Unix platform limit. `threadId` maps exactly to the persisted Codex conversation
(session); Mesh's claim `session` is that thread, with `generation` equal to the
runtime attachment counter rendered as a string. Context and owner fence remain
separate. `app_server` claims carry the complete `appServer` identity. The legacy
terminal-only attachment slots are empty/zero for this typed native attachment;
they never represent a pane and cannot be passed through RuntimeFactory to tmux.
The compiled descriptor matches build, host, configuration, trust and transport
exactly; no wildcard build is native attachment authority. Missing/unsupported
transport refuses native attachment. Account root is checked against the
initialized response's `codexHome`. Process
identity is PID plus `/proc` start ticks, never PID alone. Mesh reads no account
contents. An unknown contract/build or incomplete/stopped/recreated identity
refuses before any native input.

5b must preserve all foreign fields, add its fields to compared runtime commits
and authored/preserve sets, and publish the host identity and bumped attachment
generation atomically. Resolve root authority, team/member incarnation, account,
launch policy and named persistent thread before publication. No `--last`, cwd
adoption, fresh-thread substitution after failed resume, or PID-reuse adoption.
Owner restart never launches a child. Dispatcher outage is not agent launch
failure. Separately authoritative compaction must supply context generation and
recovery readiness; Mesh does not invent either from native output.

## Exclusion, deadlines and lifecycle

Every member operation uses the existing scheduler recipient executor. Mesh and
Taurhaus additionally share the stable inode
`teams/TEAM/state/app-server/MEMBER.lock` (exclusive flock). Never replace/unlink
it. Taurhaus UI input, effort changes, compaction transitions, shutdown and
attachment publication must acquire it, then take/release only short runtime
snapshots before host I/O. Mesh tries once and visibly defers on contention. No
task/runtime data lock, terminal lock or model-turn wait spans native RPC I/O.
The host-operation lock covers only bounded submission/state operations, not an
entire streaming turn. UI controls/approvals need their own bounded host route;
Mesh fails closed if it encounters a server request on its connection.

Mesh connects before taking the shared host-operation lock, then revalidates the
attachment under that lock before handshake/state/input. OS Unix connect remains
blocking and can stall that member's Mesh executor; it cannot hold the shared
host-operation lock while waiting. The daemon must keep its listener independently
accepting and verify backlog behavior before transport eligibility.

The Mesh attempt has at most five seconds for handshake/state/input response;
every read/write recomputes the total remaining deadline. At most 128 events,
64 KiB per NDJSON frame, and 16 full-body messages with a 16 KiB/8000-scalar
encoded budget including wrappers. Disconnect closes only Mesh's connection;
timeout is cancellation of the local wait, never cancellation of the model turn.
No child is owned or killed by Mesh. A loss after possible input is unknown,
including a partial write, malformed output or missing correlated response.

Before changing host/context/effort, Taurhaus excludes submissions with the same
host-operation lock, publishes unavailable/new-generation runtime facts, releases
short data locks, stops/excludes old clients/children, and proves named-session
recovery. It must not buffer Mesh input across such a boundary. App-server does
not enforce Mesh fencing epochs; the cooperative host exclusion is required.

## RPC and receipts

Each new connection sends `initialize` with clientInfo
`{"name":"mesh_native_push","version":"1"}` and
`capabilities.experimentalApi:true`, checks the exact reported client/build and
account root, then sends `{"method":"initialized"}`. Messages are NDJSON with
**no `jsonrpc` field**. Responses are correlated to a UUID separately from events.
Thread creation/resume stays with Taurhaus. Mesh uses
`thread/read {threadId,includeTurns:true}` to verify identity and state each time.
Idle with no active turn uses `turn/start`; active with exactly one `inProgress`
turn uses `turn/steer` and its nonempty `expectedTurnId`. Unknown/inconsistent
state fails closed, including absent/non-boolean `canAcceptDirectInput` or
absent/non-array `status.activeFlags`. Permission/input waits and unverified state
defer as `pending`, without consuming the non-Pending attempt budget. No completion
notification can silently retarget input.
This Unix second-client/read-state path is software-tested, not part of the old
paid stdio probe's verified scope; 5b must verify its exact host schema/behavior.

A delivery ID is durable locally; a request UUID is **not server deduplication**.
Stage-2 claims precede connect; dispatch markers precede handshake and input.
Per-attempt `.native` files in
`state/delivery` persist the claim, exact request, expected turn and optionally
the correlated result receipt; they are not another inbox or scheduler. They
contain the same restricted message material as claim history and inherit its
local cooperative-trust retention boundary. Once a known `failed` or
`native_enqueued` receipt is durable in claim history, Mesh unlinks that attempt's
`.native` file (also on receipt-only repair). An interrupted cleanup is retried
by reconciliation of a known receipt. Unknown/unfinished requests retain their
evidence; history retains receipt identity and cause without the extra request
body. A durable enqueue suppresses replay.
A response lost before durable evidence stays visibly unknown. Receipt-only
reconciliation uses the retained exact-claim RPC result without resubmission,
including after an owner epoch changes; missing proof never becomes enqueue.

Successful start returns a nonempty turn id; steer must return its expected id.
A returned thread id, when present, must match. Receipt: `native_enqueued`, with
thread, turn, expected turn, method and request id. Events, model markers,
completion, UI viewing and notification writes never become `consumed_by_read`
or task acceptance. Existing explicit full-body Mesh reads remain independent.

`-32600` plus the observed `no active turn to steer` is definite rejection,
including completion races. The expected/actual-turn rejection class is also
`failed`. The numeric code alone is insufficient: unclassified rejections remain
`outcome_unknown`. A definite failure may take the common bounded retry budget,
with a new attempt, handshake, runtime and thread-state validation. Disconnect
during initialize/thread-read is definite pre-input failure, not input ambiguity;
a later claim is allowed and no unresolved native outcome is created. Host-lock
contention is `pending` with thread/cause evidence and never consumes that budget.
Pre-input evidence has null input method/request/turn fields. Never turn a
rejected steer into start in the same attempt. Known enqueue/unknown cannot be
blindly retried. Neither failure nor socket loss permits automatic tmux fallback.

## Selection, policy and eligibility

`app_server` is an additive selection vocabulary but every switch into **or out
of** it refuses with `app_server_switch_requires_5b_recoverable_relaunch_packet`.
There is no activation command in 5a. 5b must extend the stage-4 fenced switch
with the exact build/host/opt-in/attachment/fence record, stopped old executors,
all unresolved receipts, and a recoverable named-session relaunch/rollback.
Configuration alone cannot enable a compiled descriptor. A member on tmux/hook
never gets an app-server call. A native attachment never falls back to a former
TUI. Existing launch/focus/interrupt/effort/manual operations are not retired.

Native input uses the same canonical recipient/causal selection, generated-action
currency and effort checks, and committed no-wake decisions. Full attributed JSON
includes author, IDs, task/assignment and restricted-review metadata. INFO-only,
empty and suppressed inputs never submit. Overflow remains explicitly readable;
no truncated body creates exposure evidence. A first item too large for encoded
native input records a pending diagnostic and is skipped so later fitting work
can progress; overflow after a fitting prefix presents that prefix first.
Authored repeats have separate IDs.
Recovery is not synthesized/composed in 5a: 5b must prove the authoritative card
arrives at startup/resume and actual mid-turn compaction before declaring ready.
Message bytes confer no GO, launch-health or lifecycle authority.

`mesh delivery capabilities` preserves the hook response and adds
`native_descriptors`: Codex 0.153.4 is **verified only for the exercised input
path**, with managed-host eligibility disabled; other builds disabled; Claude
channel disabled with strongest ordinary receipt `submitted`; agy/Grok trial
only. No eligibility bypass or mandatory acknowledgment ceremony. Claude's tested
print/stream NO-GO does not establish organization denial or general TUI failure.
agy standalone agentapi and stream hosting are distinct; Grok ACP is owned-service
hosting. Optional agy/Grok trials are explicitly deferred (no build/host/budget
commission here); guarded fallback may remain indefinitely.

5b owns SD4–SD8 intended-host proofs and SD11 paid eligibility packet: exact paired
tips and tuple, persistent identity/restart/resume, idle-after-Stop, streaming/tool/
Stop, human input/transcript/approval/cancel controls, permission wait/deny,
error/cancellation, compaction boundary, switch and teardown. Missing proof keeps
the pin disabled. Record distinct benign marker joined to thread/turn, positive
and negative controls, bytes/headers/read costs, generations separately from
turns, token classes, duplicates and actionable-context time. Fake output proves
no comprehension or savings. The paid authorization/budget belongs to 5b only.

Mesh protocol/schema and canonical format/minimum writer remain unchanged:
additive defaulted attachment and capability fields, disabled selection semantics.
Host-interface version is `appServer.contract:1`. 5b separately reviews Taurhaus
app/daemon protocol 25: bump to 26 only if wire/runtime ownership semantics the app
must interpret change, per the orchestrator decision. Pair and release app first;
this lane performs no install, pin update, merge or release.
