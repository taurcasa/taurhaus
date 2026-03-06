# Mesh Versioning + Bundling Contract (Joint Proposal)

Date: 2026-03-06  
Owner: architect  
Contributors: architect + mesh-expert

## 1. Problem Statement

Taurhaus bundles `mesh` in `src-tauri/resources/mesh` and installs it to `~/.local/bin/mesh`.  
Today, version matching is string-based (`mesh --version` vs `resources/mesh.version`), and mesh has no machine-readable version/compatibility metadata contract.

We need a reliable versioning and compatibility strategy that works for:

- local Linux/WSL builds
- `just build-windows`
- `just build-macos*` remote builds

## 2. Decisions

### 2.1 Versioning Scheme

Adopt SemVer now for mesh (starting from current `0.1.0`) with explicit pre-1.0 policy:

- release tags: `vX.Y.Z`
- while `<1.0.0`, treat `MINOR` as potentially breaking (major-equivalent)
- `1.0.0` marks contract freeze for compatibility guarantees

Reason:

- mesh is now an external dependency for taurhaus, so downstream pinning must be deterministic.
- commit-only pinning is insufficient for operator-facing compatibility and upgrade policy.

### 2.2 Runtime Version Contract

Mesh should expose machine-readable metadata:

- keep existing human output: `mesh --version` (e.g. `mesh 0.2.0`)
- add `mesh version --json` returning:
  - `version`
  - `git_commit`
  - `git_dirty`
  - `build_time_utc`
  - `protocol_version`
  - `schema_version`

`protocol_version` and `schema_version` are compatibility keys (not just SemVer).

JSON endpoint compatibility rule:

- within the same compatibility line, `mesh version --json` fields evolve additively only.
- required fields `version`, `protocol_version`, and `schema_version` cannot be renamed or removed without a breaking bump.

### 2.3 Taurhaus Compatibility Contract

Taurhaus pins and verifies:

- exact mesh version (`version`)
- required protocol compatibility (`protocol_version`)
- required schema compatibility (`schema_version`)
- optional exact commit pin (`git_commit`) for reproducible/dev builds

Recommendation: enforce equality for protocol/schema during pre-1.0.

## 3. Build/Bundling Recipe Changes

### 3.1 Add Lock Manifest In Taurhaus

Add a tracked lock file (proposal: `src-tauri/resources/mesh.lock.json` or repo-root `mesh.lock.json`):

```json
{
  "version": "0.2.0",
  "protocol_version": 1,
  "schema_version": 1,
  "git_commit": "optional-exact-pin",
  "release_tag": "v0.2.0",
  "sha256": "optional-for-released-artifact"
}
```

This becomes the source of truth for bundling and install checks.

Pinning policy:

- Taurhaus release builds should pin exact `version` + `sha256`.
- Optional `git_commit` pin is recommended for local/dev reproducibility.
- Compatibility can still be reasoned via protocol/schema keys and SemVer policy, but shipped bundles should use exact pins.

### 3.2 Add/Update `just` Recipes

Proposed recipe set:

1. `just mesh-verify-lock`
- build/locate mesh binary
- run `mesh version --json`
- fail if version/protocol/schema (and optional commit) do not match lock

2. `just update-mesh-lock <version> [--commit <sha>]`
- updates lock manifest intentionally (single bump entry point)
- optional: validates release tag exists and checksum matches published artifact

3. `just bundle-mesh` (replace current loose version read)
- depends on `mesh-verify-lock`
- bundles binary to `src-tauri/resources/mesh`
- writes:
  - `src-tauri/resources/mesh.version` (plain semver, backward compatible)
  - `src-tauri/resources/mesh.manifest.json` (full metadata snapshot)

4. `build-windows` and all `build-macos*`
- must depend on `mesh-verify-lock` before bundling
- remote mac recipes must verify remote-built mesh metadata matches lock before copying into resources

### 3.3 Remote macOS Specifics

Current macOS recipes build mesh on remote host after rsync.  
Add a verification step on remote:

- `~/projects/mesh/target/.../mesh version --json`
- compare against lock values passed from local build context
- fail build early on mismatch

## 4. Runtime Compatibility Checks In Taurhaus

Current state already blocks mesh usage in Mesh setup flow when installed and bundled versions differ (`check_mesh_install_status` + `MeshAvailabilityGate`).

Proposal:

1. Keep Mesh-tab gate as hard block for mesh operations.
2. Add protocol/schema checks (once `mesh version --json` exists), not just semver string checks.
3. Do not hard-block full app startup globally; show non-blocking warning outside Mesh if mismatch is detected.

Rationale:

- mismatch only affects coordination features, not entire app usability.
- avoids startup hard failures for users not using Mesh immediately.

## 5. Migration Path

Phase 0 (immediate, before mesh JSON endpoint lands):

- keep current `mesh --version` comparison path
- introduce lock manifest in taurhaus
- require `bundle-mesh` and platform build recipes to verify pinned semver (and optional commit pin by git check where possible)

Phase 1 (mesh PR lands):

- mesh adds `mesh version --json`, protocol/schema constants, build metadata stamping
- taurhaus build recipes switch to JSON verification
- taurhaus install/status commands prefer JSON endpoint, fallback to `--version` temporarily

Phase 2 (release discipline):

- mesh publishes tagged releases (`vX.Y.Z`) + sha256 artifacts
- `update-mesh-lock` can pin by release artifact checksum (not only local source tree)
- mesh install docs/scripts should use correct home expansion (`$HOME`, not `$$HOME`)

Phase 3 (contract freeze at `1.0.0`):

- SemVer major guarantees enforced
- protocol/schema compatibility policy formalized in mesh release notes

## 6. Answers To Team-Lead Questions

1. Should mesh adopt SemVer now?
- Yes. Immediately.

2. Which scheme?
- SemVer + explicit protocol/schema compatibility keys.
- Release bundles pin exact version/checksum; commit pinning is supplemental for reproducibility.

3. How should `build-windows` / `build-macos` enforce correct mesh?
- Require lock-manifest verification in recipes before bundling.
- Remote mac builds must verify remote binary metadata against same lock.

4. Need startup compatibility check?
- Keep hard check at Mesh gate; add non-blocking global warning only.
- Add protocol/schema validation when JSON endpoint is available.

5. Interim before full SemVer contract is in place?
- Use lock-manifest + strict current `--version` pinning now.
- Transition to `mesh version --json` as soon as mesh-side change lands.

## 7. Mesh-Side Feasibility (from mesh-expert)

Feasible minimal mesh PR:

- add `mesh version --json`
- stamp build metadata via `build.rs`
- expose `protocol_version` and `schema_version` constants
- add tests for JSON shape and non-empty version
- document release/tag/checksum workflow
- document install command examples with correct home expansion, e.g. `cargo install --path . --root "$HOME/.local" --force`

Operational note:

- daemon restart coordination remains necessary after binary replacement (running processes may still point at deleted inodes).
