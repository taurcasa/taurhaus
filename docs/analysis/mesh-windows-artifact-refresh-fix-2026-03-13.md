# Mesh Windows artifact refresh fix

Date: 2026-03-13
Author: dev-2

## Objective

Determine why the built Windows executable and NSIS installer did not refresh after the Taurhaus Mesh resource pin changed to `0.2.12`, even though the synced Windows resource tree already contained the new Mesh payload.

## Initial failure state

After the Mesh rollout repin:

- Taurhaus source tree resources were updated to Mesh `0.2.12`
- the synced Windows build tree at `D:\taurhaus_build` also had:
  - `src-tauri/resources/mesh.version = 0.2.12`
  - bundled Mesh JSON identity `0.2.12 / fabb518681d6f4336e715ae2a22ed2f3166b4db9`

But the built Windows artifacts remained stale:

- `D:\taurhaus_build\src-tauri\target\release\taurhaus.exe` timestamp stayed at `2026-03-13 09:05:10`
- `D:\taurhaus_build\src-tauri\target\release\bundle\nsis\taurhaus_0.5.10_x64-setup.exe` timestamp stayed at `2026-03-13 09:05:10`

Installing that stale NSIS artifact produced an installed Windows app that still bundled Mesh `0.2.11`.

## Root cause

The stale-artifact behavior was caused by the Windows build being launched through a PTY.

Observed PTY failure mode:

- `just build-windows` entered the `windows_build` phase
- the wrapper emitted `ESC[6n`
- the Windows-side build never reached the PowerShell-script output lines such as `[windows_bun_install] starting...`
- no fresh Windows exe or installer was written, so the previous `09:05` artifacts remained on disk

That means the synced Windows resource tree was updated correctly, but the actual native Windows build step never completed, so the stale Windows installer/exe were left in place and later installed.

## Fix

### 1. Run the Windows build non-PTY

Re-ran:

- `MESH_PROJECT=/tmp/mesh-1255 just build-windows`

without a PTY-backed terminal session.

This time the Windows phase progressed normally:

- `[windows_bun_install] starting...`
- `[windows_cargo_tauri_build] starting...`
- `Built application at: D:\taurhaus_build\src-tauri\target\release\taurhaus.exe`
- `Running makensis to produce ...\taurhaus_0.5.10_x64-setup.exe`

Fresh artifact timestamps after the successful non-PTY run:

- `D:\taurhaus_build\src-tauri\target\release\taurhaus.exe` -> `2026-03-13 14:04:57`
- `D:\taurhaus_build\src-tauri\target\release\bundle\nsis\taurhaus_0.5.10_x64-setup.exe` -> `2026-03-13 14:04:57`

### 2. Strengthen build invalidation for bundled resources

Updated `src-tauri/build.rs` to declare bundled resource inputs explicitly:

- `tauri.conf.json`
- `resources/taurhaus-daemon`
- `resources/mesh`
- `resources/mesh.version`

This makes the Rust/Tauri build graph more explicit when bundled resource files change.

## Verification

### Windows build artifacts

After the non-PTY rerun:

- fresh exe and NSIS installer timestamps confirmed the Windows artifacts were rebuilt

### Windows install verification

Re-ran:

- `MESH_PROJECT=/tmp/mesh-1255 just install-windows`

Installed app verification:

- installed exe: `C:\Users\mstie\AppData\Local\taurhaus\taurhaus.exe`
- installed exe timestamp: `2026-03-13 14:04:34`
- installed `resources/mesh.version`: `0.2.12`
- installed bundled Mesh JSON identity:
  - version: `0.2.12`
  - git commit: `fabb518681d6f4336e715ae2a22ed2f3166b4db9`
  - protocol version: `1`
  - schema version: `1`

Installed Mesh payload hash matches the synced Windows build-tree Mesh payload hash:

- `2e7a2a73e84917396d67dfdfba33e898f83c079cbe92e2eddd052229964919e4`

## Conclusion

The stale Windows artifacts were not caused by a bad Mesh pin or a bad sync. The actual problem was that the PTY-launched Windows build stalled before the native Windows build step completed, leaving previous artifacts in place.

The fix path is:

1. run the Windows build without a PTY-backed terminal session
2. keep explicit bundled-resource invalidation in `src-tauri/build.rs`

With that combination in place, the Windows artifacts refresh correctly and the installed app now bundles Mesh `0.2.12`.
