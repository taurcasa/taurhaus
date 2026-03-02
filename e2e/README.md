# E2E Runbook (Avoid `localhost connection refused`)

Use this sequence whenever running E2E locally.

## 1) Ensure daemon is current and running

```bash
just install-daemon
```

This rebuilds `taurhaus-daemon`, installs it to `~/.local/bin/`, and restarts it if it was running.

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
