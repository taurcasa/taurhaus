# Build and release

End-to-end build and release procedures for taurhaus across Linux, Windows, and macOS. This document expands on the quick-reference tables in [CLAUDE.md](../../CLAUDE.md#build--development), with detailed steps, constraints, and troubleshooting.

![Build and Release Pipeline](../images/build-release-pipeline.jpg)

## Overview

Build and release operations are standardized in `justfile`. Use `just` recipes only.

Core rules:

- Do not run ad-hoc cross-compilation for Windows or macOS from WSL/Linux.
- Build artifacts on their native target platforms (Windows via `cmd.exe`, macOS via remote Mac SSH).
- Use `just release` to publish; do not create/edit GitHub releases manually.

## Prerequisites

| Dependency | Why it is required |
|---|---|
| `just` | Entry point for all supported build/release workflows. |
| Rust toolchain (`cargo`, `rustc`, `clippy`, `rustfmt`) | Backend build, tests, lint, and packaging. |
| Node.js + Bun (Bun available on build hosts) | Frontend build and Tauri frontend pipeline. |
| Tauri CLI | App bundling (`cargo tauri build` or `bunx tauri build` via recipes). |
| `rsync` + `ssh` | Remote sync/build on macOS host. |
| Windows `cmd.exe` interop from WSL | Native Windows NSIS build from synced workspace. |
| `gh` CLI (authenticated) | `just release` creates GitHub releases and uploads artifacts. |
| macOS `codesign` + `lipo` (on remote Mac) | Daemon signing and universal binary assembly. |

Project-specific environment assumptions from `justfile`:

- Windows sync/build directory: `D:\taurhaus_build` (WSL path `/mnt/d/taurhaus_build`)
- macOS build host: `m1@62.210.195.235`
- macOS remote project path: `~/projects/taurhaus`

## Development commands

| Recipe | Purpose |
|---|---|
| `just dev` | Full Tauri development mode (frontend + backend hot-reload). |
| `just dev-frontend` | Frontend-only development server. |
| `just check-quick` | Standard iteration gate (`cargo check --tests`, typecheck, frontend tests). |
| `just check` | Full quality gate (`fmt`, lint, typecheck, tests) for team-lead serialized runs or release validation. |

Use the quick-reference table in [CLAUDE.md](../../CLAUDE.md#build--development) for the full dev/test matrix.

## Platform build procedures

### Linux build

Use when validating Linux packaging artifacts.

```bash
just build-linux
```

What it runs:

- `bun run tauri build`

### Windows build (native via WSL interop)

Use this for production Windows installers.

```bash
just build-windows
```

Pipeline summary:

1. `install-daemon` (WSL daemon binary rebuilt/installed).
2. `bundle-daemon` (copies daemon binary into `src-tauri/resources/`).
3. `sync-windows` (rsync to `D:\taurhaus_build`).
4. `cmd.exe /c "cd /d D:\taurhaus_build && bun install --frozen-lockfile"` (with `%USERPROFILE%\.bun\bin\bun.exe` fallback from WSL).
5. `cmd.exe /c "cd /d D:\taurhaus_build && set PATH=%USERPROFILE%\.bun\bin;%PATH% && cargo tauri build --bundles nsis"`.

Expected artifact location:

- `/mnt/d/taurhaus_build/src-tauri/target/release/bundle/nsis/*.exe`

Troubleshooting:

- `UNC paths are not supported`: informational from `cmd.exe` in this workflow; safe to ignore.
- `Access is denied` during `.exe` output: the app is still running on Windows. Close it and rebuild.
- Do not use `cargo xwin` or `--target x86_64-pc-windows-msvc` from WSL for release builds.

### macOS build (arm64 on remote Mac)

Use for native arm64 `.dmg` output.

```bash
just build-macos
```

Pipeline summary:

1. `sync-macos` via rsync.
2. Remote `bun install --frozen-lockfile` using `zsh -ilc` login shell.
3. Remote daemon release build (`cargo build --release --bin taurhaus-daemon`).
4. Copy daemon to `~/.local/bin/` and `src-tauri/resources/`.
5. Re-sign daemon binaries (`codesign --force --sign - ...`).
6. Remote app build (`cargo tauri build`).
7. Copy artifacts back to `builds/macos-aarch64/`.

Why login shell matters:

- The recipe intentionally uses `zsh -ilc` so PATH includes `bun`, `cargo`, Homebrew tools, and expected environment vars.

Expected artifact location:

- `builds/macos-aarch64/*.dmg`
- `builds/macos-aarch64/taurhaus-daemon-aarch64`

### macOS universal build (arm64 + x86_64)

Use for one universal release artifact.

```bash
just build-macos-universal
```

Pipeline summary:

1. Build daemon for `aarch64-apple-darwin`.
2. Build daemon for `x86_64-apple-darwin`.
3. Combine with `lipo -create` into `target/universal-apple-darwin/release/taurhaus-daemon`.
4. Re-sign universal daemon and bundled resource copy.
5. Build app with `cargo tauri build --target universal-apple-darwin`.
6. Copy `.dmg` to `builds/macos-universal/`.

Expected artifact location:

- `builds/macos-universal/*.dmg`

Daemon signing note:

- Re-signing after copy is required for modern macOS compatibility (Sequoia linker-signature behavior).

## Mesh CLI build and bundle

The mesh binary is built from a separate project and bundled into `src-tauri/resources/` alongside a version file.

```bash
just build-mesh       # Build mesh release binary from $MESH_PROJECT (default: ~/projects/mesh)
just bundle-mesh      # Build + copy binary and version to src-tauri/resources/
```

`bundle-mesh` writes two files:
- `src-tauri/resources/mesh` — the mesh binary
- `src-tauri/resources/mesh.version` — pinned version string (read at runtime by `check_mesh_install_status`)

The `build-linux`, `build-windows`, and `build-macos` recipes automatically include `bundle-mesh` as a dependency.

## Daemon build and install

Use daemon recipes when validating daemon runtime independently or before app builds.

```bash
just build-daemon
just install-daemon
```

`install-daemon` behavior:

- Stops running daemon if present.
- Builds release daemon binary.
- Atomically installs to `~/.local/bin/taurhaus-daemon`.
- Restarts daemon if it had been running.

## Release workflow

Canonical release flow:

1. `just bump <VERSION>`
2. Edit `CHANGELOG.md` entry for that version.
3. Commit the version/changelog changes.
4. `just check` (team-lead serialized full gate)
5. Build release artifacts (`just build-windows` and macOS target recipe).
6. `just release`

`just release` enforcement checks:

- Current branch must be `main`.
- Working tree must be clean.
- Tag `v<VERSION>` must not already exist.
- At least one version-matching artifact must be found.

Artifact upload sources:

- `builds/macos-universal/*.dmg`
- `builds/macos-aarch64/*.dmg`
- `builds/macos-x86_64/*.dmg`
- `D:\taurhaus_build\src-tauri\target\release\bundle\nsis\*.exe`

Notes:

- `just release` pushes `main` before creating the GitHub release.
- Release notes are extracted from the matching `CHANGELOG.md` section.
- Never replace assets on an existing release; bump and release a new version instead.

## Version management (`just bump`)

`just bump <VERSION>` updates:

- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `package.json`
- `src-tauri/Cargo.lock` (regenerated via `cargo check`)
- `CHANGELOG.md` (adds `## [VERSION] - YYYY-MM-DD` under `[Unreleased]` if missing)

After bumping:

- Fill in changelog content.
- Commit before building/releasing.

## Troubleshooting quick reference

| Symptom | Likely cause | Fix |
|---|---|---|
| `Access is denied` in Windows build | Existing app process has installer/exe locked | Close running app/processes, rerun `just build-windows` |
| macOS build cannot find `cargo`/`bun`/Homebrew tools | Non-login shell environment on remote Mac | Use recipes as-is (`zsh -ilc`); avoid manual non-login SSH commands |
| macOS app/daemon blocked after copy | Unsigned copied daemon binary | Ensure `codesign --force --sign -` runs on daemon install/resource copy |
| `just release` refuses due to dirty tree | Uncommitted changes | Commit or stash, then rerun |
| `just release` says tag exists | Version already released/tagged | Bump to a new version and run release again |
| `just release` finds no artifacts | Build recipes not run for current version | Run build recipes first, then rerun release |

## Key files

| File | Purpose |
|---|---|
| `justfile` | Source of truth for build, daemon, version, and release automation. |
| `CLAUDE.md` | Quick-reference build/development tables and release policy constraints. |
| `src-tauri/tauri.conf.json` | App version source used by release tagging (`v<version>`). |
| `CHANGELOG.md` | Release notes source section consumed by `just release`. |
| `docs/architecture/daemon-protocol.md` | Daemon runtime/signing context for macOS packaging. |

## Related documents

- [CLAUDE.md Build & Development](../../CLAUDE.md#build--development) — quick recipe table.
- [CLAUDE.md Release Workflow](../../CLAUDE.md#release-workflow) — policy-level release rules.
- [Testing guide](testing-guide.md) — validation lanes that pair with build/release gates.
