# App-server hosting contract — transport amendment (WebSocket over the Unix socket)

Issued 2026-09-09 by the orchestrator from the attached-TUI trial
(`docs/design/evidence/native-eligibility/codex-0.153.4-attached-tui.md`,
sidecars under `attached-tui/`). Amends `app-server-protocol.md` (mesh
stage 5a; copy under `docs/design/evidence/stage5-architect/`), whose
`unix_ndjson` framing assumption is WRONG on the installed Codex
0.153.4: a raw newline-delimited JSON-RPC `initialize` on the Unix
socket returns EOF (zero bytes); the endpoint answers only after an
HTTP/1.1 Upgrade to WebSocket.

## Observed wire (S, `trial-events.jsonl:4–5`)

Client → server over the Unix socket:

```
GET / HTTP/1.1\r\n
Host: localhost\r\n
Upgrade: websocket\r\n
Connection: Upgrade\r\n
Sec-WebSocket-Key: <base64 16-byte nonce>\r\n
Sec-WebSocket-Version: 13\r\n
\r\n
```

Server → client:

```
HTTP/1.1 101 Switching Protocols\r\n
connection: Upgrade\r\n
upgrade: websocket\r\n
sec-websocket-accept: <RFC 6455 accept of the key>\r\n
\r\n
```

Then RFC 6455 framing: client TEXT frames, MASKED (as the RFC requires
of clients), each carrying one JSON-RPC object WITHOUT a `jsonrpc`
field (as the probe observed for stdio); server TEXT frames, unmasked,
one JSON object each; `initialize` with `clientInfo` and
`capabilities.experimentalApi: true` first; request ids are per
connection. Both an "owner" client and a second "mesh" client
initialized on separate connections and shared the thread.

## What changes (both halves)

- **Mesh `app_server` adapter (5a):** replace the raw NDJSON client
  with a minimal RFC 6455 client over the Unix socket — the Upgrade
  handshake (random key, accept verified), masked text frames, close
  frames, ping/pong tolerance, a bounded frame size — hand-written
  (no new dependency; ~200 lines), unit-tested against the fake
  app-server which now speaks the same framing. Descriptor for 0.153.4
  records `transport: unix-websocket`.
- **Taurhaus host client (5b):** the same framing for the daemon's own
  client (handshake, thread/start, thread/read, input, shutdown); the
  fake app-server fixture updated; the contract text in
  `docs/architecture/harness-model.md` corrected.
- **Version pin:** the transport check is part of the descriptor's
  build pin — a Codex CLI bump re-runs step 1 of the trial before the
  descriptor stays enabled.

## Attach mechanism (S, `commands-and-rpc.md:47`)

The TUI attached by exact thread id to the running server (a remote
resume against the server's socket, launched in a private tmux pane
with `env -u TMUX` and the scratch `CODEX_HOME`), displayed the
existing transcript, showed live socket deliveries, accepted typed
input during an active steer, and reattached with full history after
the pane was closed. The taurhaus attach lane launches this in the
member's pane through the existing launch machinery.

## Binding additions from the Opus lens on the trial evidence (2026-09-09)

Evidence lens verdict: trustworthy enough to build on; these bind the
attach lane and the mesh transport lane:

1. **The attached TUI is not a passive viewer.** On the operator's
   first submission it pushed its own client-side thread settings
   (`collaborationMode` with 1,288 chars of developer instructions,
   `personality`, `multiAgentMode`) into the daemon-owned thread
   (`trial-events.jsonl:106`). Harmless in the trial only because the
   pane's scratch `config.toml` matched the `thread/start` parameters.
   **Requirement:** the attached TUI runs with a DAEMON-GENERATED
   `config.toml` under `--strict-config` in a per-member launch home
   whose model, effort, sandbox and approval policy equal the thread's
   `thread/start` parameters. Project instruction sources are retained as
   described in the 2026-09-10 amendment below; the daemon never attaches a
   TUI on the operator's own `~/.codex` config; a settings-updated
   notification that differs from the daemon's parameters is logged
   and re-asserted by the daemon.
