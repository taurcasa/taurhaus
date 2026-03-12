## Summary

Taurhaus bundled Mesh was rebuilt from the frozen cutover commit
`af30eaa267a7f4bdc4a08a031b1c8744d393ef12` and repinned away from the older
`0b346da` build.

## What Was Rebuilt

- built `/tmp/mesh-cutover-af30-1223/target/release/mesh` from detached worktree commit
  `af30eaa267a7f4bdc4a08a031b1c8744d393ef12`
- reinstalled that exact binary to `~/.local/bin/mesh`
- rebundled Taurhaus `src-tauri/resources/mesh` from the same release build

The rebuilt binary reports:

- `version = 0.2.10`
- `protocol_version = 1`
- `schema_version = 1`
- `git_commit = af30eaa267a7f4bdc4a08a031b1c8744d393ef12`

## What Was Repinned

- `src-tauri/resources/mesh.lock.json`
- `src-tauri/resources/mesh.manifest.json`

Both now point at the full frozen cutover commit instead of the previous short
`0b346da` pin. `.gitignore` was updated so `mesh.manifest.json` is no longer an ignored
generated-only artifact; the tracked repo state now carries the same runtime compatibility
contract Taurhaus reads at install/status time.

## Verification

Verified locally that all relevant surfaces agree on:

- `version`
- `protocol_version`
- `schema_version`
- `git_commit`

Checked surfaces:

- `src-tauri/resources/mesh.lock.json`
- `src-tauri/resources/mesh.manifest.json`
- `src-tauri/resources/mesh version --json`
- `~/.local/bin/mesh version --json`

The bundled and installed paths now both match the frozen `af30eaa…` cutover build, and no
longer report the older `0b346da` identity.
