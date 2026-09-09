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
`hosted.instruction_sources.loaded`. The generated view config and policy
reassertion still enforce model/effort/sandbox/approval. This supersedes the
empty-instruction-source refusal; it makes no claim that instruction loading or
the attached TUI's settings push is a no-op.
