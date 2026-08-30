# W4 experiment 3 — a managed Codex member completes a bounded task through the assignment contract (2026-08-30)

Experiment 3 of [`../w4-managed-stages-design.md`](../w4-managed-stages-design.md): does a *stage* — an assignment to a managed member — actually carry a bounded implementation task end to end, with the assignment's effort in force before the member reads it? It ran live on the development host against a real Codex subscription. It does, and the whole gate costs about two seconds.

The lane is `e2e/specs/managed-stage-codex.js`. It is paid, so `e2e/specList.js` keeps it out of the default WDIO spec list and it only runs when named.

## Commands

```bash
# once, if the installed mesh is not the locked build (see "Host state" below)
just install-mesh

# the lane (builds the E2E binary by default; E2E_SKIP_BUILD=1 only when it is known fresh)
E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex
```

The measured run passed nothing else and built its own binary — `cargo clippy --all-targets` had replaced `target/debug/taurhaus` earlier in the session, and E2E needs the Tauri debug build. The isolated roots, the tmux server, the fixture project and the scratch `CODEX_HOME` are all created by the lane and its wdio config.

## Setup as measured

| | |
|---|---|
| Host | Linux (WSL2), app + daemon 0.8.3, daemon protocol 14 |
| mesh | 0.2.23 (`806258f`), both on `PATH` and at `~/.local/bin/mesh` |
| Codex | 0.150.1, `gpt-5.6-sol`, launched at effort `low` |
| Team | Claude lead `e2e-lead` (launch-new, isolated credential-free `CLAUDE_CONFIG_DIR`, never took a turn) + `codex-stage`, launched at `low`, for the gate; `codex-stage-medium`, launched at `medium`, added for the negative path |
| tmux | a server of the run's own: `TMUX_TMPDIR` inside the wdio session temp root, inherited `TMUX` cleared, killed whole at teardown. The operator's server is never written to |
| Project | throwaway git repo under the wdio session temp root: `package.json`, one Svelte file, one commit |
| Assignment | `--effort medium --why "experiment 3: bounded slice"`, first step / deliverable / completion signal as the design specifies |
| Deliverable | one commit adding `src/lib/greet.js` (exported `greet(name)`) and `src/lib/greet.test.js`, `bun test` passing |

## Measured

Every duration below comes from a record, not from the test process's own stopwatch: the hold from mesh's attention projection (`assignedAt` → `deliveredAt`), the resume from the app's `effort.resume.started` / `.completed` events, the member's time from `deliveredAt` → the `RESULT` message's own timestamp.

| Phase | Duration | Source |
|---|---:|---|
| Hold — assignment to notice delivery, spanning the whole effort switch | **1.91 s** | `assignedAt` → `deliveredAt` |
| ↳ of which: stop + relaunch + reattach | 1.63 s | `effort.resume.started` 07:31:28.216Z → `.completed` 07:31:29.850Z |
| ↳ of which: assignment to `appliedEffort = medium` observed | 2.15 s | task assign → runtime record poll (1 s poll granularity) |
| Member wall clock — notice delivered to `RESULT` | **32.15 s** | `deliveredAt` → `RESULT` message timestamp |
| Total — assignment to `RESULT` | **34.05 s** | `assignedAt` → `RESULT` message timestamp |
| Second assignment to a member launched at `medium`: assignment to delivery | 0.67 s | `assignedAt` → `deliveredAt` |
| mesh's effort-wait bound, which both holds were judged against | 180 s | `MESH_EFFORT_WAIT_SECS` default |

Two `it`s, both passing, 3 m 2.6 s wall clock for the spec including team setup, the second member's launch and teardown; 3 m 7 s for the run.

### What the records said

