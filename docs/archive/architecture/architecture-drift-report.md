# ARCHITECTURE.md Drift Report

Review date: 2026-03-07

Scope:
- `ARCHITECTURE.md`
- backend module layout under `src-tauri/src/`
- frontend module layout under `src/lib/`
- Tauri IPC registration in `src-tauri/src/lib.rs`
- infographic manifest in `docs/images/infographics.manifest.yaml`

Verified accurate as written:
- SQLite storage summary and six-table count at [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:104)
- total IPC command count of `82` at [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:111)
- daemon protocol method count of `21` at [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:239)

## Findings

1. System overview overstates daemon ownership of file watching.
- Doc lines: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:7), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:11), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:12)
- Current code:
  - local/native project watches are created by the app via [`watchers.rs`](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs:28)
  - daemon watch bootstrap/reconcile is only used for WSL targets via [`daemon_lifecycle.rs`](/home/mstie/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs:98)
  - local file classification and `.gitignore` handling live in [`watcher.rs`](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs:13)
- Correction needed:
  - reword the overview so the app owns native/local file watching
  - describe the daemon as the WSL-side watcher/process scanner/tmux bridge, not the universal file-watching owner

2. Platform abstraction section omits the Windows implementation and current command-spawn helper.
- Doc lines: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:18), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:28)
- Current code:
  - platform dispatch now includes Linux, macOS, and Windows in [`platform/mod.rs`](/home/mstie/projects/taurhaus/src-tauri/src/platform/mod.rs:11)
  - Windows has explicit stubbed process-inspection implementations in [`windows.rs`](/home/mstie/projects/taurhaus/src-tauri/src/platform/windows.rs:1)
- Correction needed:
  - update this section to mention the Windows stub module
  - note that `apply_background_command_settings()` is also part of the platform boundary for hidden-window process spawning on Windows

3. Frontend IPC description points at the wrong file.
- Doc line: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:60)
- Current code:
  - [`src/lib/ipc.js`](/home/mstie/projects/taurhaus/src/lib/ipc.js:1) is only a compatibility re-export
  - real IPC modules live under `src/lib/ipc/`
- Correction needed:
  - change the description to say `src/lib/ipc.js` is a thin compatibility export and `src/lib/ipc/` holds the actual domain modules and mock fallbacks

4. Backend module map is missing several active top-level modules and understates the daemon/provider boundary.
- Doc lines: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:67), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:78), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:81), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:89)
- Current code:
  - top-level modules include [`daemon_api.rs`](/home/mstie/projects/taurhaus/src-tauri/src/daemon_api.rs), [`project_provider.rs`](/home/mstie/projects/taurhaus/src-tauri/src/project_provider.rs), and [`watch_targets.rs`](/home/mstie/projects/taurhaus/src-tauri/src/watch_targets.rs) via [`lib.rs`](/home/mstie/projects/taurhaus/src-tauri/src/lib.rs:16)
  - `daemon/` contains protocol/server/listener/launcher concerns, while lifecycle orchestration is split into [`daemon_lifecycle.rs`](/home/mstie/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs:1)
- Correction needed:
  - add rows for `daemon_api.rs`, `project_provider.rs`, and `watch_targets.rs`
  - tighten the `daemon/` and `provider/` descriptions so they match the actual split

5. IPC group breakdown is stale for coordination commands.
- Doc line: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:127)
- Current code:
  - `coordination` contributes `15` commands in [`lib.rs`](/home/mstie/projects/taurhaus/src-tauri/src/lib.rs:241)
  - the current set includes `coordination_resume_team`, `coordination_reonboard`, `coordination_get_live_team_status`, `coordination_preflight_check`, `coordination_get_feature_availability`, and `coordination_get_project_mesh_snapshot`
- Correction needed:
  - update `Coordination (13)` to `Coordination (15)`
  - mention live-status/snapshot/preflight coverage so the summary matches the current surface

6. Startup sequence says watchers are registered for all projects, but the implementation is activity-based and split across local and daemon watch plans.
- Doc lines: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:253), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:254), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:260)
- Current code:
  - startup creates a local watcher and event processor in [`watchers.rs`](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs:28)
  - actual watch selection is activity-threshold based through [`watch_targets.rs`](/home/mstie/projects/taurhaus/src-tauri/src/watch_targets.rs:1)
  - WSL watches are separately reconciled via [`daemon_lifecycle.rs`](/home/mstie/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs:98)
  - the startup path also ensures a dedicated Claude task-directory watch in [`watchers.rs`](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs:255)
- Correction needed:
  - replace “register watchers for all projects” with activity-based local/daemon watch reconciliation
  - mention the dedicated Claude task-directory watch

7. Data-flow section attributes filesystem change detection entirely to the daemon.
- Doc lines: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:274), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:275), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:276)
- Current code:
  - native/local changes come from [`ProjectWatcher`](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs:192)
  - WSL changes come from daemon watch/event-listener plumbing in [`daemon_lifecycle.rs`](/home/mstie/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs:176)
- Correction needed:
  - split this flow into local watcher and WSL daemon watcher paths instead of describing one daemon-only path

8. The system architecture diagram is definitely stale, and the startup/data-flow diagrams should be flagged pending regeneration.
- Doc lines: [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:14), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:248), [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md:266)
- Current evidence:
  - the system architecture prompt still hardcodes `IPC (66 cmds)` and “File Watcher” inside the daemon in [`infographics.manifest.yaml`](/home/mstie/projects/taurhaus/docs/images/infographics.manifest.yaml:553)
  - `data-flow` and `startup-sequence` are older unmanaged assets with no current-generation prompt metadata in [`infographics.manifest.yaml`](/home/mstie/projects/taurhaus/docs/images/infographics.manifest.yaml:133) and [`infographics.manifest.yaml`](/home/mstie/projects/taurhaus/docs/images/infographics.manifest.yaml:524)
- Correction needed:
  - explicitly flag these diagrams in `ARCHITECTURE.md` as pending regeneration
  - note why: current IPC count, local-vs-daemon watcher split, and updated startup/watch behavior
