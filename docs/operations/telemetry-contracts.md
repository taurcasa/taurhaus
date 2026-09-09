# Mesh telemetry and wait conformance

These Linux regression tests generate their inputs with the lock-matching Mesh
binary (`src-tauri/resources/mesh.lock.json`), never handwritten task, ruling,
monitor or workflow records. Install that binary at `~/.local/bin/mesh`, or set
`MESH_CONTRACT_BIN` to its absolute path. Python 3 and `cc` are also required.
`just test-mesh-contracts` is the explicit Linux operator gate for changes to
these readers or fixtures. Missing prerequisites or a lock mismatch fail there.
The four `mesh_binary_` tests carry `#[ignore]` with a prerequisite message;
the operator recipe explicitly selects them with `--ignored`.
The default `just test-rust-unit` recipe prints a NOT RUN notice; the integration
recipe does not select these lib tests either. Neither CI lane nor an unfiltered
`cargo test` needs a host Mesh installation.

Run from the checkout root:

```sh
just test-mesh-contracts
just test-rust-unit
just check-quick
just lint
just test-contracts
```

`scripts/mesh-contract-fixture.py` creates only temporary teams with an explicit
`--claude-dir`, empty child environment and scratch HOME. No AI harness or tmux
client runs. The monitor fixture advances only Mesh's realtime clock with a
scratch shared library, leaving host time and monotonic polling unchanged. Each
foreground monitor is terminated and reaped in `finally`. No generated inputs
are checked in. Taurhaus launch/snapshot fixtures remain Taurhaus-produced.
Monitor cycles wait up to 30 seconds for the producer's completed-cycle log;
the final cycle must persist `launch_health` in task metadata. The exact final
record sequence remains asserted. Fake-clock wait regressions run in the
default unit lane (or `python3 scripts/mesh-contract-fixture.py --self-test`),
without starting any CLI.

## Canonical activation candidate contract

`MESH_CONTRACT_BIN=/absolute/path/to/candidate/mesh just test-canonical-mesh-contract`
selects one ignored real-binary test for canonical team creation and
`TeamConfigStore` round-trip preservation, including Mesh's joined lead identity.
It requires an explicitly selected canonical-capable binary; it never resolves
`~/.local/bin/mesh`. Missing selection reports NOT RUN and exits nonzero; the
ordinary unit lane reports NOT RUN and leaves this test ignored. The child runs
with an empty environment, scratch HOME/cwd and explicit tempdir `--claude-dir`.
It checks help, creates a disposable team using the shipping frontend policy,
and verifies that Mesh still authenticates the lead after Taurhaus saves the
config (delivery refuses only for the missing runtime record). No harness, daemon
or tmux is launched. A passed creation contract does not prove live delivery eligibility.

## Reader decisions

- Monitor metadata and workflow echoes are joined by originating message ID.
  Two ignored nudges count as two `monitor_nudges`; the subsequent launch-health
  record counts separately in the report footer, attributed to the affected
  seat rather than the lead who receives the health notice. A leadless health
  episode uses the originating ignored-nudge IDs. Existing sidecar events keep
  their prior meanings; legacy sidecars carry no identity for a cross-source
  join. The new adapter does not write sidecars.
- Report membership remains task-based: a recent sidecar event, monitor record
  or ledger ruling selects the task, then its launch history supplies role/model
  attribution. This preserves existing task-lifetime metric semantics. A recent
  ruling must not disappear solely because the launch predates the report cut.
  Missing launch attribution remains excluded and disclosed in the footer.
