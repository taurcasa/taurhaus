## Summary

Taurhaus now verifies Mesh against the pinned JSON compatibility contract instead of parsing
`mesh --version` text. The runtime install/status path, the frontend availability gate, and the
bundling recipes all compare the same fields:

- `version`
- `protocol_version`
- `schema_version`
- optional exact `git_commit`

`src-tauri/resources/mesh.lock.json` remains the build-time source of truth, and
`src-tauri/resources/mesh.manifest.json` remains the bundled runtime source of truth.

## What Changed

### Backend status and install verification

`src-tauri/src/commands/mesh.rs` now:

- reads the bundled compatibility contract from `mesh.manifest.json`
- invokes installed Mesh binaries with `mesh version --json`
- parses the JSON contract instead of scraping version text
- compares the installed contract against the bundled contract
- verifies native and WSL installs through the same JSON contract before reporting success

`MeshInstallStatus` now carries structured compatibility state:

- `bundled_contract`
- `installed_contract`
- `compatibility_issues`

This keeps the old `version`, `bundled_version`, and `needs_update` fields intact for compatibility
while exposing the real reason Taurhaus is blocking Mesh use.

### Frontend availability gate

`src/lib/components/MeshAvailabilityGate.svelte` now merges backend-provided compatibility issue
messages into the blocking prerequisite list. That means the user sees the actual reason for a
block, such as:

- version mismatch
- protocol mismatch
- schema mismatch
- exact pinned commit mismatch
- installed Mesh binary not supporting `mesh version --json`

### Bundling and verification recipes

`justfile` now verifies Mesh builds and remote macOS bundle inputs through `mesh version --json`
instead of `--version` string parsing.

Updated paths:

- `mesh-verify-lock`
- `bundle-mesh`
- `build-macos`
- `build-macos-intel`
- `build-macos-universal`

The local and remote bundle lanes still write `mesh.version` for backward compatibility, but the
verification authority is now the JSON contract.

## Verification

Focused verification for this cutover:

- `bunx vitest run src/lib/components/MeshAvailabilityGate.test.js src/lib/ipc.test.js`
- focused Rust command tests under `src-tauri/src/commands/mesh.rs`

## Notes

This cutover intentionally does not make `mesh.version` authoritative again. It remains a legacy
derived artifact so older tooling can still read a plain version string, while Taurhaus itself uses
the structured contract for compatibility decisions.