```
07:29:23.386Z {"event":"launch.command.rendered","tool":"codex","member":"codex-stage","reasoning_effort":"low",
               "command":"CODEX_HOME='…/codex-home' codex --yolo --dangerously-bypass-hook-trust
                          -c 'notify=[…]' -m 'gpt-5.6-sol' -c 'model_reasoning_effort=\"low\"'"}
07:31:28.216Z {"event":"effort.resume.started","member_name":"codex-stage","effort":"medium","previous_effort":"low"}
07:31:28.334Z {"event":"launch.command.rendered","tool":"codex","member":"codex-stage","reasoning_effort":"medium",
               "command":"CODEX_HOME='…/codex-home' codex resume '01a05192-b83a-7eb3-b539-b44521204f40'
                          --yolo --dangerously-bypass-hook-trust -c 'notify=[…]'
                          -m 'gpt-5.6-sol' -c 'model_reasoning_effort=\"medium\"'"}
07:31:29.850Z {"event":"effort.resume.completed","member_name":"codex-stage","effort":"medium","previous_effort":"low"}
07:32:03.457Z {"event":"launch.command.rendered","tool":"codex","member":"codex-stage-medium","reasoning_effort":"medium",
               "command":"CODEX_HOME='…/codex-home' codex --yolo --dangerously-bypass-hook-trust
                          -c 'notify=[…]' -m 'gpt-5.6-sol' -c 'model_reasoning_effort=\"medium\"'"}
```

The member's completion signal, in the lead's inbox:

```
RESULT #1 {"commit":"2db467ea494597c1f21e97f00a200a4bd1ad600f",
           "files":["src/lib/greet.js","src/lib/greet.test.js"],
           "validation":"bun test passed"}
```

The lane does not take that line on trust, and does not settle for "some commit exists, and the tree passes". `2db467e` had to differ from the commit the fixture started at, to be a commit object in the fixture repo, and to be *the* commit that added `src/lib/greet.js` and `src/lib/greet.test.js` (`git show --diff-filter=A`); `bun test` was then run in a clean detached worktree of that commit, not in the fixture's working tree, so a member that wrote the files and never committed them could not have passed. The payload itself is checked against the shape the completion signal asked for — `{commit, files, validation}`, with a real sha rather than a symbolic name.

## What this establishes

1. **The contract carries a real slice.** Create → assign → notice → accept → start → commit → `RESULT` with a machine-readable payload worked first time, with no steering, on a task the member had never seen. The member also used `mesh task accept` / `task start` on its own, which the assignment did not ask for — the mesh onboarding is enough for a member to drive the task lifecycle.
2. **The effort gate is real and effectively free.** mesh reported `pendingEffort: true` and `deliveryState: "pending"` for the assignment while the member ran at `low`; taurhaus stopped it, resumed the *same* conversation with `-c model_reasoning_effort="medium"`, `appliedEffort` caught up, and mesh delivered. The hold cost 1.91 s of a 34.1 s stage — about 6%.
3. **The gate acts on a mismatch, not on every assignment.** The negative path uses a member of its own, `codex-stage-medium`, *launched* at `medium` rather than resumed there by this lane: its launch seeded `appliedEffort = medium`, its assignment reported `pendingEffort: false` from the first read, the notice was delivered in 0.67 s, and neither member produced an `effort.resume.*` event.
4. **No effort wait expired, and that is now provable from mesh's own records.** taurhaus spawns the member daemon with `Stdio::null` (`coordination/runtime/process.rs`), so mesh's `effort wait expired for …` line reaches nobody. It does not have to: mesh releases a held notice for exactly two reasons — the member reached the level, or `now - assignedAt` reached the wait bound (`decide_notice_effort_gate`, 180 s unless `MESH_EFFORT_WAIT_SECS` says otherwise, and nothing re-arms it) — so a hold of 1.91 s against a 180 s bound cannot be an expiry. The lane also watches both records together while the level catches up, and fails where it happens if a `deliveredAt` ever appears while `appliedEffort` is still `low`: an expiry is invisible a moment later, because the relaunch lands and every reading then looks like a gate that closed properly.
5. **32.1 s of member time for one function and one test** is the number to compare the `codex exec` babysitter against. It is a floor, not a typical stage: the slice was deliberately tiny and needed no installs. (43.7 s on the 2026-08-29 run of the same lane — one sample either way.)

## Cost

The lane spends Codex subscription turns and nothing else.

