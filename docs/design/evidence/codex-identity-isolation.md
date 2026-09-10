# Codex identity isolation — partial delivery, hosted journal contract blocked

Base: `6f61f611`, branch `feat/codex-identity`, protocol 27 unchanged.
Implementation: `329f6384`, with the read-only boundary repair in `6c48eefa`. No real model CLI, live daemon, standing team or
operator tmux was used. All spawned fixtures use temporary roots and private
sockets; the fake TUI is a symlink to `/bin/sleep`.

## Defect C: established cause and remaining readiness boundary

Read-only evidence sources are the committed `messaging-e2e` packet in
`/home/mstie/projects/taurhaus-msg` and `l2-tmux-busy` packet in
`/home/mstie/projects/taurhaus-l2-tmux-busy`.

The single-seat trial's inventory has **no rollout files**, only the
`thread-writer-locks/01a08a70-8595-7ea0-a004-4ae0385d4e2f.lock` and matching
shell snapshot. Its daemon and TUI share CODEX_HOME. The executed controller
copies the native binary into scratch `.local/bin/codex`; captured native argv
starts with `codex`. The registry already accepts that basename, a symlinked
path and the node entry-script wrapper. Private tmux uses the same
`TMUX_TMPDIR` convention as `terminal_io::socket_path`; the record's socket,
PID/start and pane probe agree. `activitySnapshotPath` matches the exporter's
root/team/member path.

The retained daemon events explicitly identify **Codex PID 139** and alternate
`process_io` active readings with `none` idle readings. The last transition is
`active → idle` at **08:31:47.269Z**; the final activity file is written at
**08:31:47.271945624Z**, about 3 ms later, still `uncertain`. Process detection
therefore worked. An account mismatch, node-wrapper mismatch or private-socket
mismatch is not established as the cause of this trial. The rollout-only identity resolver
cannot identify its fresh TUI. Separately, `classify_activity_confidence` returns
`uncertain` for a quiet running non-shell process without working evidence.
The snapshot schema has no PID/tool/session fields; their absence from that
JSON does **not** establish an empty process inventory. Its
`stall_no_active_process: false` also reports a matched scanner session.

This change resolves the launch account and the process-owned writer-lock
identity. It does not turn lock ownership into idle-readiness evidence or change
Claude/shared activity semantics. **The real trial's initial-prompt delivery
readiness remains unproven and is not reported fixed.** An offline diagnostic
against the existing activity exporter confirmed that an attributed idle row
still enters its working/uncertain branches; that diagnostic is not a shipped
behavior change.

## Identity checks

Four regression tests failed on assertions before implementation (exit 101):
process-account resolution, private-tmux runtime-account resolution, hosted
exclusion, and pre-rollout writer-lock identity all returned no identity where
the fixture expected its own thread. The scoped test seam initially also failed
to compile before its methods existed; the later assertion failures are the
behavioral red evidence. After implementation, all 23 Codex resolver tests pass
(exit 0). The first contract gate found a new registry-writer handle in the scanner
and an extra tool literal; a read-only runtime-record snapshot API and the shared
Codex predicate fixed both without weakening the assertions (33 boundary tests,
exit 0).

The private server test verifies the real process inventory's PID/tool/cwd and
TTY, the runtime-selected account despite wrong process and daemon defaults,
a hosted exclusion read from a runtime record, and the resolved session/idle
result. Other assertions cover ambiguous rollouts, a formerly cached hosted
binding, and a lock descriptor belonging to a different process.

The retained messaging packet contains thread/read ID/cwd/version and runtime
records, **no session_meta rows**. Test rollouts reconstruct the minimal existing
session_meta schema; they are not claimed to be byte copies of real 0.153.4
session files, nor do they invent a recorded PID field. The requested
`docs/design/journal-stage3-brief.md` is absent at this lane's base; its 3b
contract was recovered read-only with `git show main:docs/design/journal-stage3-brief.md`
after main advanced. It specifies producer acceptance and retaining returned
IDs, but does not supply an external hosted-outcome endpoint.

## Defect B: missing external outcome endpoint

The specified existing candidate `/home/mstie/projects/mesh-push/target/release/mesh`
reports commit `4388d6a1590e3072c9dfdc61ccd08b00bff2508b` and
`journal_writer: mesh-journal/2`. Temporary-root probes returned:

- `version --json`, `journal --help`, `delivery --help`: exit 0.
- `journal receipt --help`: exit 2, `unrecognized subcommand 'receipt'`.
- `journal outcome --help`: exit 2, `unrecognized subcommand 'outcome'`.
- `delivery receipt --help`: exit 0, reserved native-hook-offer contract.

Source inspection confirms that `delivery receipt` requires a stored offer,
its matching hook request and owner claims. It is not a service endpoint for
an arbitrary hosted projection delivery ID. The journal CLI has acceptance,
reconcile, read, census, export and root handoff, but no external outcome verb.

No hosted acceptance-only patch was applied: it would leave a pending canonical
projection after socket submission and permit duplicate delivery. No consumer
read/mark-read was substituted for a transport receipt. Startup and next-turn
recovery still use the existing direct socket path; hosted journaling and the
requested positive startup/census/receipt/idempotency contract test remain
blocked on a paired Mesh endpoint. The candidate was not rebuilt or modified.

## Validation

All commands ran from the requested checkout with its own `target/`, after
bounded 30-second Cargo queue polls. Gate subprocesses had temporary HOME,
harness homes, app data and tmux roots. No foreign process was stopped.

| Command | Exit | Observed result |
| --- | --- | --- |
| `just check-quick` | 0 | Rust test compilation, typecheck, 2,495 frontend tests |
| `just lint` | 0 | Rust, frontend, workflows and recipe guards |
| `just test-contracts` | 101 → 0 | Boundary repair; final 68 contracts pass |
| `just test-rust-unit` | 0 | 2,680 library + 4 binary tests; recipe exclusions retained |
| `MESH_CONTRACT_BIN=/home/mstie/projects/mesh-push/target/release/mesh just test-canonical-mesh-contract` | 0 | Existing isolated creation/authority contract; not hosted receipt coverage |
| Codex resolver tests after final fixture isolation | 0 | 23 pass |

The final fixture-only change passes an empty runtime snapshot to the existing
notify test so it cannot enumerate a real team registry when run independently.
It was compiled by the canonical gate and then executed in the 23-test resolver
rerun; production code is unchanged from the four required green gates.
