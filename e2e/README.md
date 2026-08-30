# E2E Runbook (Avoid `localhost connection refused`)

Use this sequence whenever running E2E locally.

This runbook covers the WebdriverIO + `tauri-driver` lane (`just test-e2e`,
`test-e2e-full`, `test-e2e-spec`), which runs on Linux only. Windows is not
supported there: the shared app data directory and tantivy index corruption
make reliable isolation impractical.

macOS has a separate lane. `just test-macos-e2e` (`justfile:785-790`) syncs the
tree to the Mac mini and runs `scripts/macos-e2e-test.sh` over SSH — a shell
smoke suite over the built `.app` bundle, tmux and the CLI harnesses, not a
WDIO suite. Nothing below applies to it.

## 1) Ensure daemon is current and running

```bash
just install-daemon
```

This rebuilds `taurhaus-daemon`, installs it to `~/.local/bin/`, and restarts it if it was running
(preserving the previous process's `TAURHAUS_*`/`RUST_LOG` env and re-passing normalized
`--data-dir`/`--port`).

The E2E recipes are **safe by default**: `just test-e2e`, `just test-e2e-full` and
`just test-e2e-spec` do *not* run `install-daemon` for you, so a daemon you are using
elsewhere is never restarted underneath you. Opt in explicitly when you want the rebuild:

```bash
E2E_INSTALL_DAEMON=1 just test-e2e
```

**Why this step matters:** the app validates the daemon's protocol version on every
connect path and refuses a mismatch outright rather than half-working. The constant is
`PROTOCOL_VERSION` in `src-tauri/src/daemon/protocol.rs` (currently 14). If you
have just pulled a branch that bumped it, an
installed older daemon is rejected and every session-backed spec fails — reinstall it.

Every WDIO worker puts all writable product roots under its session temp root:
`TAURHAUS_DATA_DIR`, `TAURHAUS_CLAUDE_DIR`, `CODEX_HOME`, `GROK_HOME` and the
taurhaus-only Antigravity override `TAURHAUS_AGY_DIR`. The fixture path knobs
`E2E_PROJECTS_DIR` and `E2E_TAURHAUS_PROJECT_PATH` point into that temp root too.
Ordinary workers receive an empty scratch Codex home; only a paid lane copies
`auth.json` into it.

Every worker also gets a private daemon port and a private tmux server. The
runner passes `TAURHAUS_DAEMON_PORT` to the app and its daemon launcher, points
`TMUX_TMPDIR` at `<session-temp-root>/tmux`, and removes an inherited `TMUX`
before tauri-driver starts. Teardown kills that tmux server; no spec addresses
the operator's server or port 17233.

Process cleanup is ownership-checked. A unique run token is inherited by the
driver, WebKitWebDriver, app, and daemons, and the runner records each live
process as PID plus Linux `/proc` start time in a checkout-scoped ledger under
the system temp directory. Pre-run cleanup reads only abandoned ledgers from
this checkout and kills only identities whose PID and start time still match.
A live concurrent run is left alone, as is a reused PID. The final in-run
fallback is limited to this worker's exact WebDriver ports.

## 2) Build the correct app binary for E2E

```bash
just build-e2e
```

Important: do **not** replace this with plain `cargo build`.
E2E requires the Tauri debug/no-bundle build so the app serves embedded assets correctly.

For the same reason, do not run `cargo build`, `cargo check --all-targets` or
`cargo clippy --all-targets` against `src-tauri/` between building and running E2E:
they overwrite `src-tauri/target/debug/taurhaus` with a binary that was not produced by
the Tauri build, and the app then starts, logs a healthy backend, and renders a blank
page — which surfaces as `App did not render within 45s`. Re-run `just build-e2e` (or
drop `E2E_SKIP_BUILD=1`) after any such cargo invocation. Running cargo *during* a live
E2E run replaces the binary underneath the app and ends the session with
`invalid session id`.

