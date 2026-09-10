# Codex 0.153.4 integration — IN PROGRESS (attempt 8)

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

## Attempt 2 — production initialization passes; step 1 setup remains INCONCLUSIVE

2026-09-10 (Europe/Berlin; runtime logs use 2026-09-09 23:14 UTC).
**No eligibility enabled. No app-server input was exercised.** Production
`coordination.initialize_team` successfully created the canonical team, launched
both seats, and then completed `opt_in_delivery`. The controller next set the
scratch member's `adapter_mode` to `app_server` and called
`coordination.resume_member` while that seat was online. The RPC refused at
`load_member`: **`Conflict: member 'seat' in team 'integration' is not offline`**.
Controller exit **1**; it stopped immediately and restored the descriptor.

This is a **controller/setup limitation, not evidence of an app-server launch
failure**. In particular, it does not establish that stop-then-resume would fail:
that sequence was not run after the first refusal. The live seat never reached
`HostedMembers::launch`, and neither recorded product defect was exercised.
The code's later `app_server_switch_requires_5b_recoverable_relaunch_packet`
guard is source evidence only, not this run's observed refusal.

Pair: Taurhaus **`0f1a6e90`**, based on canonical activation **`06d1267b`**
(PR #157), protocol **27**, branch `feat/integration-trial`; Mesh
**`4388d6a1590e3072c9dfdc61ccd08b00bff2508b`**, branch `feat/native-push`.
Both binaries were rebuilt from these worktrees with checkout-local targets.
The temporary compiled descriptor changed exactly the 0.153.4 disposition to
`trial` and enabled it, leaving host/configuration/trust and all other builds
unchanged. [Exact diff](integration/attempt2/mesh-trial-descriptor.diff).
The descriptor has been reverted; **Mesh flip commit: none**.

| Brief step | Attempt 2 outcome | S-runtime evidence / missing proof |
| --- | --- | --- |
| 1. Daemon-owned app-server + attached strict TUI | **INCONCLUSIVE: setup refusal** | Production initialization passes all nine stages. Subsequent resume refuses the already-online member at `load_member`; the runtime has a regular pane, no `appServer`, and no discovered session/thread ID. |
| 2. Idle send → start, marker, receipt, explicit read | **NOT RUN** | No `mesh send`, `turn/start`, marker reply, `native_enqueued`, or explicit read. |
| 3. Active send → steer | **NOT RUN** | No active model turn or expectedTurnId. |
| 4. Operator/socket ordering + mutual exclusion | **NOT RUN** | No operator input or host-operation holder. Mesh owner.lock is not host-operation evidence. |
| 5. Compaction + recovery card | **NOT RUN** | Startup inbox acceptance exists; it does not prove compaction recovery or model uptake. |
| 6. Daemon restart | **NOT RUN** | No restart or persistent hosted identity. |
| 7. Rollback + tmux delivery | **NOT RUN** | No hosted attachment to roll back. Separate cleanup of the started processes passed. |

### Runtime sequence and identities

[Executed controller](integration/attempt2-controller.py),
[event JSONL](integration/attempt2/run/events.jsonl),
[initialize result](integration/attempt2/run/initialize-result.json),
[resume result](integration/attempt2/run/resume-result.json),
[daemon log](integration/attempt2/run/daemon.log),
[structured log](integration/attempt2/run/taurhaus.log.jsonl).

Scratch root **`/tmp/th-int-ik66igdt`**, private port **31164**, team
`integration`, lead `lead`, member `seat`. Scratch project is a fresh committed
git repository containing the short `AGENTS.md` shown in attempt 1; creation
commands are `events.jsonl:5–10`. No operator repository content was copied.
Random reserved marker **`990084b281d9d0ea`** was never submitted.

The exact `coordination.initialize_team` request is `events.jsonl:18`, request
ID **`66967cbcee57b97d`**. Its messaging object was parsed directly from
`DEFAULT_CANONICAL_POLICY` in `src/lib/components/meshTabUtils.js`, with no policy
substitution; [retained policy](integration/attempt2/run/policy.json).
Lead: Claude `claude-haiku-4-5` on credential-free scratch CLAUDE_CONFIG_DIR.
Member: Codex **0.153.4**, **gpt-5.6-luna**, effort **low**, read-only sandbox,
never approvals. Binary SHA-256 identities are `events.jsonl:1–4`.

Initialization run **`init_ea40f09e6e724233a3e98cd30046b847`** returned:

```json
{"failed_step":null,"message":"team initialized","succeeded_steps":["validate_configuration","create_team","add_lead","create_panes","launch_sessions","join_mesh","start_daemons","opt_in_delivery","send_onboarding"]}
```

The lead reached its first-run theme screen, not a model turn:
[lead capture](integration/attempt2/run/initialize-pane-1.txt).
The Codex pane showed v0.153.4 with `model: loading` and an empty input:
[member capture](integration/attempt2/run/initialize-pane-2.txt).
The production rendered commands are `taurhaus.log.jsonl:12–13`.
Both runtimes have `terminalContract: 1`, attachment generation 1,
context generation `"0"`, and null `session_id`; neither has `appServer`.
Member pane **`%2`**, pane PID **134** inside the private PID namespace,
start ticks **21621355**; lead pane **`%1`**, PID **115**, start ticks
**21621350**. These namespace PIDs are distinct from the host PID inventory.
[Member runtime](integration/attempt2/run/team/runtime/seat.json),
[lead runtime](integration/attempt2/run/team/runtime/lead.json).

Team incarnation
**`c7ac345bdeb68789a58744da96d42a955d16f8ab2bcc6e2eb5ffbc83e27d4637`**;
member incarnation **`c836b70c-b762-4c0e-b31c-fd1b37b2a950`**.
The config's bootstrap leadSessionId is not a Codex thread ID.

After the scratch config opt-in (`events.jsonl:30`), request
**`7d1b699ded323c7e`** called `coordination.resume_member`; run
**`resume_e48c1ef9a1e949cfb4ae14e6548d380c`** returned:

```json
{"failed_step":"load_member","resumed":false,"pane_id":null,"message":"Conflict: member 'seat' in team 'integration' is not offline"}
```

`events.jsonl:42` records the stop. There was no retry, pane identity deletion,
runtime forgery, or continued numbered step. AgentDefinition has no adapter-mode
field and `member_from_agent_setup` initializes `extra` empty; therefore the
production initialize request itself did not opt this seat into hosting. The
subsequent config-plus-resume attempt did not satisfy the launch prerequisites.

### Receipts, journal references and spend

The two startup cards reached **inbox acceptance only**. For `seat`, structured
log line 27 and its runtime record retain this journal reference:

```json
{"message_id":"8940e5ae-bb88-4d91-b705-2b49d67aa484","delivery_id":"918588c2-009d-4b3d-9205-16b797cab5bf","sequence":2,"projection":"pending"}
```

Associated observation: `stage: accepted`, `path: inbox`, `accepted_bytes: 2273`,
`offered_bytes: 0`, `returned_by_read_bytes: 0`. The member dispatcher health
records **`activity not freshly idle`**, zero completed deliveries and zero
failures. These are runtime/journal-reference excerpts, **not exported original
canonical journal rows**: the inherited snapshot glob did not retain those rows.
No `consumed_by_read` or native enqueue is claimed. No app-server wire frames or
host-operation holder files exist in this attempt's evidence.

| Spend item | Model turns / generations / compactions | USD |
| --- | --- | --- |
| Scratch Claude first-run screen | 0 / 0 / 0 | $0.00 |
| Scratch Codex launch, still loading; onboarding pending | 0 observed / 0 observed / 0 | $0.00 model spend observed |
| Controller input or compaction | 0 / 0 / 0 | $0.00 |
| Attempt 2 total versus caps | **0 / 8 observed model turns** | **$0.00 / $3.00 observed** |

[Cost ledger](integration/attempt2/run/cost-ledger.json),
[usage-event export](integration/attempt2/run/usage-events.json).
No work prompt, TUI Enter, native input, or compaction was requested. The usage
export is empty, session IDs remain null, and delivery remained pending before
teardown. No vendor billing statement was queried; $0 is the observed no-turn
result, not a claimed subscription invoice measurement. There were no paid
reruns. Attempt 1 plus attempt 2 remain at zero observed model turns.

### Isolation, cleanup and reproduction

Scratch root mode 0700; CODEX_HOME started with only copied `auth.json` mode
0600 and then a generated minimal config. HOME and all harness/data roots were
scratch, inherited TMUX was absent, TMUX_TMPDIR pointed to the private server.
Bubblewrap hid `/home`, `/tmp`, `/run`, exposed only the writable scratch root,
and put the real daemon, Mesh, Claude, Codex and tmux in a disposable PID
namespace. All executables were copied into scratch `$HOME/.local/bin` and no
operator installation directory was exposed to the child namespace.

The controller's finally block stopped and waited its own process groups,
ending the descendant PID namespace. The ten recorded host PID/start-tick
identities are in [identities.json](integration/attempt2/run/identities.json).
[Cleanup](integration/attempt2/run/cleanup.json): no surviving scratch processes,
private port closed, root and auth copy removed. Descriptor restoration command
exited **0**, and the Mesh worktree is clean. The native trial binary remained
only as an unused build artifact in the authorized Mesh target; its scratch
executable copy was deleted. No standing daemon, server or team was contacted.

Exact reproduction of this **failed setup controller**, not a passing seven-step
trial, from the specified checkout root:

```sh
python3 docs/design/evidence/native-eligibility/integration/attempt2-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt2-controller.py attempt2/NEW_RUN
python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

Do not treat the inspection checkpoint in the controller as seven-step coverage:
that branch was not reached. A future commissioned run must resolve the hosted
activation route before submitting model work.

Both builds first ran `pgrep -af '(^|/)cargo( |$)'`; each returned **1**, no Cargo
running. Daemon `cargo build --bin taurhaus-daemon` and Mesh `cargo build --bin
mesh` each exited **0**, using `src-tauri/target` and Mesh `target` respectively.
[Daemon build](integration/attempt2/daemon-build.json),
[Mesh build](integration/attempt2/mesh-build.json).
No install recipe or forbidden daemon port was used.

### Attempt 2 gates and deviations

All exact gates ran from this checkout root in the credential-free
[gate controller](integration/attempt2-gates.py): private HOME, harness roots,
PID namespace, rejecting CLI/tmux/Mesh shims and checkout-local Rust target.
Each Cargo preflight returned 1 (no competing Cargo).

| Exact gate | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, frontend typecheck; 150 frontend files / 2,486 tests passed. |
| `just lint` | **0** | Rust, frontend, workflow and recipe guards passed. |
| `just test-contracts` | **0** | All 68 Rust contract tests passed. |
| `just test-rust-unit` | N/A | No `src-tauri/` changes. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A | Conditional on a passing flip; no flip was made. |

Gate logs and exit metadata are under [attempt2/gates](integration/attempt2/gates).
[Final verification](integration/attempt2/final-verification.json) checks retained
identities, port/socket/root cleanup, descriptor restoration, evidence hygiene,
and the production initialize ordering.

No product code changed, no new dependency or registry entry was needed, and
non-evidence inserted lines are **0 / 400**. Neither product defect was reached,
so neither was fixed and no regression red/green is claimed. The existing
`HostedDescriptor::codex()` already covers the pinned build and transport.

Deviations and limits:

- The controller used scratch config opt-in followed by resume on a live pane.
  That invalid activation sequence stopped before hosting; it is not a product
  regression or a conclusive native eligibility test. No additional activation
  repair was authorized within the two-defect product scope.
- Original canonical journal rows were not exported by the inherited glob;
  retained runtime and logging records contain journal references and inbox
  acceptance observations only. There is no wire/native-receipt evidence.
- There was no completed numbered trial step, so this is one evidence commit,
  not seven green-step commits. Attempt 1 remains intact above.
- An Opus evidence lens was unavailable in the exposed model/tool surface;
  no cross-family review is claimed. Final verification is a local artifact
  audit, not that independent lens.
- No continuation after the first refusal; no changes to plan ledger rows,
  deployment, release, installation, or descriptor eligibility.

Retained text pane captures and the contract-test log omit trailing blank lines;
full pane output remains in the event JSONL.

## Attempt 2 continuation — stop/reconcile succeeds; step 1 host switch refused

2026-09-10, newly authorized by the user's instruction to continue from the
current tree. Prior results and their commits remain intact. **Step 1 FAIL at
`launch_host`; native eligibility remains disabled.** This continuation corrects
the previous online-seat resume mistake: it calls the production `stop_session`
RPC, waits for that owned pane to disappear, then calls
`coordination.reconcile_live_presence` and verifies `health: session_dead` before
setting the scratch member opt-in and resuming. The resume now passes
`load_member` and reaches the host, which refuses:

```text
Conflict: app_server_switch_requires_5b_recoverable_relaunch_packet
```

This is an observed activation guard, not the previous controller prerequisite
failure. It still does not test the app-server process or attached TUI: refusal
precedes `HostProcess::launch`. No identity was erased by the controller to get
past it. The user limited product changes to the two recorded defects (plus a
registry entry if needed); a recoverable activation/switch packet is additional
product scope. The trial stopped here, as the original stop-on-failure rule
requires. Neither instruction-source refusal nor rollback pane leak was reached.

| Step | Continuation outcome | Evidence |
| --- | --- | --- |
| 1. Daemon-owned host and attached strict TUI | **FAIL: activation refused** | Canonical initialize passes; daemon stops the pane and reconciles offline; resume reaches `launch_host` and returns the named switch-packet refusal. No host/thread/socket published. |
| 2. Idle Mesh input and explicit read | **NOT RUN** | No send, native input, marker reply or native receipt. |
| 3. Active steer | **NOT RUN** | No model turn or expectedTurnId. |
| 4. Operator/socket interleave and host exclusion | **NOT RUN** | Stop used terminal exclusion; this is not proof of mutual host-operation exclusion. |
| 5. Compaction | **NOT RUN** | Startup inbox acceptance does not establish compaction recovery. |
| 6. Restart | **NOT RUN** | Fresh scratch run is not the daemon restart experiment. |
| 7. Rollback | **NOT RUN** | No hosted attachment. Final owned-process cleanup passes separately. |

### Continuation commands and S-runtime identities

[Exact controller](integration/continuation-controller.py),
[event JSONL](integration/attempt2-continuation/run/events.jsonl),
[initialize report](integration/attempt2-continuation/run/initialize-result.json),
[resume report](integration/attempt2-continuation/run/resume-result.json),
[daemon log](integration/attempt2-continuation/run/daemon.log).
Source pair: Taurhaus **`6ff97a25`** / canonical implementation **`06d1267b`**,
protocol **27**; Mesh **`4388d6a`**. Both rebuilt locally; both builds exit **0**,
each Cargo preflight exit **1**, no competing Cargo. Temporary descriptor diff
still changes only 0.153.4 `disposition: trial`, `enabled: true`.

Scratch root **`/tmp/th-int-4mx35359`**, private port **30058**; marker
**`68573aa36d40ed95`**, never submitted. The same credential isolation, private
tmux and PID namespace were used. The member project again contains a committed
short AGENTS.md; only auth.json was copied into the otherwise-empty Codex home.
Member model **gpt-5.6-luna**, effort **low**; Claude lead remains credential-free
and takes no model turn. Binary digests are `events.jsonl:1–4`.

Exact operation order and request IDs:

1. `coordination.initialize_team`, request `50031efd220fdbcb`, run
   `init_3de2ad0682bf4320a7e2210ddb8fa9e7` — all nine stages succeeded, including
   launch before delivery opt-in (`events.jsonl:18–23`). Uses the unchanged
   builder `DEFAULT_CANONICAL_POLICY` through the production RPC.
2. `stop_session`, request `f1d2cf681888d633`, params
   `{"tmux_pane":"%2","cli_tool":"codex"}` (`events.jsonl:30`). Private tmux
   pane enumeration then confirms `%2` absent. The daemon's graceful-exit and
   pane teardown messages are in the daemon log.
3. `coordination.reconcile_live_presence`, request `4d905e5cd619d95e`, params
   `{"team_name":"integration"}` (`events.jsonl:42`). Saved
   [before-stop](integration/attempt2-continuation/run/before-stop.json) and
   [after-stop](integration/attempt2-continuation/run/after-stop.json) records show
   health changed from `healthy` to `session_dead`, while paneId **`%2`**, panePid
   **134**, paneStartTime **21676363**, terminalContract **1** and attachment
   generation **1** were retained. Session ID is null, appServer absent.
4. Scratch member config `adapter_mode: app_server`, then
   `coordination.resume_member`, request `7e6dd1af11606534`, run
   `resume_9047a57f21c549a998fc512834b5cdb0` (`events.jsonl:46`). Result:

```json
{"failed_step":"launch_host","succeeded_steps":["validate","load_member"],"resumed":false,"message":"Conflict: app_server_switch_requires_5b_recoverable_relaunch_packet"}
```

`events.jsonl:55` records the immediate stop. The source explanation is now
corroborated at runtime: `HostedMembers::launch` rejects a record with no
appServer and an existing pane or session identity. Production reconciliation
retains the old paneId even after that pane is gone. No guard was removed, no
runtime record fabricated, and no additional feature was implemented.

### Continuation receipts, cost and cleanup

Only startup inbox observations exist. The member runtime's journal reference is
message **`51b5a0fe-5c17-4002-957d-3085f1415615`**, delivery
**`984ab1be-73d6-454b-b00e-42d69bd5d69f`**, sequence **2**, projection `pending`,
stage `accepted`, path `inbox`, 2,273 accepted bytes, zero offered/read bytes.
No native wire, native enqueue, explicit read, compaction card, or host-operation
holder evidence was generated. Journal references are not original journal rows.

[Cost ledger](integration/attempt2-continuation/run/cost-ledger.json): **0 observed
Codex turns / $0 observed model spend**, **0 Claude turns / $0**, **0 compactions**.
The [usage-event export](integration/attempt2-continuation/run/usage-events.json)
is empty. No work prompt or model-turn submission was sent. Across attempt 1,
attempt 2 and this continuation: **0 observed turns / $0**, versus caps of 8 and
$3. No subscription invoice measurement is claimed.

Controller exit **1**. Its finally block restored the Mesh descriptor with the
specified `git checkout -- src/delivery/app_server/capabilities.rs` command,
exit **0**, stopped and waited its own process groups, ended the private PID
namespace and removed the scratch root/auth copy. Eight remaining owned process
identities were recorded before final cleanup; the member pane had already
closed through `stop_session`. [Cleanup](integration/attempt2-continuation/run/cleanup.json)
reports no survivors and the private port closed. Both worktrees remain on the
specified branches; no operator daemon, tmux server, home or standing team was used.

Reproduction from this checkout root (the controller stops at the guard):

```sh
TRIAL_EVIDENCE_LABEL=attempt2-continuation python3 docs/design/evidence/native-eligibility/integration/attempt2-build.py
python3 docs/design/evidence/native-eligibility/integration/continuation-controller.py attempt2-continuation/NEW_RUN
TRIAL_EVIDENCE_LABEL=attempt2-continuation python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

The optional evidence label on the build/gate helpers preserves previous results;
default behavior is unchanged. Original attempt 2 controller remains unchanged.

### Continuation gates and final disposition

| Exact gate from checkout root | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, typecheck, 150 frontend files / 2,486 tests passed. |
| `just lint` | **0** | All lint and recipe guards passed. |
| `just test-contracts` | **0** | All 68 contract tests passed. |
| `just test-rust-unit` | N/A | No `src-tauri/` diff. |
| Mesh gates | N/A | No passing eligibility flip. |

[Gate artifacts](integration/attempt2-continuation/gates),
[final cleanup/evidence audit](integration/attempt2-continuation/final-verification.json).
The read-only audit verifies offline reconciliation, retained pane identity,
named host refusal, PID/start-tick absence, closed private port/socket, removed
roots/auth copy, clean Mesh tree, and artifact hygiene. Text captures and logs
omit trailing blank lines; complete captured pane bytes remain in event JSONL.

**Descriptor flip: none. Mesh commit: none. Registry entry: not needed.** Product
code changes **0**, non-evidence insertions **0 / 400**, tests added **0**, regression
red/green **not applicable**: neither authorized defect was reached or fixed.
The missing activation packet cannot be supplied by either requested defect fix.
No runtime/native attachment or eligibility tuple was fabricated to bypass it.

Continuation deviation: one newly authorized corrected setup run after the
previous turn's stop, using production stop/reconcile before resume. This run
stopped at its first failed stage. No numbered trial step was green; the completed
deliverable is an evidence commit with the requested co-author trailer. The
independent Opus lens remains unavailable. Original canonical journal rows and
native wire receipts remain unproved; no model spend or uptake is inferred from
startup inbox acceptance. No plan ledger, deployment or install changes.

## Attempt 3 — fresh hosted seat; startup recovery rejected (2026-09-10)

**Step 1 FAIL.** The production `coordination.initialize_team` request carried
`delivery: "app_server"` on the fresh Codex seat. The daemon reached the real
host, created thread **`01a088d7-f6e4-74c3-b4d2-8ab1b821510f`**, and logged one
loaded instruction source. Startup recovery then failed with the exact public
refusal **`Conflict: host rejected request; reconcile before retrying`**.
No retry or later numbered step ran. Mesh eligibility remains disabled; no
product code changed in either repository.

S = observed runtime/artifact; I = source-based inference; U = unverified.

| Step | Outcome | S-runtime evidence / missing proof |
| --- | --- | --- |
| 1. Daemon host + strict attached TUI | **FAIL: `launch_host` rejected** | [Initialize report](integration/attempt3/run/initialize-result.json), [structured log](integration/attempt3/run/taurhaus.log.jsonl): real thread and instruction-source count, then failed startup card. No attached member pane, published attachment snapshot, or terminalContract 1 proof survived. |
| 2. Idle `mesh send`, native receipt, explicit read | **NOT RUN** | No send/read, marker reply, native enqueue or consumption. |
| 3. Active `turn/steer` | **NOT RUN** | No model turn or expectedTurnId. |
| 4. Operator input + host-lock interleave | **NOT RUN** | No operator input or retained holder file; no mutual exclusion claim. |
| 5. Compaction | **NOT RUN** | Failed startup recovery is not a compaction trial. |
| 6. Daemon restart | **NOT RUN** | No restart or recovery claim. |
| 7. Operational rollback + tmux send | **NOT RUN** | No rollback RPC, refusal probe, remove/add or tmux delivery. Final teardown passed separately. |

### Executed pair, isolation and commands

Taurhaus source **`de71587e`**, product base **`80290f36`** (#158), branch
`feat/integration-trial`; Mesh **`4388d6a`**, branch `feat/native-push`.
The daemon's real `ping` returned protocol **27**, version **0.9.7**.
Both checkout-local builds exited **0**, each after
`pgrep -af '(^|/)cargo( |$)'` returned **1** (no competing Cargo):

```sh
# Under /home/mstie/projects/taurhaus-trial/src-tauri, target=./target
cargo build --bin taurhaus-daemon
# Under /home/mstie/projects/mesh-push, target=./target
cargo build --bin mesh
```

[Build controller](integration/attempt3-build.py),
[daemon build metadata](integration/attempt3/daemon-build.json),
[Mesh build metadata](integration/attempt3/mesh-build.json),
[exact temporary descriptor diff](integration/attempt3/mesh-trial-descriptor.diff).
Only the 0.153.4 descriptor was temporarily admitted, with `disposition: "trial"`,
`enabled: true`, transport `unix-websocket`, and the three strings verified
against this checkout's `coordination/hosted.rs`:
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1`.
The wildcard and other harness descriptors remained disabled.

[Executed runtime controller](integration/attempt3-controller.py) copied those
binaries only into scratch `$HOME/.local/bin`. SHA-256 identities are the first
four rows of [events.jsonl](integration/attempt3/run/events.jsonl). The isolated
version probe returned **codex-cli 0.153.4**; the unused Claude lead was **2.1.267**.
Scratch root **`/tmp/th-int-h0o7f0s_`** was mode 0700; all writable homes/data roots
were beneath it. CODEX_HOME began with only the authorized auth.json copy, mode
0600, then received generated config: Luna, low effort, read-only sandbox, never
approvals, disabled web search, 32768 context window and scratch-project trust.
No operator config, hooks, history or sessions were imported. Auth/token contents
were never logged or exported.

Bubblewrap hid `/home`, `/tmp`, `/run`, re-exposed only the writable scratch root,
and supplied a private PID namespace. A credential-free Claude lead and one
Codex seat used a new committed scratch git project with this entire AGENTS.md:

```text
This is an isolated transport trial. Reply briefly. Never execute tools or commands.
```

The private tmux server used `TMUX_TMPDIR=/tmp/th-int-h0o7f0s_/tmux`, inherited
TMUX absent, `tmux -D -f /dev/null`; all capture commands used that environment.
The private daemon bound the successfully probed **28528** port, with explicit
`--port 28528 --data-dir /tmp/th-int-h0o7f0s_/data`. Its token was used only on
that private RPC connection. No installed daemon or operator tmux was contacted.

Initialization request **`e3ae3202b2a588cc`**, run
**`init_fd5d137eb6fe4dd5a223b3aee53acd3c`**, is retained in `events.jsonl:18`.
Its messaging policy was parsed directly from the builder's unchanged
`DEFAULT_CANONICAL_POLICY`; [policy.json](integration/attempt3/run/policy.json)
retains it. [Team config](integration/attempt3/run/team/config.json) proves
`messaging_format: 2`, `delivery_owner: "team"`, `minimum_writer: "mesh-journal/2"`
and the freshly created seat's `adapter_mode: "app_server"`. There was no
post-launch config switch or resume attempt.

### Failure, receipt and journal excerpts

Final status request **`a418df344fbcd848`** returned:

```json
{"failed_step":"launch_host","message":"Conflict: host rejected request; reconcile before retrying","retryable":true,"succeeded_steps":["validate_configuration","create_team","add_lead"]}
```

S: `taurhaus.log.jsonl:17` records `hosted.instruction_sources.loaded`, count **1**,
thread **`01a088d7-f6e4-74c3-b4d2-8ab1b821510f`**.
At line 20, `onboarding.delivery.observed` records:

```json
{"delivery_id":"9b0be71718566219a263ac974d6742889cd84736530f245314a4f0cd34d7aebd","kind":"baseline","path":"app_server","stage":"failed","generated_bytes":2172,"accepted_bytes":0,"offered_bytes":0,"returned_by_read_bytes":0}
```

Card context is `[1,0]`, member incarnation
`e469b72e-2b23-424a-940b-cb3b3e2396f6`, team incarnation
`86317ec32ff7c286e992383466edb027da8c1508ca9afbb373df2d0181e921df`.
The original [canonical segment](integration/attempt3/run/team/state/messaging-v2/segments/000001.jsonl)
was retained and is **empty (0 bytes)**; there is no invented journal row.
No `native_enqueued` or `consumed_by_read` receipt exists in this attempt.

I: the loaded-source event proves `thread/start` passed the daemon's real
initialize/transport checks. In `hosted_process.rs`, `input()` first calls
`transcript()`; errors after submitting turn/start carry `outcome_unknown:`.
The unprefixed rejection plus failed recovery observation locate this failure
in pre-input recovery processing, consistent with the empty usage export.
The daemon intentionally replaces arbitrary host error text with this generic
refusal (`RpcError::Rejected`). **U:** the underlying host error object, exact
rejected wire request, Unix socket pathname, child PID/start ticks, and actual
instruction-source string were not exported. No more specific root cause is
claimed and no tracing/product patch or second run was made to obtain them.

The final [seat record](integration/attempt3/run/team/runtime/seat.json) has null
session/pane/launchRoot, no appServer, terminalContract **0**, attachment generation
**0**, and health `session_dead`. This is the post-failure snapshot; it does not
prove an attachment was never transiently published. The only remaining capture,
[bootstrap pane %0](integration/attempt3/run/initialize-pane-0.txt), is an empty
shell. Production failure cleanup had already removed member panes. The
instruction-source warning fix was reached; the required runtime record proof
and strict attached-TUI proof were not obtained.

### Spend, cleanup and gates

Reserved random marker **`a6796ec88e60e144`** was never submitted.
[Cost ledger](integration/attempt3/run/cost-ledger.json) and
[usage export](integration/attempt3/run/usage-events.json) show:

| Spend item | Observed model turns / generations / compactions | Observed USD |
| --- | --- | --- |
| Claude lead startup, no work | 0 / 0 / 0 | $0.00 |
| Codex host/thread creation; failed startup recovery | 0 / 0 / 0 | $0.00 |
| Mesh messages, operator input, compaction | 0 / 0 / 0 | $0.00 |
| Attempt 3 total versus hard caps | **0 / 8 turns** | **$0.00 / $3.00** |

No accepted paid submission or token usage was observed. Actual subscription
billing was not queried; zero is the observed no-generation result, not an
invoice measurement. The controller included startup turns in its rollout
watchdog, with a one-turn/$0.60 reserve and conservative packet-rate accounting;
that watchdog's nonzero-spend path was not exercised.

Controller exit **1** was observed. Its finally block waited its own children,
ended the descendant PID namespace, and removed the scratch root/auth copy.
[Cleanup](integration/attempt3/run/cleanup.json): no survivors, port closed, root
removed. Five remaining host PID/start-tick identities are retained in
[identities.json](integration/attempt3/run/identities.json); the short-lived
app-server was already gone before that inventory. Descriptor restoration ran
exactly as requested, exit **0**:

```sh
git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs
```

The final read-only audit additionally checks PID/start-tick absence, private
socket/port/root removal, restored descriptor, evidence hygiene and zero product
diff. The built Mesh target artifact remains unused; its scratch executable copy
was deleted. No process started by another worker was killed.

Exact reproduction from `/home/mstie/projects/taurhaus-trial` (new authorized run
only; this controller reproduces the failed setup, not seven passing steps):

```sh
python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt3-controller.py attempt3/NEW_RUN
TRIAL_EVIDENCE_LABEL=attempt3 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

The gate helper is reused unchanged. It runs each exact recipe from this checkout
root with a credential-free HOME, all harness roots isolated, rejecting
CLI/tmux/Mesh shims, a private PID namespace, and checkout-local Cargo target.

| Required gate | Exit | Observed result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, frontend typecheck, 150 frontend files / 2,492 tests passed. |
| `just lint` | **0** | Rust/frontend/workflow/recipe gates passed. |
| `just test-contracts` | **0** | All contract test binaries passed; see retained log. |
| `just test-rust-unit` | N/A | No `src-tauri/` diff. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A | Conditional on seven-step PASS; no descriptor flip or fix. |

[Gate logs and exit metadata](integration/attempt3/gates),
[final audit](integration/attempt3/final-verification.json),
[audit controller](integration/attempt3-audit.py).
Each Cargo preflight returned 1, with no competing Cargo; gate cleanup passed.
**Descriptor flip: none. Mesh commit: none. Taurhaus registry entry: not needed**
(`HostedDescriptor::codex()` already pins 0.153.4 and unix-websocket).
Non-evidence product insertions **0 / 200**, dependencies **0**, tests added **0**.
The live acceptance assertion observed red at step 1; there was no product repair
and no claimed regression red/green cycle. The conditional Mesh missing-record
refusal test/fix was not undertaken after this failure.

Deviations / evidence limits:

- The fresh-seat request follows the sanctioned production path. In this source,
  `members.rs` executes hosted shared activation during the `CreatePanes` pass
  and skips it in `LaunchSessions`. The refusal therefore occurred before the
  later launch/opt-in steps; the controller did not reorder those stages.
  The lead's command was rendered and its version probed, but its model CLI
  launch and login-screen observation were not established before cleanup.
- The brief's full wire, holder, attachment and pane evidence is incomplete at
  this stopped boundary. The public rejection deliberately omits its raw error;
  no exact underlying host cause, instruction-source string, or turn is inferred.
  The controller's unexecuted action loop is not a seven-step implementation or
  proof; it was never reached. No paid rerun was performed.
- No numbered step passed, so there is one failed-trial evidence commit rather
  than seven green-step commits. All prior attempt evidence remains intact.
- An Opus evidence lens is unavailable in the exposed agent/model/tool surface;
  no independent cross-family review is claimed. The read-only audit is local
  verification, not a replacement for that lens.
- Text captures and gate logs trim trailing blank lines; the actual captured
  pane output remains in events.jsonl. No plan ledger, product code, release,
  deployment, operator installation or other Taurhaus checkout was changed.

## Attempt 4 — INCONCLUSIVE: step 1 observer confounded host initialization

2026-09-10. **No eligibility enabled.** The production initialize operation
returned `Conflict: unsupported app-server handshake` at `launch_host`.
**This run does not establish a product defect:** the trial controller added an
observer that could initialize before the daemon. Codex 0.153.4 seeds its
`userAgent` from the first client, as the binding transport amendment already
warns. The observer therefore was not passive with respect to initialization.
This was a controller mistake. The trial stopped immediately, without a retry,
product patch, paid input, or continuation to steps 2–7.

Pair: Taurhaus `ae9ae356cce35829e8449fca34246ca9a32399f1` on
`feat/integration-trial`, containing PR #159 / `6f61f611`, protocol **27**;
Mesh `dfa22bc103fc84c32de77a7b6cbe054d58cba13b` on `feat/native-push`.
[Execution metadata](integration/attempt4/execution.json),
[daemon build](integration/attempt4/daemon-build.json),
[Mesh build](integration/attempt4/mesh-build.json): both builds exited **0**.
Each build used its checkout-local target after the exact Cargo preflight
`pgrep -af '(^|/)cargo( |$)'` returned **1** (no competing Cargo).
The temporary [descriptor diff](integration/attempt4/mesh-trial-descriptor.diff)
enabled only 0.153.4 as `trial`, with exact class identities
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1` and
`unix-websocket`. The candidate binary was copied only into scratch
`HOME/.local/bin`; its SHA-256 and the other copied executable hashes are in
[events](integration/attempt4/run/events.jsonl).

| Ordered step | Outcome | S-runtime evidence / missing proof |
| --- | --- | --- |
| 1. Daemon launches host, attached pane, runtime record and startup card | **INCONCLUSIVE; execution assertion FAIL** | Production RPC failed at `launch_host`: `Conflict: unsupported app-server handshake`. Observer's Unix WebSocket Upgrade returned 101 and initialized against the scratch home. No thread/turn, appServer record, attached TUI or startup-card uptake established. |
| 2. Idle Mesh delivery, marker/reply, receipt and explicit-read consumption | **NOT RUN** | Stopped at step 1; no `mesh send`, receipt or explicit read. |
| 3. Active-thread pending, then one idle `turn/start` | **NOT RUN** | No thread or delivery obligation. The binding deferred-delivery rule was not exercised. |
| 4. Typed input plus pending delivery; mutual host lock exclusion | **NOT RUN** | No attached TUI; no holder-file proof. |
| 5. Compaction and recovery card on the same thread | **NOT RUN** | No compaction or recovery generation. |
| 6. Daemon restart and post-restart delivery | **NOT RUN** | Teardown only; no restart or identity-resume claim. |
| 7. Operational stop/remove/re-add tmux rollback and delivery | **NOT RUN** | No hosted seat to roll back; the documented refusal was read in source, not observed at runtime. Cleanup nevertheless completed. |

### Exact stopped boundary and the observer confound

[Controller](integration/attempt4-controller.py) created the team only through
`coordination.initialize_team`; its request has one Claude lead and one Codex
seat, `model: "gpt-5.6-luna"`, `reasoning_effort: "low"`,
`delivery: "app_server"`, and `messaging.mode: "canonical"` with the exact
[DEFAULT_CANONICAL_POLICY](integration/attempt4/run/policy.json) parsed from
`meshTabUtils.js`. It created a scratch git project with a short AGENTS.md,
read-only sandbox and never approvals. No launched seat was opted in later.
As in attempt 3, this source executes shared hosted activation during the
`CreatePanes` pass; the controller did not reorder production stages.

S: root `/tmp/th-int-02qek9op` (created private, 0700), daemon port **30683**,
private tmux socket `/tmp/th-int-02qek9op/tmux/tmux-1000/default`.
All product/account homes were under this root; inherited `TMUX` was absent.
Only the authorized `auth.json` was copied to the initially empty CODEX_HOME.
The daemon and its descendants ran inside a disposable Bubblewrap PID namespace,
with operator homes, `/tmp` and `/run` hidden except for the scratch bind.
The lead CLI version probe succeeded (Claude 2.1.267); a lead login screen was
not reached. No Claude model turn was requested.

The production operation id was **`init_62bda75b0bbe41d783a36fcd9518b23e`**.
[initialize-result.json](integration/attempt4/run/initialize-result.json) retains:

```json
{"failed_step":"launch_host","message":"Conflict: unsupported app-server handshake","retryable":true,"succeeded_steps":["validate_configuration","create_team","add_lead"]}
```

The [observer](integration/attempt4-observer.py) ran before initialization and
watched for the daemon-created Unix socket. It independently sent:

```json
{"id":"observer-init","method":"initialize","params":{"clientInfo":{"name":"trial_observer","version":"4"},"capabilities":{"experimentalApi":true}}}
```

S: [host-events.jsonl](integration/attempt4/run/host-events.jsonl), six rows,
records the exact HTTP Upgrade/accept, successful observer initialize against
`/tmp/th-int-02qek9op/codex`, and server close. It contains no `thread/started`,
`turn/started`, `thread/tokenUsage/updated`, or host JSON-RPC error object.
The observer retained only `codexHome` from its initialize result; it did **not**
retain `userAgent`. I: first-client seeding is the likely direct cause of the
refusal: `hosted_process.rs` parses the successful initialize response using
`strip_prefix("taurhaus_host/")` and returns the exact observed refusal when
that prefix is absent. U: the exact daemon initialize response and relative
initialize ordering were not captured, so that causal explanation is not a
wire-proven root cause. A future controller must avoid competing for first
initialization; this run was not repeated to test that correction.

S: [taurhaus.log.jsonl](integration/attempt4/run/taurhaus.log.jsonl) records
`coordination.step.failed` for `launch_host` at
`2026-09-10T02:53:31.315Z`. **There is no `hosted.rpc.rejected` log line or host
error object to quote**: the observed refusal is local validation of an
initialize response, not an observed JSON-RPC rejection. The raw error required
for a host-rejected-request case is therefore unavailable, not fabricated.
No `hosted.instruction_sources.loaded` event was reached; loaded AGENTS.md
sources cannot be claimed. Failure cleanup removed the provisional team before
the snapshot, so there are no runtime records, journal rows, receipts or holder
files for this attempt. The only surviving pane capture is the private bootstrap
shell [initialize-pane-0.txt](integration/attempt4/run/initialize-pane-0.txt).

### Spend, teardown and reproduction

Reserved random marker **`74212e7ee11810b2`** is in the isolation event; it was never submitted.
[Cost ledger](integration/attempt4/run/cost-ledger.json) and
[usage export](integration/attempt4/run/usage-events.json) agree:

| Spend item | Model turns / compactions | Observed spend |
| --- | --- | --- |
| Claude lead version probe; no work | 0 / 0 | $0.00 |
| Codex version probe and app-server initialization | 0 / 0 | $0.00 |
| Mesh inputs, operator inputs, compaction, restart, rollback | 0 / 0 | $0.00 |
| Attempt 4 total | **0 / 8 turns; 0 compactions** | **$0.00 / $3.00** |

No tokenUsage event or rollout turn was observed. No nonzero charge or null
per-turn cost is hidden. The unexercised observer ledger calculates each
`tokenUsage.last` at the packet's $0.20/$0.02/$1.20 per million
input/cached/output rates, plus a conservative all-tokens-at-$1.20 bound;
these are API-equivalent estimates, not actual subscription invoices.
Its pre-initialization connection hazard prevents calling that metering design
validated. Installation identifiers were redacted before logging, account usage
notifications omitted, and no credential contents retained.

Controller exit **1** was observed. Its finally block stopped and joined the
observer, ended the owned namespace, waited its child processes, verified no
run-tagged survivors and a closed port, and removed the scratch root and auth
copy. [Identities](integration/attempt4/run/identities.json) retain five
PID/start-tick pairs; the short-lived app-server had already exited before that
inventory. [Cleanup](integration/attempt4/run/cleanup.json) and the later
[read-only audit](integration/attempt4/final-verification.json) confirm all five
absent, no run-tagged process, private socket/root absent and port 30683 closed.
No foreign process was killed. Exact descriptor restoration exited **0**:

```sh
git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs
```

The Mesh worktree is clean. **Descriptor flip: none; Mesh commit: none.**
The scratch executable was deleted; the rebuilt target artifact is not installed.
**Taurhaus registry entry: not needed**; the existing hosted launch descriptor
already pins Codex 0.153.4 / unix-websocket.

Exact reproduction from `/home/mstie/projects/taurhaus-trial`, for a separately
authorized reproduction only (these scripts reproduce this stopped, confounded
setup, not a passing seven-step trial):

```sh
TRIAL_EVIDENCE_LABEL=attempt4 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt4-controller.py attempt4/NEW_RUN
TRIAL_EVIDENCE_LABEL=attempt4 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt4-audit.py
```

The audit targets the retained `attempt4/run`; a new evidence directory needs a
matching audit path. No paid retry, alternate client probe or product fix ran.

### Required gates and deviations

| Exact gate, from checkout root | Exit / result |
| --- | --- |
| `just check-quick` | **0**; Rust test compilation, frontend typecheck, 150 files / 2,495 frontend tests. |
| `just lint` | **0**. |
| `just test-contracts` | **0**. |
| `just test-rust-unit` | N/A: no `src-tauri/` diff. |
| Mesh `just check-quick`, `just lint`, `just test` | N/A: conditional seven-step PASS was not reached. |

[Gate logs and exit metadata](integration/attempt4/gates) retain the exact
commands and preflights (all Cargo probes exited 1), using the unchanged
credential-free gate helper: scratch HOME, rejecting CLI/tmux/Mesh shims,
private PID namespace, checkout-local target. Gate cleanup removed its root.
[Audit controller](integration/attempt4-audit.py) additionally checks branch,
merged prerequisite, isolation, policy, zero spend, cleanup and artifact hygiene.

- Material deviation: the added observer could become the first client and
  invalidate the intended production initialization. The trial is **INCONCLUSIVE**,
  not an eligibility FAIL attributed to either product. No downstream acceptance
  claim is made; all steps after the refusal were stopped as instructed.
- No numbered step passed, so this is one evidence commit, not seven green-step
  commits. The live step-1 assertion observed red; no product regression was
  repaired, no artificial red/green unit-test claim, and no tests added.
- The conditional Mesh `runtime_record_missing:<member>` test/fix was not made.
  Product insertions **0 / 200**, new dependencies **0**, plan ledger edits **0**.
- An Opus evidence lens is unavailable in the exposed model surface. No
  cross-family review is claimed; the audit is local verification only.

Attempt-4 text captures and logs trim trailing whitespace/empty lines; events.jsonl
preserves the original pane capture string. No artifact contains auth contents.

## Attempt 5 — FAIL at step 2: missing native adapter selection (2026-09-10)

Pair: Taurhaus `f2a7553b` (includes PR #159 / `6f61f611`, protocol 27),
Mesh `dfa22bc`, Codex 0.153.4, gpt-5.6-luna / low. Both checkout-local builds
exited 0 after Cargo preflight found no competing build. Only the Mesh scratch
working-tree descriptor enables the exact paired class identities.

**Step 1 PASS (S).** Production `coordination.initialize_team` completed all
nine stages, including `opt_in_delivery`, with the builder's canonical policy,
credential-free Claude lead and fresh `delivery: app_server` Codex seat.
The runtime publishes terminalContract 1, Unix WebSocket, all three class
identities, and `/tmp/th-int-3r8tuw4q/project/AGENTS.md` as an instruction source.
The daemon-generated strict config and attached pane are retained under
[integration/attempt5/step1](integration/attempt5/step1).
Thread `01a08948-2281-74c0-8bba-67bb9da54f94` received the recovery card in
turn `01a08948-22b4-7a73-afe8-d42785e3b31a`; the attached pane shows the card
and the model's reply. No observer connection was opened. The daemon transcript
RPC supplied the actual turn events and usage; structured logs show instruction
loading and startup delivery. One generation: 10,787 input, 6,912 cached,
32 output, 0 reasoning output; API-equivalent **$0.00095164**, conservative
all-tokens-at-$1.20/M **$0.01298280**. These are metered-token calculations at
the packet's rates, not billing invoices. Claude: zero turns / $0.

Controller: [attempt5-controller.py](integration/attempt5-controller.py), based
on attempt 3. It never connects to an app-server socket; reads go through the
daemon after successful initialization. Evidence helpers have three offline
checks, observed red (missing helper module), then green. No product code changed.

### Attempt 5 final outcomes

**FAIL, stopped at step 2. Descriptor disabled; no Mesh commit.** Step 1 is a
real production launch success, not an eligibility PASS. The idle Mesh message
never reached the thread. No product change was made to Taurhaus or Mesh.
No Taurhaus registry entry is needed: the existing launch path succeeded.
The conditional Mesh named-refusal fix and descriptor flip were not performed.

| Brief step | Outcome | S-runtime evidence |
|---|---|---|
| 1. Daemon host + strict attached TUI + startup card | **PASS** | [Frozen step-1 evidence](integration/attempt5/step1), runtime, strict config, completed startup transcript and pane. |
| 2. Idle Mesh send, marker reply, native receipt, explicit-read semantics | **FAIL** | [Send acceptance](integration/attempt5/run/step2-send.txt), [scheduler diagnostic](integration/attempt5/run/step2-diagnostic.json), [idle transcript](integration/attempt5/run/step2-hosted-transcript.json), [pane](integration/attempt5/run/step2-pending-pane-2.txt). Accepted message stayed pending for approximately 84 seconds; no marker input/reply or seat receipt. |
| 3. Active thread defers, then exactly one idle delivery | **NOT RUN** | Stop-on-failure rule at step 2. Binding behavior would be `pending: thread_active`, then `turn/start`, not steer. |
| 4. Operator/socket ordering and reciprocal host-lock contention | **NOT RUN** | No operator input or contention experiment; no holder-file proof claimed. |
| 5. Actual compaction and recovery card | **NOT RUN** | No compaction; identity/recovery at that boundary unverified. |
| 6. Daemon restart and subsequent delivery | **NOT RUN** | No restart or identity-preservation claim. |
| 7. In-place refusal and operational remove/re-add rollback | **NOT RUN** | No rollback attempted after step-2 failure. Teardown nevertheless completed and was verified. |

### Exact failure boundary

At `2026-09-10T03:08:20.836203067Z`, this command ran through a new window on
the private tmux server, inside the daemon's PID namespace, using only the
scratch `$HOME/.local/bin/mesh` candidate:

```sh
mesh send seat 'ACTION REQUIRED: Reply exactly cobalt90cc90082c. Do not execute tools.' --team integration --name lead --summary 'idle marker'
```

It exited **0** and returned:

```json
{"message_id":"6446e66f-2b6b-44a3-9614-bae46484550d","sequence":4,"status":"accepted","projection":"pending"}
```

Delivery ID: `3a4769cb-348e-46fd-83bf-ab89ccb3570e`.
The [canonical journal](integration/attempt5/run/team/state/messaging-v2/segments/000001.jsonl)
contains exactly one marker `message_accepted`, with
`dispatch_decision: {expectation: "action", wake_eligible: true}`.
No marker `delivery_attempt`, receipt or `consumed_by_read` followed. The
journal's earlier `native_enqueued` row is **the Claude lead's native-mailbox
startup projection**, not a Codex delivery. No explicit read was performed:
that part of step 2 remains unverified because native delivery already failed.

The live scheduler's heartbeat advanced through `03:09:44.717969379Z`, with:

```json
{"last_defer_reason":"IO error: delivery: pending: activity absent","failures":0,"completed":0}
```

The [runtime](integration/attempt5/run/team/runtime/seat.json) retains
`appServer.state: "ready"`, the original thread, and
`activitySnapshotPath: null`. The daemon transcript reports `status.type:
"idle"` and only the startup turn. The [team config](integration/attempt5/run/team/config.json)
retains `adapter_mode: "app_server"`; the directory inventory proves
`state/delivery/adapter-seat.json` is **absent** while the app-server socket
still exists. No `.native` attempt evidence exists.

**Source explanation (I, supported by S runtime):** Mesh
`src/delivery/hook/state.rs::selected_mode` loads its own
`state/delivery/adapter-MEMBER.json`; an absent file returns `Selection::default`
whose mode is `Tmux`. It does not read the config's `adapter_mode`.
`src/delivery/runtime.rs::RuntimeFactory::resolve` therefore takes the tmux
branch and calls `Record::idle`, which refuses the missing activity path before
constructing the native adapter. Taurhaus's fresh hosted activation publishes
its config/runtime identity but did not publish Mesh's selection file. This
is a paired activation-seam failure, not proof that WebSocket delivery failed.
No selection/runtime files were forged and no post-launch opt-in was attempted.

There is **no `hosted.rpc.rejected` line or host error object for step 2**:
this refusal occurs before a host input RPC. The exact Mesh refusal above is
the available error evidence. The [daemon JSONL](integration/attempt5/run/taurhaus.log.jsonl)
and [daemon transcript responses](integration/attempt5/run/events.jsonl)
are retained; the final audit verifies the absence of host rejection events.

### Cost and isolation ledger

| Input / boundary | Model turns / generations | Metered usage and spend |
|---|---|---|
| Startup card, step 1 | 1 / 1 | Turn `01a08948-22b4-7a73-afe8-d42785e3b31a`; 10,787 input, including 6,912 cached; 32 output, 0 reasoning. **$0.00095164 API-equivalent**; **$0.01298280 conservative**. |
| Marker, step 2 | 0 / 0 | Never reached host; **$0.00 additional**. |
| Steps 3–7, including compaction/restart/rollback | 0 / 0 | Not run; **$0.00 additional**. |
| Claude lead | 0 / 0 | Credential-free setup screen; **$0.00**. |
| Total vs caps | **1 / 8 turns**, one generation, zero compactions | **$0.00095164 API-equivalent**, conservative **$0.01298280 / $3.00**. Actual billed dollars are not exposed. |

[Cost ledger](integration/attempt5/run/cost-ledger.json) and
[host tokenUsage events](integration/attempt5/run/host-events.jsonl) agree with
[rollout usage](integration/attempt5/run/usage-events.json). The controller
polled the daemon's bounded event buffer; repeated snapshots are retained and
must not be counted as additional turns/generations. The helper deduplicates
identical usage events by thread/turn/cumulative usage. No unmetered turn exists.
No observer client connected, before or after startup. Daemon `hosted_transcript`
was the only host-state read path, so first-client identity was never contested.

The private root `/tmp/th-int-3r8tuw4q` was mode 0700; HOME, all harness/data roots,
TMUX_TMPDIR and generated configs were inside it. Only `auth.json` was copied
into initial CODEX_HOME; its contents were never printed or retained. Private
port **25925**, never 17233. Bubblewrap hid operator homes, `/tmp` and `/run`,
with one writable scratch bind and a private PID namespace. The namespace
held the daemon, real Claude/Codex executables, private tmux server, attached
TUI, and team delivery owner. No model tool execution occurred.

[Cleanup](integration/attempt5/run/cleanup.json),
[eleven PID/start-tick identities](integration/attempt5/run/identities.json), and
[final audit](integration/attempt5/final-audit.json) prove all recorded processes
absent, port closed, socket/root absent and the scratch credential copy destroyed.
The controller's finally block terminated/waited only its children/process groups;
namespace teardown removed their descendants. No operator process was killed.
`git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs`
exited **0**, and the Mesh worktree is clean. Its built scratch candidate is not
installed into the operator's bin directory; source eligibility remains disabled.

### Reproduction, gates, and deviations

Executed from `/home/mstie/projects/taurhaus-trial`, without a branch switch:

```sh
TRIAL_EVIDENCE_LABEL=attempt5 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt5-controller.py attempt5/run
TRIAL_EVIDENCE_LABEL=attempt5 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt5_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt5-audit.py
```

[Execution metadata](integration/attempt5/execution.json) records build exits 0,
runtime exit **1** (the deliberate failed-step stop), gate-controller exit 0,
and helper/audit exits 0. For a newly authorized reproduction, choose a fresh
output label. Once `inspection_ready` and idle startup completion are observed,
submit the [recorded actions](integration/attempt5/actions.json) one at a time
as `action.json` in that run directory, waiting for `action_done`. These are
all executed actions, including the final failure stop; they are not a claimed
implementation of unrun steps 3–7. The short AGENTS.md, request, exact child argv,
private tmux bootstrap and cleanup are in the executed controller. The step-1
strict-config capture was a read of the published socket's sibling
`tui/config.toml`, recorded in `step1/config-capture.json`.

| Gate (checkout root; private credential-free home, inert CLI/tmux shims) | Exit |
|---|---|
| `just check-quick` | **0**; 150 frontend files, 2,495 tests; Rust tests compiled |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: conditional passing descriptor flip was not reached |

[Gate logs](integration/attempt5/gates) retain exact commands and Cargo preflight
exit **1** (no competing Cargo) before each recipe. All gate scratch roots were
removed. Four offline evidence-helper tests pass; the final audit checks 48
facts. Initial red was missing helper import; an additional real red caught an
over-broad evidence sanitizer before its correction. No product regression fix
or associated product test was commissioned after the runtime failure.

Deviations/limits:

- The marker command emitted a warning about missing formal assignment fields,
  but accepted it as an actionable, wake-eligible message; this was a bounded
  echo probe, not a task assignment. The scheduler diagnostic predates the send
  and identifies the earlier adapter-selection/runtime boundary.
- The executed sanitizer also redacted `author`, authority revision, and some
  scratch executable path suffixes because it matched `auth` and `/home` too
  broadly. These placeholders remain honest gaps; journal checksums describe
  pre-redaction rows, not byte-identical exports. Sender is still retained in
  the envelope, root revision in the card text, and full executable construction
  in the controller. No redacted observation was guessed back into existence.
  The [executed helper](integration/attempt5/support-executed.py) is archived;
  the current helper fixes only export filtering. A red regression test naming
  `d3b95226` preceded that correction. Final export additionally removes rollout
  account rate-limit metadata; no credentials, installation IDs or account
  usage rows are retained.
- No Opus evidence lens was available among the callable models/tools. The
  independent Opus review remains outstanding; the report claims only the
  runtime results and offline audit, not cross-family approval.
- One commit records green step 1; a second records failed step 2, final audit,
  gates and teardown. Steps 3–7 have no commits because they were not executed.
  Non-evidence product diff: **0 lines**, no dependencies, release, installation,
  plan-ledger changes, or Taurhaus registry change.

Attempt-5 text logs and pane exports trim trailing whitespace/empty lines;
JSONL retains the repeated daemon event snapshots, with documented redactions.


## Attempt 6 — production paired selection candidate

Taurhaus baseline `7fc64f0e` (includes hosted thread-state PR #159; protocol 27),
Mesh `a6ee296`. Both checkout-local builds exited **0**, after Cargo preflight
exit 1 (none running). The scratch-only compiled descriptor matches the daemon
class identities; no registry entry is needed by the launch path.

### Step 1 — PASS

The production `coordination.initialize_team` request used the builder's exact
DEFAULT_CANONICAL_POLICY, a credential-free Claude lead, and one
`gpt-5.6-luna` low seat with `delivery: app_server` at creation. Every pipeline
step completed, including `launch_sessions` and `opt_in_delivery`.
[Request and outcomes](integration/attempt6/step1/initialize-result.json),
[policy](integration/attempt6/step1/policy.json).

The [runtime record](integration/attempt6/step1/step1-runtime.json) publishes
terminalContract 1, thread `01a0898d-222f-7581-8cd8-332d3b3e2fb4`, Unix WebSocket
host identity and the scratch project's AGENTS.md instruction source. The
[process identities](integration/attempt6/step1/step1-identities.json) record
the app-server child and attached TUI; the [strict config](integration/attempt6/step1/generated-config-0.toml)
sets the matching model/effort/read-only/never policies.
The [pane](integration/attempt6/step1/step-1-pane-2.txt) shows the startup card
and the model reply. [Host events](integration/attempt6/step1/host-events.jsonl)
and [structured log](integration/attempt6/step1/taurhaus.log.jsonl) retain
turn/start acceptance, instruction loading, and onboarding delivery. No observer
connection was opened.

Startup turn `01a0898d-225c-77a1-b591-bd7e93510d55`: 10,777 input, 6,912 cached,
27 output, zero reasoning; **$0.00094364 API-equivalent**, **$0.01296480
conservative**. [Metered ledger](integration/attempt6/step1/cost-ledger.json).
The host emits token counts, not invoiced USD; rates use the attached-TUI packet.

Reproduction: `TRIAL_EVIDENCE_LABEL=attempt6 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py`,
then `python3 docs/design/evidence/native-eligibility/integration/attempt6-controller.py attempt6/run`.
The controller reuses attempt 5 with lossless overlapping-buffer export and
retains only new host events. Its offline regression names `d3b95226`; red was
`ModuleNotFoundError: attempt6_support`, then one passing regression test after
implementation. No product code or product regression fix was made.

### Step 2 — PASS

[Owner status](integration/attempt6/step2/step2-status-before.txt) reports
`adapter seat: mode=app_server source=config diagnostic=none`; live team owner,
epoch 2. The [send](integration/attempt6/step2/step2-send.txt) accepted random
marker **cobalta6e6c2cd57**, message `c9199479-21ad-4b17-aab2-0d4a13ae0e31`,
delivery `d498033d-dfd5-4533-baed-5f3ef9081c53`.
The [journal before read](integration/attempt6/step2/journal-before-read.jsonl)
records exactly one native enqueue for it, method `turn/start`, request
`69eabd28-632e-4dd4-8f1c-bc9c80810bc7`, turn
`01a0898e-39df-79e2-bf52-02e8ac1ca7bc`, unchanged thread. It contains no
`consumed_by_read` for the marker. The [pane](integration/attempt6/step2/step2-pane-2.txt)
shows the input once and exact reply once. Only the subsequent
`mesh read --unread --mark-read --team integration --name seat` produced the
[explicit read receipt](integration/attempt6/step2/explicit-read-receipt.json).

The echo turn used 11,840 input (9,984 cached), 14 output, no reasoning:
**$0.00058768 API-equivalent**, **$0.01422480 conservative**. Cumulative:
**2 turns, $0.00153132 / $0.02718960 conservative**.
The send warned about absent formal task-assignment fields; it remained an
authorized bounded echo message, accepted as actionable and wake-eligible.

### Attempt 6 final outcome — INCONCLUSIVE

| Ordered step | Verdict | Runtime evidence / boundary |
|---|---|---|
| 1. Production daemon launch + startup card | **PASS** | Runtime, strict config, child/TUI identities, pane and startup usage above. |
| 2. Idle native delivery + explicit-read semantics | **PASS** | Config-selected app_server; one marker and reply; native enqueue precedes explicit read, above. |
| 3. Active-thread deferral, then one idle delivery | **INCONCLUSIVE as a full step; protocol behavior PASS** | Five `pending: ... thread_active` receipts precede one `turn/start` enqueue; one user item and one reply. Active pane captured, final reply pane not captured before controller reserve stop. |
| 4. Typed operator/socket ordering + both lock directions | **NOT RUN** | Controller already stopped; no holder contention claim. |
| 5. Compaction and recovery boundary | **NOT RUN** | No compaction submitted. |
| 6. Scratch daemon restart | **NOT RUN** | No restart submitted. |
| 7. Operational rollback | **NOT RUN** | No in-place refusal, stop/remove/re-add or tmux delivery claimed. Final safety teardown did run. |

**Stopping reason is a controller limit, not a product rejection.** The exact
controller exception was `AssertionError: evidence reserve reached` at step 3,
exit **1**, at epoch timestamp `1789014389.2898245`. The implementation added
an 850,000-byte runtime-output reserve to protect the requested <1 MB retained
sidecar ceiling. It successfully stopped further work, but counted unfiltered
periodic daemon telemetry and stderr, so it stopped prematurely. This is an
avoidable trial-controller deviation. No second paid run or product patch was
made to work around it. The final retained evidence is below 1 MB after offline
filtering; that does not retroactively complete the stopped trial.

#### Step 3 observations

The [exact action driver](integration/attempt6-step3.py) starts one bounded,
80-line response through `coordination.hosted_input`, then sends random marker
**deferred4e17e869** through the scratch Mesh binary while that turn is active.
[Start result](integration/attempt6/run/step3-active-start.json),
[send result](integration/attempt6/run/step3-send.txt),
[active pane](integration/attempt6/run/step3-active-pane-2.txt),
[active status](integration/attempt6/run/step3-active-status.txt).
No observer connected; no observer turn/start, steer, or interrupt was used.

The [canonical journal](integration/attempt6/run/team/state/messaging-v2/segments/000001.jsonl)
retains message `15b2d258-0b8f-49ef-b114-6927e55eee5a`, delivery
`46008da4-087a-4bf8-988a-40c12bc97983`. Receipt sequences **10, 12, 14, 16, 18**
are pending, each with this exact evidence class:

```text
pending: pre_input_failure: IO error: delivery: thread_active
```

Each pending receipt has null method/request/turn fields because it is
**pre-input**, not an unmetered model turn. Sequence **20** is the sole native
enqueue for this obligation, with request `ea8dc458-d2e5-4b16-bcc3-0be38367a447`,
method `turn/start`, turn `01a08991-1738-7d52-a973-4b2148cd3b9d`, and unchanged
thread `01a0898d-222f-7581-8cd8-332d3b3e2fb4`. Its host start followed the
active seed's completion. [Host events](integration/attempt6/run/host-events.jsonl)
retain all four starts/completions and all four tokenUsage events, appended
only once across rolling polls. `item/completed` contains exactly one user
item and one exact reply for each random marker. The final
[hosted state](integration/attempt6/run/hosted-transcript.json) is idle and
contains the deferred reply; its bounded `eventsTruncated: true` view no longer
contains early turns, which remain in the appended event archive.

The active status capture reported `failed to acquire lock: delivery mirror
busy`, and the later status reported `error=none deferred=none` after completion.
Those are retained observations, not evidence for the separate Step 4 host-lock
requirement. The journal provides the required named active deferral. There is
**no `hosted.rpc.rejected` event and no host error object** in this run; none is
fabricated for the controller stop. The final host turns report `error: null`.
The stderr also repeatedly warns about `_active-project-teams/config.json`
being absent; this is retained with counts, not patched or assigned causality.

#### Complete cost ledger

All generations used `gpt-5.6-luna`, low effort. Input includes the cached subset;
output includes reasoning. Dollar figures are numerical API-equivalent estimates
from the host's actual tokenUsage events and the attached-TUI packet's rates
($0.20/$0.02/$1.20 per million input/cached/output), **not invoiced charges**.
Conservative cost applies $1.20/M to every input and output token.

| Input | Turn ID | Input / cached / output / reasoning | API-equivalent USD | Conservative USD |
|---|---|---|---|---|
| Startup | `01a0898d-225c-77a1-b591-bd7e93510d55` | 10,777 / 6,912 / 27 / 0 | 0.00094364 | 0.01296480 |
| Idle Mesh echo | `01a0898e-39df-79e2-bf52-02e8ac1ca7bc` | 11,840 / 9,984 / 14 / 0 | 0.00058768 | 0.01422480 |
| Step 3 active seed | `01a08990-e90d-7693-8669-a5dfdcfd3f87` | 11,885 / 11,008 / 519 / 34 | 0.00101836 | 0.01488480 |
| Deferred Mesh echo | `01a08991-1738-7d52-a973-4b2148cd3b9d` | 12,508 / 11,008 / 11 / 0 | 0.00053336 | 0.01502280 |
| **Total** | **4 / 8 turns; 4 generations; 0 compactions** | **47,010 / 38,912 / 571 / 34** | **0.00308304** | **0.05709720 / 3.00** |
| Claude lead and unrun steps 4–7 | 0 turns | 0 / 0 / 0 / 0 | 0.00 | 0.00 |

[Ledger](integration/attempt6/run/cost-ledger.json) and
[rollout accounting events](integration/attempt6/run/usage-events.json) agree:
`metering_complete: true`, no unmetered turn IDs. No tool-execution item occurred.

#### Isolation, teardown, export, and gates

Private scratch root `/tmp/th-int-3s1yjoo9`, mode 0700; private port **28897**.
Only auth.json was copied into the initially empty CODEX_HOME. The allowlisted
HOME, harness/data roots and TMUX_TMPDIR all remained beneath scratch, TMUX was
absent from the parent environment, and bubblewrap hid operator homes, /tmp and
/run while owning a private PID namespace. The Claude lead stayed at its
credential-free setup screen. [Exact commands, requests, binary hashes and
identities](integration/attempt6/run/events.jsonl).

The controller's finally block terminated and waited only its own process
groups; namespace teardown removed the daemon, child, attached TUI and private
tmux server. [Cleanup](integration/attempt6/run/cleanup.json) records no
survivors, closed port, and removed scratch root/auth copy. The final offline
PID/start-tick audit independently found every recorded process absent, and
checked that the private port and app socket were closed/absent. No operator
process was killed. Descriptor restoration ran the exact authorized
`git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs`
and exited **0**. Mesh is clean, its descriptor disabled, **no Mesh commit**.
No Taurhaus registry entry is needed; no Taurhaus product code changed.

Reproduction after the build/controller commands above: replay the
[recorded actions](integration/attempt6/actions.json) one at a time through
`action.json`, waiting for `action_done`; the Step 3 driver preserves the timed
sequence. These scripts reproduce the executed portion only, not unrun steps.
A new live run requires a newly authorized trial and a fresh output label.
The executed controller is retained unchanged, including its premature reserve.
Offline completion commands, from this checkout root:

```sh
TRIAL_EVIDENCE_LABEL=attempt6 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt6-export.py
python3 docs/design/evidence/native-eligibility/integration/attempt6_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt6-audit.py
```

The exporter is a one-time post-teardown transform of the fresh run directory.
It retains first/last periodic telemetry/stderr samples and all other structured
records, with original counts in the [manifest](integration/attempt6/export-manifest.json).
Identical frozen/final files are retained once and mapped there; final-view
history points to the append-only event file. Redactions remove installation
IDs, credentials/account metadata and disallowed operator-home paths. These
are sanitized exports, not byte-identical journal checksum validation.
Initial unfiltered evidence exceeded 1 MB (**1,163,225 bytes**, observed red);
the final bounded export passes, including the scripts and audit. Frozen
per-step snapshots remain distinct time-scoped evidence, not extra usage.

| Exact gate, credential-free isolated environment at checkout root | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: conditional all-seven-step PASS was not reached |
| Offline overlapping-buffer regression / final audit | **0 / 0**; one regression test and 140 initial audit assertions |

[Gate records](integration/attempt6/gates/gate-check-quick.json),
[lint](integration/attempt6/gates/gate-lint.json),
[contracts](integration/attempt6/gates/gate-test-contracts.json),
[final audit](integration/attempt6/final-audit.json). Every gate's Cargo preflight
exited 1 (no competing Cargo); gates used inert harness/tmux shims, no credentials,
and private PID namespaces. Their scratch roots were removed.

Remaining deviations: the reserve guard prematurely ended the run; Step 3's
final pane was not captured; Steps 4–7 and the conditional Mesh named-refusal
fix/descriptor flip were not reached. No Opus model/tool is callable here, so the
requested independent Opus evidence lens remains unavailable. No review approval
is claimed. The initial offline audit assumed full history in the bounded final
view and user items in turn/completed; observed red corrected it to authoritative
item/completed events in the complete append-only archive. No paid retry was used.
Non-evidence product diff **0 lines**, no dependencies, installation, release,
branch switch or plan-ledger edit.


### Continuation preparation — offline green, no additional model turns

On the user's continuation request, the checkout was clean: Step 1 was already
committed as `45abdcf7`, Step 2 as `e1d004a9`, and the stopped Step 3 result,
gates and teardown as `c4735af5`. No completed green runtime step was left
uncommitted. The original scratch root/thread was deleted; a live continuation
requires a new production initialization. The original hard cap has **four
model turns remaining**. No budget reset was inferred from “continue.”

The prepared [next controller](integration/attempt6-next-controller.py) reuses
attempt 6's production initialization, private namespace, auth-only scratch
home, exact pinned tuple, token ledger and teardown. It requires an explicit
new-turn allowance argument (1–8); the operator must supply the authorized
remaining allowance. The executed attempt-6 controller and earlier continuation
controller are preserved as historical evidence. This prepared controller has
**not been run against a live daemon or model** and proves no new runtime step.

The premature evidence-reserve defect is corrected in collection: raw stderr
stays beneath scratch, only its last 16 KB is retained at teardown; periodic
structured telemetry retains first/last samples and counts; final transcript
snapshots reference the single appended event archive; streaming text deltas
are omitted while completed items, thread/turn boundaries, receipts and token
usage remain intact. The 850 KB retained-output reserve still applies.

[Three offline regression tests](integration/continuation_retention_test.py)
name `c4735af5`; red was missing `continuation_retention`, followed by three
green assertions suites with the helper and controller integration.
[Replay of real retained events](integration/attempt6/continuation-preflight/retention-replay.json)
reduced 569 events / 165,547 bytes to 50 events / 27,152 bytes while preserving
every tokenUsage, turn start/completion and completed item. This replay opened
no runtime socket and read no credentials.

Fresh gates ran from the checkout root with the existing credential-free
private-namespace gate controller:

```sh
python3 docs/design/evidence/native-eligibility/integration/continuation_retention_test.py
TRIAL_EVIDENCE_LABEL=attempt6/continuation-preflight python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
```

| Exact gate | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| New offline collector regression | **0**, 3 tests |

[Gate records](integration/attempt6/continuation-preflight/gates/gate-check-quick.json),
[lint](integration/attempt6/continuation-preflight/gates/gate-lint.json),
[contracts](integration/attempt6/continuation-preflight/gates/gate-test-contracts.json).
Each Cargo preflight found no competing Cargo. Gate scratch cleanup completed.
No `src-tauri/` change, product code, registry entry, descriptor edit, or Mesh
commit. **Additional spend: zero turns / $0.00.** The next live run awaits the
user's answer whether its budget is fresh or shared with the four spent turns;
no missing capture or unrun step has been promoted to PASS.

## Attempt 7 — FAIL at step 4, 2026-09-10

**Latest verdict: FAIL at step 4 under an injected process pause.** Steps 1–3
passed on the real daemon; the typed-input portion of step 4 also passed.
The lock interleave used SIGSTOP/SIGCONT on verified trial-owned processes.
It observed exclusion in both directions, but the daemon's interrupted read
failed, then subsequent reads returned `uncorrelated host response`. This is
not evidence of failure under ordinary, unpaused scheduling. No product code
was patched; steps 5–7 were not run. The compiled Mesh descriptor is restored
to disabled, the Mesh tree is clean, and there is no Mesh flip/fix commit.
The existing Taurhaus registry launched the seat successfully; no entry is needed.

Pair: Taurhaus worktree `feat/integration-trial`, starting `ebca1917`, including
hosted thread-state merge `6f61f611`, protocol 27; Mesh `feat/native-push`
`a6ee296`. The trial-only descriptor matches `taurhaus-daemon-owned-thread/1`,
`strict-config/1`, `daemon-owned/1`, and `unix-websocket` on exactly 0.153.4.
Builds used checkout-local targets after the required Cargo probes found no
competing Cargo. Both build exits were 0.

### Ordered outcomes (S: observed runtime)

Paths below are beneath [attempt7/run](integration/attempt7/run/); identical
snapshots have aliases in [duplicate-aliases.json](integration/attempt7/duplicate-aliases.json).

| Step | Outcome | Runtime evidence |
|---|---|---|
| 1. Hosted launch/startup | **PASS** | `step1-runtime.json`, `step1-identities.json`, `generated-config-0.toml`, `step-1-pane-2.txt`: child on Unix socket, attached strict-config TUI, terminalContract 1; scratch AGENTS.md instruction source. Startup recovery card and reply. `initialize-result.json` proves production canonical initialization and opt-in completed. |
| 2. Idle native delivery/read | **PASS** | `step2-status-after.txt`: seat mode=app_server source=config. `step2-pane-2.txt`: saffron7c82ab input/reply. Message `7bc10aed-1448-4372-b95f-41c5e5d30c06`, receipt native_enqueued, turn/start `01a089a5-2174-76c1-a7d4-3ee845098b65`. Journal sequences 1–6 have no consumed_by_read; `step2-read-receipt.json` records it at sequence 7 after explicit `mesh read --unread --mark-read`. |
| 3. Active-thread deferral | **PASS** | `step3-receipts.json`: message `052292df-27e8-4cc9-b5c0-18ac2b05b4bc` first pending/thread_active, then exactly one native_enqueued turn/start. `step3-exposure.json`: one completed user item and one reply for juniper8e21cd. `step3-final-pane-2.txt` captures the reply. |
| 4. Typed input + host exclusion | **FAIL after lock interleave** | `step4-final-pane-2.txt` and completed items prove maple7d91cb typed through private tmux while cedar9a38ef was pending; operator reply precedes socket delivery, each once. `step4-locks.json` proves actual kernel lock owners and both deferrals, followed by a failed host read. `events.jsonl` records the terminal uncorrelated-response failure and teardown. |
| 5. Compaction | **NOT RUN** | Stopped at step 4. |
| 6. Daemon restart | **NOT RUN** | Stopped at step 4. |
| 7. Operational rollback | **NOT RUN** | Stopped at step 4; no in-place rollback refusal or operational remove/re-add is claimed. Final process teardown was completed separately. |

Thread throughout the completed observations:
`01a089a4-2183-70a1-b1c6-566c00026910`. Root `/tmp/th-int-9n5s3dud`, private
port **23640**; private tmux socket beneath that root. Namespace host PID 124
corresponded to OS PID 1744349/start ticks 23616732. All panes, the team owner,
Claude login lead, host, daemon and scratch credentials were isolated. The
controller used an allowlisted environment without inherited TMUX, copied only
auth.json, and hid operator homes and services through Bubblewrap. No observer
connected to the member socket; host evidence came through the daemon RPC.
Claude ran no model turn.

### Exact failure and lock-holder evidence

The first single read-only sample missed the brief lock hold (no turn spent;
`step4-locks-initial-sample.json`). The second bounded sample briefly paused the
owned app-server to let a real daemon transcript request hold the host-operation
lock, then paused the owning daemon and resumed the app-server. During the
1.1-second hold, Mesh recorded this receipt for the lock marker:

```text
message_id: 63dc0eca-8cbd-457b-82c3-79d14807fd01
stage: pending
class: pending: pre_input_failure: IO error: delivery: host_operation_busy
```

Kernel holder evidence (same stable inode, preserved in `step4-locks.json`):

```text
FLOCK ADVISORY WRITE 1744225 08:30:1155236 0 EOF  # daemon
FLOCK ADVISORY WRITE 1745865 08:30:1155236 0 EOF  # Mesh
```

The host-operation implementation does not publish a `.holder.json`; those
files belong to terminal locks. The sidecar captures actual `/proc/locks` and
`/proc/<pid>/fdinfo` holder records, not an invented product holder file.
After resuming the daemon, its outstanding transcript RPC returned:

```json
{"code":"HOST_OPERATION_FAILED","message":"host read failed; outcome may be unknown"}
```

The helper then captured Mesh holding the same lock and issued one daemon
transcript read, which waited about two seconds and returned the expected:

```json
{"code":"HOST_OPERATION_FAILED","message":"Conflict: host operation deferred: lock busy"}
```

The helper's local `PASS` means only these two exclusion observations. Once
all paused processes resumed, the main controller and its final read returned:

```json
{"code":"HOST_OPERATION_FAILED","message":"uncorrelated host response"}
```

It exited **1**, stopped at step 4, and tore down immediately. The lock probe
had continued its read-only reverse-exclusion check after the first read error;
that continuation and its paused-process condition are deviations, not normal
host behavior. Source inspection suggests an interrupted socket read followed
by a late correlated response rejected on the next RPC (inference, not an
established underlying OS error: the implementation discards that error).
No `hosted.rpc.rejected` row or app-server JSON-RPC error object was emitted:
these are host-client transport/correlation errors. `taurhaus.log.jsonl`
preserves the actual structured instruction/onboarding/coordination rows;
`final-audit.json` records the absence rather than fabricating a rejection.

### Spend (S token counts; I API-equivalent dollars)

The model was **gpt-5.6-luna, low**, as pinned in the packet. Rates are the
packet's $0.20/$0.02/$1.20 per million input/cached/output tokens; amounts are
not billing receipts. Every completed generation has real host tokenUsage
numbers in `host-events.jsonl`, checked against rollout events in
`usage-events.json`. Ledger: [cost-ledger.json](integration/attempt7/run/cost-ledger.json).

| Input / generation | Turn ID | Input / cached / output | API-equivalent USD |
|---|---|---|---|
| Startup card | `01a089a4-21b1-7660-9d59-c37505975bd3` | 10773 / 6912 / 30 | $0.00094644 |
| Idle marker | `01a089a5-2174-76c1-a7d4-3ee845098b65` | 11838 / 9984 / 11 | $0.00058368 |
| Active timing input | `01a089a5-c1e8-7c13-880d-2662be2ba50e` | 11880 / 11008 / 517 | $0.00101496 |
| Deferred marker | `01a089a5-edd8-7021-ba5d-cd7452e0ba50` | 12505 / 11008 / 19 | $0.00054236 |
| Typed-order timing input | `01a089a7-310b-7a93-b7ae-7cef13d84fb9` | 12555 / 12032 / 495 | $0.00093924 |
| Typed operator steer (same turn) | `01a089a7-310b-7a93-b7ae-7cef13d84fb9` | 13069 / 12032 / 10 | $0.00046004 |
| Pending socket marker | `01a089a7-6204-76e0-aea4-2e38a79c9075` | 13182 / 12032 / 10 | $0.00048264 |
| Lock marker, interrupted | `01a089a9-946d-7ca3-8b43-17d518b961f1` | **Unreported** | **Unreported** |

**Seven protocol turns started / eight paid inputs including the typed steer.**
Seven completed generations have a metered subtotal of **$0.00496936**
(conservative all-tokens-at-$1.20/M subtotal **$0.1042728**). The final lock
marker started at 04:53:07.311Z as the paused Mesh process resumed; the main
controller failed at 04:53:07.320Z. Cleanup raced that already-pending delivery.
It has task_started but no tokenUsage/token_count before namespace termination.
No zero, null charge, invented token count, or verified full-run USD total is
claimed. **The required complete cost ledger was not achieved.** Counts stayed
below 16; the full-run dollar cap cannot be independently verified from the
missing final usage. No further paid retry was made.

### Reproduction, tests, gates and cleanup

Exact retained controllers (all are evidence, no product changes):

```sh
TRIAL_EVIDENCE_LABEL=attempt7 python3 docs/design/evidence/native-eligibility/integration/attempt3-build.py
python3 docs/design/evidence/native-eligibility/integration/attempt7_test.py
python3 docs/design/evidence/native-eligibility/integration/continuation_retention_test.py
python3 docs/design/evidence/native-eligibility/integration/attempt7-controller.py attempt7/run
# While the controller is live: replay the serialized action objects in events.jsonl
# through attempt7-actions.py, in order; the step-4 lock probe is:
python3 docs/design/evidence/native-eligibility/integration/attempt7-locks.py
# After teardown:
TRIAL_EVIDENCE_LABEL=attempt7 python3 docs/design/evidence/native-eligibility/integration/attempt2-gates.py
python3 docs/design/evidence/native-eligibility/integration/attempt7-audit.py
```

Attempt 7 reuses attempt 6's controller and ebca1917 retention helpers. It drops
stream deltas, deduplicates overlapping event buffers, excludes verbose periodic
telemetry, caps pane captures at 60 lines, and has no size or overall-time abort.
The new offline budget test first failed on a 1.1 MB evidence sample and on the
missed seventeenth host-only turn, then all three passed after correction.
The three existing retention tests also passed. No test invoked a real CLI or
read credentials. Three per-step commits retain completed steps 1–3.

| Exact gate (checkout root, isolated credential-free homes and inert harness shims) | Exit |
|---|---|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run: passing descriptor flip/named-refusal fix was conditional on seven passing steps |

Gate records and bounded log tails are under `attempt7/gates/`. No dependencies,
product changes, releases, installations or plan-ledger edits. Non-evidence
insertions: **0/200**. No Opus evidence review was run; this session exposes no
Opus review model. The interrupted-read condition and incomplete final metering
remain explicit limits for review.

Cleanup: `cleanup.json`, `identities.json`, and `final-audit.json` verify no
PID/start-tick survivor, port 23640 closed, private tmux/server namespace ended,
and scratch root including copied auth removed. All paused processes were
resumed in the lock helper's finally block before controller teardown. No
foreign process was signalled. Mesh restoration used the exact authorized
`git -C /home/mstie/projects/mesh-push checkout -- src/delivery/app_server/capabilities.rs`
and exited 0; its whole working tree is clean. Deduplicated retained evidence
is about **372 KB**, excluding controller scripts. Latest verdict remains
**FAIL step 4**, with steps 5–7 unrun and eligibility disabled.


## Attempt 8 — in progress, 2026-09-10

Fresh 16-turn / $3 budget, including every setup generation. No product change.
Pair: Taurhaus `47b7b8c2` (hosted merge `6f61f611`, protocol 27), Mesh
`a6ee296`; both checkout-local builds passed after Cargo exclusion. The temporary
0.153.4 descriptor pins the three published class identities and unix-websocket.

| Step | Outcome | Evidence under integration/attempt8/run |
|---|---|---|
| 1. Hosted launch/startup | PASS | initialize-result.json; step1-runtime.json; step1-identities.json; generated-config-0.toml; step-1-pane-2.txt; host-events.jsonl |
| 2. Idle delivery/read | Pending | |
| 3. Active deferral | Pending | |
| 4. Typed input/passive lock sampling | Pending | |
| 5. Compaction | Pending | |
| 6. Normal daemon restart | Pending | |
| 7. Operational rollback | Pending | |

The first setup launched successfully and showed its startup card/reply, then the
evidence collector parsed a concurrently appended partial JSONL row and exited.
`attempt8/setup-collector-abort` retains that complete metered startup generation
($0.00095444 API-equivalent; $0.0129816 conservative) and verified teardown. This
was no failed product assertion. The collector now reads complete JSONL rows;
an offline test observed red then green. Repeated setup is charged to this same
attempt's budget. The controller reserves that turn and conservative spend.

The continuing setup passed all step-1 assertions. The production canonical
initialize RPC launched the Claude login-only lead and one Luna/low hosted seat.
Scratch AGENTS.md is in instructionSources; generated strict view config and the
app-server child/attached TUI identities are retained. No observer connection.
No Taurhaus registry entry is needed.
