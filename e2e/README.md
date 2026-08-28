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
`PROTOCOL_VERSION` in `src-tauri/src/daemon/protocol.rs` (currently 13; 11, 12 and 13
each changed the wire contract). If you have just pulled a branch that bumped it, an
installed older daemon is rejected and every session-backed spec fails — reinstall it.

E2E sessions isolate their roots with `TAURHAUS_DATA_DIR` and `TAURHAUS_CLAUDE_DIR`,
plus the fixture path knobs `E2E_PROJECTS_DIR` and `E2E_TAURHAUS_PROJECT_PATH`.

## 2) Build the correct app binary for E2E

```bash
just build-e2e
```

Important: do **not** replace this with plain `cargo build`.
E2E requires the Tauri debug/no-bundle build so the app serves embedded assets correctly.

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

## Quick diagnosis for "Could not connect to localhost"

1. Verify daemon is listening:

```bash
ss -ltnp | rg 17233
```

Expected: a `LISTEN` line for `127.0.0.1:17233` owned by `taurhaus-daemon`.

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