2. **WebSocket client completeness** (the trial client was minimal):
   handle fragmentation (continuation frames), 64-bit payload lengths,
   received pings (answer pong), a proper close handshake, a RANDOM
   16-byte key with the RFC 6455 accept verified (the trial reused the
   RFC sample key), and a bounded frame size with a truthful refusal.
   No `Sec-WebSocket-Protocol`. Ids are per connection (responses are
   routed per connection; notifications broadcast to every client).
3. **Attach argv of record** (`commands-and-rpc.md:47`,
   `trial-events.jsonl:51`): `codex --remote unix://<socket> resume
   <thread-id> --no-alt-screen`; server `codex app-server
   --strict-config --listen unix://<socket>`. A pathless `unix://` form
   and the subcommands `app-server daemon`, `proxy`, `agents`,
   `remote-control` exist on this build and are unexplored — a note
   for the integration trial, not a change to the daemon-owned model.
4. **Not proven by the trial, owed to the integration trial:** the
   socket-started-turn + operator-typing direction; a steer arriving
   mid-generation; compaction identity on an attached thread; daemon
   restart; receipts through the real mesh adapter; double-exposure
   between an inbox card and a socket delivery.
5. `--no-alt-screen` duplicates the TUI header in scrollback; the
   app-server's `userAgent` is seeded by the FIRST client to connect;
   `codex resume` listing filters by cwd — the daemon resumes by exact
   thread id only.

## Paired identity (2026-09-10)

The daemon publishes stable class identities: `host:
"taurhaus-daemon-owned-thread/1"`, `configuration: "strict-config/1"`, and
`trust: "daemon-owned/1"`. The configuration class means the daemon generates
the per-member attached-TUI config under `--strict-config` with the thread's
model, reasoning effort, sandbox and approval policy. The launch argument digest
is separate additive evidence in `configurationDigest`.

Once the integration trial passes, the compiled Mesh descriptor pins exactly
these strings, alongside the exact build and `transport: "unix-websocket"`.
No bundled lock/hash or runtime choice enables native eligibility.

Loaded project instructions (including AGENTS.md) are allowed, recorded in
`appServer.instructionSources`, and logged once per launch as
`hosted.instruction_sources.loaded` at info. Only the view config sets
`project_doc_max_bytes = 0`; thread policy repair omits that discovery override
and fails closed if repaired instruction sources differ from the recorded strings.
The generated view config and policy
reassertion still enforce model/effort/sandbox/approval. This supersedes the
empty-instruction-source refusal; it makes no claim that instruction loading or
the attached TUI's settings push is a no-op.

## Thread state on Codex 0.153.4 (binding; from the orchestrator's real-host probe, 2026-09-10)

Evidence: `.check-logs/mesh-next-phase0/integration-trial-3/probe-turn-start.py`,
`probe-thread-read.py` and their `probe-events.jsonl` (real `codex app-server
--listen unix://…` under the daemon's exact `-c` overrides, isolated scratch home,
two model turns of gpt-5.6-luna at low). Both lanes copy these three files into
`docs/design/evidence/native-eligibility/integration/probe-thread-state/`.

1. `thread/read {threadId, includeTurns: true}` is REJECTED on this build:
   `{"code": -32601, "message": "list_turns is not supported yet"}`. This is the
   refusal integration trial attempt 3 hit at `launch_host` (the daemon reads the
   thread before every submission). Neither client may send `includeTurns`.
2. `thread/read {threadId}` succeeds when the thread is idle and returns `thread`
   with `id`, `sessionId`, `status: {type: "idle"}`, `canAcceptDirectInput: true`,
   `model`, `reasoningEffort`, `cwd`, `cliVersion`, `path`, and `turns: []` — the
   turn list is ALWAYS empty on this build, so no client may derive the active
   turn from `thread.turns`.
