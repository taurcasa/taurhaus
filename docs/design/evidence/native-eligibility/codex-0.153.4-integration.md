# Codex 0.153.4 integration trial — INCONCLUSIVE at step 1 setup

2026-09-09. **Eligibility remains disabled.** The prescribed canonical setup
stopped before member launch: the real Mesh command `team delivery --owner team`
refused the newly created team's active external lead because it had no runtime
record. This is a setup failure, **not an observed app-server or attached-TUI
failure**. No Codex model session was launched. Steps 2–7 were not run.

Pair: Taurhaus `9d3589355a11199ec2b3317bd1a13e784e64cd6f`, branch
`feat/integration-trial`, daemon protocol **26**; Mesh
`4388d6a1590e3072c9dfdc61ccd08b00bff2508b`, branch `feat/native-push`.
The temporary compiled Mesh change set only the 0.153.4 descriptor to
`disposition: "trial", enabled: true`; all other descriptor fields and builds
were unchanged. The edit was reverted after the failure. **No descriptor flip
commit exists. No Taurhaus registry entry was needed or added:** the existing
`HostedDescriptor::codex()` already names build 0.153.4 and Unix WebSockets.

S = observed runtime; I = source-based inference; U = unverified.
Retained text captures/logs omit trailing empty lines; the event JSONL preserves
full pane output including blank terminal rows.

| Brief step | Outcome | Evidence / missing observation |
| --- | --- | --- |
| 1. Daemon launches host and attached TUI | **INCONCLUSIVE: setup FAIL (S)** | Private daemon ping reports protocol 26; RPC launches the canonical setup script; Mesh creates the team, then refuses delivery-owner verification for missing `runtime/lead.json`. No member/thread/host attachment is published. |
| 2. Idle send → start, visible marker, receipt and explicit read | **NOT RUN (U)** | No thread, native request, receipt, journal consumption or marker reply. |
| 3. Active send → steer | **NOT RUN (U)** | No active turn or expectedTurnId. |
| 4. Operator/socket ordering and mutual exclusion | **NOT RUN (U)** | No host-operation holder exists; the retained `owner.lock` is a Mesh delivery-owner lock, not host-operation evidence. |
| 5. Compaction and recovery card | **NOT RUN (U)** | No compaction or recovery delivery. |
| 6. Daemon restart and retained identity | **NOT RUN (U)** | Fixture retry uses a new scratch root; it is not the restart experiment. |
| 7. Rollback and tmux delivery | **NOT RUN (U)** | No hosted member to roll back. Cleanup of the processes actually started passed separately. |

## Exact failure and identities

Authoritative runtime sidecars: [events](integration/pre-defect/events.jsonl),
[setup pane](integration/pre-defect/setup-pane-1.txt),
[daemon log](integration/pre-defect/daemon.log),
[team config](integration/pre-defect/team/config.json),
[setup commands](integration/pre-defect/setup-command.json).
The `pre-defect` directory name means that neither recorded product defect was
reached; it does not assert that a defect was reproduced.

Scratch root `/tmp/th-int-ssxg881f`, private port **26417**. Team `integration`,
lead `lead`, member agent ID `036a904f-5221-49e7-a756-82415a4e9ec1`, team incarnation
`4d7d1c2aeaa832a131697fc0e5cdd5ad971b25f98b3e27e90773fdc115d94e2b`.
The config's leadSessionId is Mesh bootstrap metadata, **not a Codex thread ID**.
Daemon request `88aaa715752816c7` returned:

```json
{"data_root":"/tmp/th-int-ssxg881f/data","protocol_version":26,"uptime_secs":0,"version":"0.9.7"}
```

Request `a9a6ba191db6a8e9`, method `launch_session`, ran the setup script in the
scratch project and returned pane `%1`, session `taurhaus`, window `project`.
The daemon RPC was used to execute the canonical Mesh CLI commands; the typed
`coordination.create_team` RPC only creates a noncanonical empty config and
does not accept the specified retention/canonical flags.

```sh
/tmp/th-int-ssxg881f/home/.local/bin/mesh --claude-dir /tmp/th-int-ssxg881f/claude --team integration --name lead team create --messaging-canonical --isolated --retention-policy /tmp/th-int-ssxg881f/policy.json
/tmp/th-int-ssxg881f/home/.local/bin/mesh --claude-dir /tmp/th-int-ssxg881f/claude --team integration --name lead team delivery --owner team
```

Pane capture, verbatim:

```text
bootstrapped new team config
[mesh] lead joined team integration
error: IO error: delivery: pending: runtime lead: IO error: No such file or directory (os error 2)
```

The setup shell used `set -eu` and never wrote its success sentinel. The
controller exited **1**. The daemon's launch RPC returned successfully before
the setup command completed, so it exposes no shell exit status; the exact Mesh
exit code was not separately captured. The config already contains
`delivery_owner: "team"` from canonical creation; that field does not prove the
following owner command passed or a dispatcher started. The epoch artifact is
retained under [state/delivery](integration/pre-defect/team/state/delivery/epoch.json).

I: Mesh `src/delivery/runtime.rs::compatible_team` reads a runtime record for
every active member and requires `terminalContract: 1`. The canonical creator
registers an active external lead but does not supply its runtime. This explains
the observed refusal; it does not prove that an appropriately launched roster
cannot work. No forged runtime record, alternate setup ordering, or additional
product repair was used to bypass the failed prerequisite.