## 3) Run tests

Single spec (safe default, includes build):

```bash
just test-e2e-spec mesh-workflow
```

Single spec (fast path, only if you just built and know binary is fresh):

```bash
E2E_SKIP_BUILD=1 just test-e2e-spec mesh-workflow
```

Full suite:

```bash
just test-e2e-full
```

## Sealed spec manifest

`e2e/specList.js` is the complete manifest for default WDIO runs. Every
non-paid `e2e/specs/*.js` file must belong to one named group; adding an
ungrouped file makes `e2e/specList.test.js` and WDIO configuration fail with an
instruction to add it to a group or `paidSpecs`. Paid lanes never enter a suite
implicitly.

## Paid lanes

Two specs drive a real Codex subscription. `e2e/specList.js` keeps both out of the
config's spec list — a suite run, including a bare `bunx wdio run e2e/wdio.conf.js`,
never picks either up — and each only runs when asked for by name:

```bash
E2E_INSTALL_DAEMON=1 just test-e2e-spec compaction-codex-hooks
E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex
```

`compaction-codex-hooks` builds a managed team and pays for the turns that take its
Codex member to compaction, manually and automatically. `managed-stage-codex` (W4
experiment 3) hands a managed Codex member one bounded implementation task through
the mesh assignment contract: it proves the assignment's effort is put into force
before the notice is delivered, and then that the commit the member reports exists
in the fixture repo and the test it wrote passes.

Both read `~/.codex` once, copying only `auth.json` into a scratch `CODEX_HOME` under
the session temp root, and never write back to it. The scratch `config.toml` is
generated, not copied: the operator's own config can register things Codex executes
(`notify`, MCP servers), and a configured `notify` in particular would displace the
notifier taurhaus installs, which is the compaction lane's only turn signal. Point
`E2E_CODEX_SOURCE_HOME` somewhere else to copy from another Codex home. On a host
without `codex` ≥ 0.147, without Codex credentials, or without mesh/tmux, a lane
skips itself and prints why; `managed-stage-codex` additionally needs `mesh` ≥ 0.2.23
for the pending-effort gate and `bun` to validate the deliverable. See
[`docs/operations/compaction-testing.md`](../docs/operations/compaction-testing.md)
and [`docs/operations/testing-guide.md`](../docs/operations/testing-guide.md).

## Quick diagnosis for "Could not connect to localhost"

1. Read the worker port from `[e2e] daemon port for this worker: …`, then verify
   that port is listening:

```bash
ss -ltnp | rg '<worker-port>'
```

Expected: a `LISTEN` line for `127.0.0.1:<worker-port>` owned by
`taurhaus-daemon`. Port 17233 belongs to the operator and is not part of the
run.

2. If missing, restart daemon:

```bash
just install-daemon
```

3. Rebuild E2E binary (debug/no-bundle) and rerun:

```bash
just build-e2e
just test-e2e-spec mesh-workflow
```

## Quick diagnosis for "daemon connected but sessions are empty"

The daemon answers TCP but the app refuses it. Check the protocol pair:

```bash
cd src-tauri && rg 'PROTOCOL_VERSION: u32' src/daemon/protocol.rs
```

Then reinstall the daemon so the binary matches the checkout (`just install-daemon`).
The app's gate is exact-match, not a floor — a *newer* daemon is rejected the same way
an older one is. On a mismatch `ensure_expected_daemon_runtime` disconnects the daemon
(`startup/daemon.rs:380-395`), so look for `daemon.connection.lost` with
`reason: startup_runtime_mismatch` in `taurhaus.log.jsonl`, plus the
`daemon protocol mismatch: running=…, expected=…` error text.
`startup.daemon_protocol.checked` is a *separate*, conditional line: it is only emitted
while the daemon is still connected at that point in bootstrap
(`startup/daemon.rs:223-282`), and it labels only a lower version `outdated`. Do not
expect it on a rejected daemon.