3. During the FIRST turn `thread/read` fails with
   `{"code": -32603, "message": "failed to read thread: thread-store internal
   error: … rollout at …/sessions/…jsonl is empty"}` until the first item is
   persisted. Both clients classify a `-32603` on `thread/read` as TRANSIENT
   (`pending`, retry within the bounded deadline), never as a rejection.
4. `turn/start {threadId, input: [{type: "text", text}]}` is ACCEPTED as is
   (the daemon's minimal shape) and returns `{turn: {id, status: "inProgress",
   itemsView: "notLoaded", …}}`; the attach trial's shape with `model`,
   `effort`, `serviceTierForTurn` is accepted too.
5. Notifications on the connection carry the turn state:
   `thread/status/changed {threadId, status: {type: "active"|"idle",
   activeFlags: [...]}}`, `turn/started {threadId, turn: {id, status:
   "inProgress"}}`, `item/started` / `item/agentMessage/delta` /
   `item/completed` (the `agentMessage` item carries the reply `text`),
   `turn/completed {threadId, turn: {id, status: "completed", items: [...]}}`,
   plus `thread/tokenUsage/updated`, `account/rateLimits/updated`,
   `mcpServer/startupStatus/updated`, `remoteControl/status/changed`.
   The ACTIVE TURN ID exists only in `turn/started` / `turn/completed`
   notifications and in the `turn/start` result; it must be tracked per
   connection.
6. Consequences: (a) the daemon's host client tracks the active turn and the
   thread status from the notifications it already receives, uses `thread/read`
   (no `includeTurns`) only for identity/status, and synthesizes the transcript
   the hosted UI shows from `item/*` and `turn/completed` events (bounded); it
   steers operator input with the tracked id; (b) mesh's adapter, a second client
   with its own connection, cannot learn a turn id another connection started:
   while `thread/read` reports `status.type == "active"` (or a transient
   `-32603`), the obligation is DEFERRED as `pending` with reason `thread_active`
   and delivered by `turn/start` once idle — the same "deliver only when idle"
   rule the tmux path already applies (activity idle ≤ 120 s); `turn/steer` is
   used only when the adapter itself started the active turn on the same
   connection and holds its id (bounded wait for `turn/completed`, then the next
   obligation); no double exposure, receipts unchanged (`native_enqueued` on a
   `turn/start` result carrying a turn id); (c) both fake app-server fixtures
   mirror 1–5 exactly (reject `includeTurns`, empty `turns`, `-32603` on the
   first mid-turn read, the notification sequence), so green tests mean the real
   shapes; (d) the host's JSON-RPC error object is recorded — bounded to `code`
   and the first 256 characters of `message` — in the daemon's structured log
   (`hosted.rpc.rejected` with method) and in mesh's receipt/health reason;
   UI surfaces keep the generic wording.

Source: orchestrator binding memo
`/home/mstie/projects/taurhaus/.check-logs/mesh-next-phase0/integration-trial-3/thread-state-0.153.4.md`.
Archived evidence: [turn-start probe](evidence/native-eligibility/integration/probe-thread-state/probe-turn-start.py),
[thread-read probe](evidence/native-eligibility/integration/probe-thread-state/probe-thread-read.py),
[wire events](evidence/native-eligibility/integration/probe-thread-state/probe-events.jsonl).
These are copied evidence, never gate executables. Credential-value and structured
credential-key checks passed before the byte-identical copy; scripts reference
an auth file but contain no credentials. No real host was run for this fix.
The supplied installed-schema snapshot contains only JSONRPCRequest and RequestId;
it establishes no separate `turn/failed` or `turn/cancelled` notification.
Terminal statuses on `turn/completed` (including interrupted/failed) clear activity.

Round-1 correction (2026-09-10): tracked turns survive lagging read/resume idle
snapshots. Transient reads retry within the existing host deadline and log one
`hosted.rpc.pending` per episode; exhausted reads stay pending without submission.
The byte-identical wire evidence deliberately retains installation UUID and
rate-limit/account-plan metadata (account-linked, not credentials).