No `mesh send` was executed, so there are **no wire start/steer excerpts,
native_enqueued receipts, consumed_by_read journal rows, host-operation holder
files or recovery cards to report**. Their absence is not a passing assertion.
The random marker reserved for this attempt was `5f19c4057b8d614a`. It was never
submitted to a model.

## Cost and isolation

| Attempt | Model turns / generations / compactions | Spend |
| --- | --- | --- |
| Initial private-tmux fixture attempt | 0 / 0 / 0 | $0.00 |
| Corrected private-server setup | 0 / 0 / 0 | $0.00 |
| Total against hard caps | **0 / 8 model turns** | **$0.00 / $3.00** |

Ledgers: [initial](integration/initial/cost-ledger.json),
[corrected setup](integration/pre-defect/cost-ledger.json). Only the pinned
binary's `--version` ran; neither member launch RPC nor paid input was reached.
There is no estimated bill or unreported compaction spend in this run.
Model/effort planned and generated in the scratch config: `gpt-5.6-luna`, `low`.

Both roots were created with mode 0700 beneath `/tmp`. The sole credential copy
was `auth.json`, mode 0600, into an initially empty CODEX_HOME. No source config,
sessions, hooks, history, MCP definitions or skills were copied. Scratch HOME,
all harness/data roots, TMPDIR and TMUX_TMPDIR were explicit; TMUX was absent.
The scratch project was a fresh git repository with this short AGENTS.md:

```text
This is an isolated transport trial. Reply briefly. Never execute tools or commands.
```

Bubblewrap hid `/home`, `/tmp` and `/run`, exposed only the writable scratch root,
and provided a private PID namespace. The native Codex executable and both built
binaries were copied into scratch `$HOME/.local/bin`; their SHA-256 identities
are in the event logs. The reference Node wrapper was replaced by a copy of its
native executable, verified as `codex-cli 0.153.4`, avoiding exposure of any
operator installation directory. Non-Codex harness executables were blocked.
No real operator tmux or port 17233 was contacted. No installation recipe ran.

Cleanup used the controller's finally block and only its own process groups;
ending the private PID namespace removed daemon/tmux descendants. The initial
root `/tmp/th-int-z4u32ik_` used port **30356**. Both ports refused connections,
both roots and credential copies were removed, and environment-token scans
found no survivors. PID/start-tick inventories and verification:
[initial](integration/initial/identities.json),
[corrected setup](integration/pre-defect/identities.json),
[initial cleanup](integration/initial/cleanup.json),
[corrected cleanup](integration/pre-defect/cleanup.json).

## Reproduction, gates and limits

[controller.py](integration/controller.py) reproduces the failed setup and
always cleans up. Archived controllers preserve the initial and corrected
runtime sequences; later controller edits only redact a credential hash from
evidence, express the policy source relative to the checkout's parent to avoid
an operator-home path, and record the generated setup script. They did not rerun
the trial. Archived scripts must be restored to `integration/controller.py` to
replay their original output-directory placement.
Run the controller from this checkout root with a new output label:

```sh
python3 docs/design/evidence/native-eligibility/integration/controller.py NEW_LABEL
```

Before reproduction, build both pinned binaries as in
[daemon build](integration/daemon-build-retry.json) and
[Mesh build](integration/mesh-build.json), applying only the authorized temporary
descriptor edit. Every build first polled `pgrep -af '(^|/)cargo( |$)'` until exit
1, with a 30-minute limit; both observed no competing Cargo. The daemon used
checkout-local `src-tauri/target`, Mesh used its own `target`. The first daemon
build exited 101 because resource placeholders were absent; `just
ensure-tauri-resources` exited 0 and the repeated `cargo build --bin
taurhaus-daemon` exited 0. Mesh `cargo build --bin mesh` exited 0. Revert with:

```sh
git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs
```

The required unpaid gate runner is [gates.py](integration/gates.py): separate
credential-free HOME/harness roots, rejecting CLI/tmux/Mesh shims, private PID
namespace, checkout-local target, and no real harness homes visible. Tool cache
paths in retained logs are redacted. `bun install --frozen-lockfile` exited 0.
Gate exits are recorded in `integration/gate-{check-quick,lint,test-contracts}.json`.

| Exact gate from checkout root | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust tests compile; typecheck passes; 150 frontend files / 2,469 tests pass. |
| `just lint` | **0** | Rust/frontend/workflow and gate-recipe guards pass. |
| `just test-contracts` | **0** | 68 Rust contracts pass (15 + 20 + 33). |
| `just test-rust-unit` | N/A | No `src-tauri/` diff. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A | Conditional on an eligibility flip; none was made. |

The final PID/start-tick, port, descriptor and evidence-hygiene audit is retained
in [final-verification.json](integration/final-verification.json).

No product code changed, so no regression tests were added or red/green claimed;
`just test-rust-unit` is not required. The instruction-source refusal and
rollback pane leak remain untouched because member launch and rollback were
never exercised. Non-evidence inserted lines: **0 / 400**. No dependency changes,
ledger edits, registry additions or feature work.

Deviations: one credential-free tmux setup retry (the initial daemon-created
server disappeared before readiness); the corrected fixture explicitly retained
`tmux -D -f /dev/null`, matching the reference trial. Initial gate invocations
exited 127 because Node was missing inside their wrapper; the wrapper was
corrected with a scratch copy of Node and all exact gates rerun. No continuation
after the canonical-owner refusal. No numbered step was green, so the deliverable
is one evidence commit rather than seven step commits. The requested Opus evidence
lens was unavailable in this session's model/tool surface and was not replaced
with a claimed cross-family review. A new authorized trial must resolve the
canonical setup prerequisite before any eligibility conclusion.