- Mesh GO markers are assignment tokens, with boolean backward compatibility.
  Reason-bearing member blocks do not expire with the activity TTL. See
  [the wait contract](../architecture/data-architecture.md#declared-waits-at-the-taurhaus-deadline-boundary).

## M3 archive reconciliation — oversize value encoding

**M3-WAVE2-31:** The orchestrator's source-verified finding in
[the follow-up brief](../design/m3-oversize-encoding-brief.md) identifies task
#31's sequence-3 ruling as `field: oversize_diff, value: recorded`: a working-diff
breach recorded by the lead, rather than a failed shipping candidate. The old
`is_oversize_failure` predicate matched only `failed`, silently omitting this
encoding. The earlier report-cut repair remains independently covered; it was
not the demonstrated cause of #31's zero count.

The shared predicate now recognizes every `oversize_diff` ruling regardless of
value and excludes all of them from review acceptance. The `oversize_diffs`
column totals attributed rulings. An `Oversize rulings: N total` section prints
one `category | value | count` line per value seen: `failed` means a
shipped-candidate breach, `recorded` a working-diff breach, and `other` preserves
the supplied spelling. Values are JSON-encoded to keep free text on one line;
missing values render as `<missing>`, and non-string values remain visible in
`other`. These counts retain owner-only, ruling-time launch attribution;
ownerless rulings and owners without launch telemetry remain excluded.

The locked-binary fixture emits both `--value failed` and `--value recorded`,
followed by an independently countable `budget_raised` ruling. It proves both
encodings are counted with recent and old launches. No live wave state or
plan-ledger row is read or modified by these tests.

## Runtime attachment and terminal exclusion (v1.1)

Deploy the Taurhaus app and matching daemon that implement `terminalContract: 1`
before opting a team into Mesh team-owned delivery; activation also requires the
paired Mesh contract and its locked stage-2b-or-later binary. Taurhaus skips member
daemon launch, liveness repair and inbox wake for `delivery_owner: "team"`.
Claude's native mailbox remains its delivery path. Launch/resume sends,
stop/interrupt, effort teardown and compaction-test injection share the permanent
`teams/<team>/state/terminal/<member>.lock` flock. Acquisition waits at most 2 s;
terminal children inherit the fd and share a 10 s hold deadline. Contention defers
without clearing input; unavailable flock support refuses terminal I/O and reports
`coordination.terminal.unavailable`. The adjacent `<member>.holder.json` is only
`{owner, op, epoch, since}` diagnostics, replaced by the next acquirer and cleared
on release. Recovery cards and deadline nudges use the shared inbox delivery
seam (journal acceptance on canonical teams) and take no terminal lock. Unit and
contract tests use scratch roots and fake terminal transports; the compaction
transport check is `python3 scripts/test_runtime_exclusion.py`.

A legacy member remains `terminalContract: 0` until a launch atomically publishes
all attachment facts, including Linux process-start ticks. A heartbeat cannot
certify it, and its historical tmux epoch value is not a ticks mismatch. Missing
attachment inventory defers stop/interrupt; unrelated corrupt records do not
block a member that was resolved. The Windows app defers managed terminal writes
to the native daemon without creating lock or holder state on its UNC mount.
Unsupported flock is reported once per path and never permits unlocked I/O.
Install the matching app and daemon together before either writes these records:
old apps cannot decode the new decimal-string `paneStartTime`. The mandated
unchanged protocol number does not guard against that mixed-version deployment.

Member terminal operations and scanner inventory probes use explicit `-S`.
Session bootstrap and emulator attachment helpers still use tmux's ambient
socket resolution (`TMUX`, otherwise `TMUX_TMPDIR` and the effective uid).
Those session-level helpers do not consume a member's recorded socket; custom
bootstrap/emulator socket selection remains outside this member-write change.

### Canonical journal producer

For teams whose config carries `messaging_format: 2`, the Taurhaus daemon
(and its existing native compaction bridge) is a journal producer, not an
inbox writer. The shared delivery seam submits once through `mesh journal
accept --producer taurhaus-daemon`, with the resolved team's `--claude-dir`,
recipient, task links and a delivery-derived idempotency key. Mesh derives the
current assignment from each task; snapshot assignment tokens are not delivery
preconditions. The authenticated actor is the explicit sender or team lead,
while the producer remains `taurhaus-daemon`. Successful capability probes
are cached per binary invocation and resolved root.
Mesh assigns `origin: generated`; acceptance is independent of projection,
transport, read and uptake. `onboarding.delivery.observed` retains the local
card `delivery_id` and the returned `journal.message_id` and
`journal.delivery_id`; compaction bookkeeping retains the same journal receipt.
An unavailable, incompatible, refusing or timed-out Mesh emits
`coordination.journal.accept_failed` with team, recipient, idempotency key and
reason (including recognized Mesh error codes, never CLI prose or credentials).
Canonical delivery never falls back to an array write or resends after a
submission with an uncertain outcome. A preflight or executable-spawn failure
records `Failed` on the matching recovery claim; the existing single bounded
retry retains the same delivery identity. A deadline preflight or executable-spawn
failure releases the nudge claim for the next pass; it does not emit an
unconfirmed-submission event. A cached binary that rejects `journal` at argument
parsing is also a pre-submission failure. A spent deadline nudge with no
confirmed acceptance emits `deadline.nudge.unconfirmed` with team, member,
task and deadline fields. Non-canonical teams keep direct append.
This adapter adds no protocol version, terminal writer, delivery owner or
activation permission; the existing runtime-exclusion contract still applies.

## Stage-5b owned-host evidence — not an eligibility packet

Tempdir fake Unix WebSocket transports pin HTTP Upgrade, masked client frames, fragmented server messages, ping/pong, close and bounded frame refusal. Raw NDJSON EOF is the regression guard for `cadd533e`. Production native eligibility remains disabled pending the paired Mesh packet in [the harness model](../architecture/harness-model.md#owned-codex-hosting-stage-5b-disabled-pending-pairing). The stable `state/app-server/MEMBER.lock` excludes input, effort, compaction, shutdown and publication. Wait is at most two seconds; ordinary operations get five seconds, cold launch thirty. No data lock spans RPC and no lock spans a turn.

The build-pinned attached-TUI path publishes `appServer.attachArgv` and the existing `threadId`, plus normal pane PID/start ticks and tmux address fields. It uses `env -u TMUX`, a private generated `CODEX_HOME` (only auth links to the selected account), `--strict-config` and the resolved launch executable. No account contents or environment secrets are emitted. Terminal holder diagnostics use `attach_tui` / `attach_cleanup` / `detach_tui`; these locks never overlap host RPC or runtime data locks. Pane restart preserves host/thread identity and does not promote a delivery receipt. Tests execute the rendered command only with a fake executable in a foreground private tmux server, then kill and reap that owned server. Descriptor transport and remote-resume evidence are scoped to `0.153.4`; every build bump requires a fresh Upgrade/initialize probe before enablement. The private config and amendment-required strict flag are software-tested integration additions, not part of the trial's runtime PASS. Nonempty/missing host instruction sources and unknown approval enums refuse attach with named errors; absent optional workspace-write fields are omitted from TOML.

Recovery retains `onboarding.delivery.observed` and existing compaction bookkeeping. A native receipt is submission, never read/acceptance. Typed `hostInputUnknown` is persisted before possible input. Classified definite steer rejection clears ambiguity; transport loss does not. Explicit stopped-member reconciliation records `hostInputAbandonedAt`, preserving receipts and abandoning input without replay. IPC operations emit the usual lifecycle spans, correlated with daemon RPC events. Lock contention defers liveness instead of failing the team's pass. Busy-seat shutdown emits `owned hosts not cleanly stopped` at warn level and continues the normal daemon shutdown path.

Controlled pre-eligibility rollback retains `hostRollback`: old/new mode, opt-in, old attachment/build/host, new-generation fence, unresolved attempts and any abandon decision. It requires a stopped child, exact root/session/account and no retained native attempts. Rollback clears the retained TUI pane identity before plain-session relaunch; a failed relaunch leaves a recoverable boundary. Hosted teardown distinguishes `attached TUI closed` from `attached TUI already closed`. Team-owned Mesh switching still requires its paired packet; no tmux delivery is retired by this lane. See the protocol document for additive UI/reconcile methods.

## Native hook bridge — software conformance only

The software seam follows Mesh 4a source `adc9b831756de2fd4282198f4e63a72ba24fe928`;
the integrated binary and uptake trial remain separate evidence.
The native daemon hook bridge pins `mesh-hook-drain/1` and resolves the absolute
Mesh executable through `coordination::mesh_cli`. It queries `delivery capabilities`
and reads `state/delivery/adapter-MEMBER.json` under the team directory. This
bridge-readable path and its `mode`/`revision` fields are pinned contract surface;
`selection_revision` in a returned offer must match that revision. Only an enabled Claude/Codex event
on a canonical, team-owned delivery seat in `hook` mode can drain. Every subprocess
receives explicit account root/team/member argv, a cleared environment with only
`LANG`, one bounded JSON stdin document and EOF. Each child has a two-second total
I/O deadline, 16 KiB request (including routing), and 64 KiB response limits.
Stderr is drained and discarded in bounded 4 KiB chunks. The 8192-byte/8000-scalar context limits
include JSON escaping, delimiters, the reserved recovery card and envelope allowance.
The runtime publishes `hookSessionId` from its captured session and
`contextGeneration` as a string; the compaction counter remains numeric internally.

After closing its output executor, the bridge sends `delivery receipt` with the
original request, offer ID, stage and evidence. `delivery.hook.receipt` records only
team/member/offer ID, stage, receipt success and a bounded reason, never message
bodies or hook prompts/tool arguments. Write+flush proves `hook_response_offered`,
not native acceptance or consumption. Nonzero, timeout, malformed/oversized output
and partial writes yield `outcome_unknown`, without drain retry or tmux fallback.
If no offer ID survived, the bridge records debug `delivery.hook.outcome_unknown`
and skips the uncorrelatable receipt child: Mesh keeps the
reservation quarantined for explicit correlated recovery. Receipt failure after
flush never changes the successful hook result.

The `hook_drain_*` unit fixtures use a Python fake Mesh executable in temporary
roots; `just test-contracts` pins the protocol vocabulary and Windows writer
boundary. This packet proves software behavior only. Mesh's production descriptors
remain disabled; real Claude/Codex uptake, exact paired tips and intended
build/host/config/trust evidence belong to the separately authorized uptake lane.
Claude native-mailbox exclusion still blocks activation in Mesh. agy/Grok and Stop
continuation remain inactive. Daemon protocol 26 excludes protocol-25 readers of
the new string runtime context generation. No transport retirement,
canonical-writer expansion or live deployment is implied.

`hosted.settings.diverged` is a warn event carrying only the owned thread ID,
never settings, instruction text or credentials. It triggers one bounded named
resume to restore model/effort/approval/sandbox parity before completing the
operation. A failed or divergent repair fails closed and remains pending for the
next operation. Attach, cleanup and detach resolve the same registered team root
for terminal exclusion. Host relaunch preserves the recorded pane identity until
ownership-checked reuse or retirement; it does not leave an unrecorded old view.
