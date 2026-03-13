# Project Path Memory

## Objective

Remember the last path used in Taurhaus project add/create flows so operators do not have to keep re-entering the same directory when switching between manual add and create-project workflows.

## Implementation

The remembered path is stored in shared settings as `project_dialog_last_path`. The backend persists that field through the existing settings SQLite path, and the frontend restores it when the add-project modal opens.

The restore and update behavior lives in `AddProjectModal.svelte`:

- manual add restores the remembered path into the manual path field when present
- create-project restores the same remembered path into the parent directory field when present
- create-project still falls back to `~/projects` when no remembered path exists
- the shared remembered path is updated on manual-path blur, manual directory selection, successful manual add, create-parent blur, create-parent directory selection, and successful project creation

`CreateWorkflow.svelte` and `ManualWorkflow.svelte` now also seed their directory browsers from the current field value so the picker opens from the restored path instead of starting from a generic home directory.

## Files Changed

- `src-tauri/src/models/mod.rs`
- `src-tauri/src/db/settings_queries.rs`
- `src-tauri/src/commands/settings.rs`
- `src/lib/ipc/system.js`
- `src/lib/ipc/mocks/base.js`
- `src/lib/ipc.test.js`
- `src/lib/mockData.test.js`
- `src/lib/AddProjectModal.svelte`
- `src/lib/AddProjectModal.test.js`
- `src/lib/CreateWorkflow.svelte`
- `src/lib/ManualWorkflow.svelte`

## Validation

- `cargo test settings_commands_get_and_update_round_trip --lib`
- `cargo test save_and_load_settings_roundtrip --lib`
- `bunx vitest run src/lib/AddProjectModal.test.js src/lib/ipc.test.js src/lib/mockData.test.js`
- `just check-quick`
