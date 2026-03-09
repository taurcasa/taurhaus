# Mac terminal and tmux validation - 2026-03-09

## Scope

Validated the current macOS terminal and tmux management path for Taurhaus and mesh on the Mac Mini. The focus was:

- terminal launch behavior for `iterm2`, `ghostty`, and `terminal_app`
- tmux layout behavior for `new_window`, `split`, and `per_project`
- mesh wake-message delivery into plain shell panes
- real runtime configuration on the Mac Mini

## Environment

- host: Mac Mini (`m1@62.210.195.235`)
- shell: `/bin/zsh`
- tmux: `/Users/m1/.homebrew/bin/tmux`
- installed terminal apps:
  - `iTerm.app`
  - `Ghostty.app`
  - `Terminal.app`
- current Taurhaus settings from the app DB:
  - `terminal.emulator=iterm2`
  - `terminal.tmux_layout=new_window`
  - `terminal.custom_command=` empty

## Code paths reviewed

- `src-tauri/src/terminal.rs`
- `src-tauri/src/session_scanner/control.rs`
- `src-tauri/src/models/mod.rs`
- `src-tauri/src/db/settings_queries.rs`
- `src-tauri/src/commands/command_center/launching.rs`
- `src-tauri/src/commands/command_center/navigation.rs`
- `src-tauri/src/commands/terminal_settings.rs`
- `/home/mstie/projects/mesh/src/daemon.rs`

## Findings

### 1. Ghostty macOS launch path had a real bug

Taurhaus was launching Ghostty on macOS with `Command::new("ghostty")`. That assumes a CLI shim on `PATH`.

On the Mac Mini:

- `Ghostty.app` is installed and discoverable by LaunchServices
- `command -v ghostty` returns nothing

That means Taurhaus could not rely on a shell-visible `ghostty` binary even though Ghostty was installed correctly.

### 2. Tmux layout behavior is correct

Headless tmux validation on the Mac Mini matched the intended layout semantics.

Observed results:

- `new_window`
  - windows created: `3`
  - names: `seed, project-a, project-b`
- `split`
  - panes in the first project window before overflow: `4`
  - overflow creates a second window as expected
- `per_project`
  - windows created: `3`
  - names/counts: `seed:1, project-a:2, project-b:1`

Conclusion: tmux layout logic in `session_scanner/control.rs` is behaving correctly on macOS.

### 3. Plain-shell mesh wake delivery had a real bug class

Earlier Mac validation showed daemon wake text landing at a plain `zsh` prompt could produce:

- literal `[mesh] ...`
- followed by `zsh: no matches found: [mesh]`

That is a shell-injection format bug, not a tmux bug. The wake line was being delivered as raw text into a shell prompt.

### 4. GUI client attachment is only partially verifiable over the current SSH path

From the remote Mac session I could observe GUI process launch for both iTerm2 and Ghostty, but not reliably prove tmux client attachment through the headless SSH/Aqua boundary.

What was observed:

- iTerm2 processes launched
- Ghostty launched with `-e tmux attach-session -t <session>`
- but `tmux list-clients -t <session>` did not provide a reliable assertion that the GUI app had attached to the session from this remote validation channel

Conclusion: GUI process launch is verified. End-to-end GUI tmux attachment is environment-limited over this remote path and should not be overclaimed.

## Fixes landed

### Taurhaus

File:

- `src-tauri/src/terminal.rs`

Change:

- Ghostty launch on macOS now uses LaunchServices:
  - `open -a Ghostty --args -e tmux attach-session -t <session>`
- added a small regression seam and macOS-only test coverage

Validation:

- remote macOS cargo test passed:
  - `ghostty_launch_uses_launchservices_with_tmux_attach_args`

### Mesh

File:

- `/home/mstie/projects/mesh/src/daemon.rs`

Change:

- tmux delivery is now shell-aware
- for shell panes, mesh sends a `printf '%s\n' '...'` payload instead of raw free-form text
- CLI panes keep the existing direct send behavior

Validation:

- local mesh tests passed:
  - `shell_single_quote_escape_handles_single_quotes`
  - `shell_detection_matches_supported_shells`
- remote Mac mesh tests passed after sync and rebuild

## Practical conclusion

The Mac terminal/tmux stack is in acceptable shape after these fixes.

Confirmed working:

- macOS settings resolution
- tmux layout behavior for all supported layout modes
- Ghostty launch path no longer depends on a missing PATH shim
- mesh wake delivery is hardened for plain shell prompts

Remaining caveat:

- remote SSH validation can prove GUI process launch, but not reliably prove interactive GUI tmux attachment from the same headless channel
- if a stronger assertion is needed later, it should be done with a local interactive macOS session or a GUI-capable automation path rather than more SSH probing