- **Claude: zero.** The lead is launched into the isolated, credential-free `CLAUDE_CONFIG_DIR`, so it never authenticates and never takes a turn. It exists as a mesh identity and an inbox, which is all the completion signal needs.
- **Codex: two completed turns** on `gpt-5.6-sol`, both `codex-stage`'s and both counted from the daemon's own `codex.notify.appended` records (07:31:27.9 and 07:32:05.6): the turn that opens its thread — the onboarding notice and the one-word prompt went in together, see below — and the stage turn at `medium`. `codex-stage-medium` was launched, seeded `appliedEffort` and had its notice delivered, but the run tore its pane down ~15 s later and it completed no turn; the negative path asserts the delivery, not a reply, so it may spend a partial turn and never a whole one.
- **Token counts were not captured.** The lane does not read Codex's usage endpoint: that reports the whole account, not this run, and reading it would mean handling a live credential for a number that would not be attributable anyway. Turns, model and effort are the honest unit here.
- A run that fails before the member's first turn — as the first attempt did — costs nothing at all.

## Two findings for the design

**The onboarding notice can sit unsent in the composer, and a member that has taken no turn has no conversation to resume.** Measured twice on this host: the member's mesh daemon types the onboarding notice into the Codex pane and sends its own submit key, and when that lands while Codex is still starting the text stays in the composer. Codex opens its thread — and writes the rollout under `$CODEX_HOME/sessions` — on its *first turn*, so a member parked like this has no session id, and `pending_member_effort` refuses the effort switch outright ("member has no recorded session to resume"). The first run of this lane died there, before spending anything.

A bare `Enter` did not submit it; typing a one-word prompt and pressing Enter did, submitting both. The lane now does that and fails loudly if neither works. **For W4 this is a `stage()` concern, not a test concern**: a stage must not assign work to a member that has never taken a turn, because the effort it asks for cannot be put into force. The cheapest fix is for `stage()` to require a member whose runtime record names a session, and to prompt it once if it does not.

**Session binding under a scratch `CODEX_HOME` works.** The lane carries a documented fallback that reads the rollout id out of the scratch home itself if the scanner has not bound one; both measured runs reported `sessionBinding: "scanner"`, so the fallback was not used. `session_scanner/idle/codex.rs` resolves `$CODEX_HOME`, and the id arrived within seconds of the first turn.

## Host state worth recording

`~/.local/bin/mesh` was replaced with a stale bundled 0.2.22 build partway through this work (an app startup installing a bundled mesh whose binary did not match its own manifest). On 0.2.22 the gate does not exist and `mesh task get --json` simply omits `pendingEffort` — the lane would have failed on an `undefined` rather than on a named cause. It was restored with `just install-mesh`, which is lock-verified.

Because `coordination/mesh_cli.rs` resolves `~/.local/bin/mesh` by *absolute path* rather than through `PATH`, the mesh that holds the notice can be a different build from the one a test calls. The lane's host check therefore version-checks both and names the fix.

The run is invisible to the operator's own tmux. Checked while it was in flight: the lane's private server held three panes (the `new-session` pane in the checkout, the lead, the member) and the operator's server held the same eight panes before, during and after — none of them under the run's temp root, and no `set-environment` from this run on any session of theirs. `kill-server` at teardown took the private server and its panes with it, and the temp root was gone with the session.

## Limits

- One run, one host, one model (`gpt-5.6-sol`), one deliberately tiny slice. The wall clock is a floor; nothing here says what a feature-sized stage costs. The 2026-08-29 run of the earlier version of this lane measured 1.23 s / 43.7 s where this one measures 1.91 s / 32.1 s — two samples, one host, and the spread is the noise.
- The negative path asserts a delivery, not a reply: `codex-stage-medium` is torn down about fifteen seconds after its notice lands. That is deliberate — the assertion is that nothing was held — but it means the lane never sees that member work.
- The third `it` in the spec is the skip recorder; wdio's `afterTest` treats a `this.skip()` as not-passed and collects a failure-artifact bundle for it. A green run therefore still prints one `failure artifacts collected` line. That is the shared wdio config's behaviour, not a failure.
- Deadline and nudge semantics (experiment 4) and concurrent stages (experiment 5) are untouched here.
