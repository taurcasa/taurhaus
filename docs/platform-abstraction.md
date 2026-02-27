# Platform Abstraction — Design Doc

## Problem

taurhaus currently targets Windows (native exe) + WSL2 (daemon). macOS support requires abstracting all Linux-specific (`/proc`) and Windows-specific (`wsl.exe`, `wt.exe`) code behind compile-time dispatched platform modules.

## Strategy

**Compile-time dispatch with `cfg(target_os)`** at the module level. No runtime branching, no trait objects. Each platform gets its own concrete implementation file. The public API is identical across platforms.

---

## 1. Platform Module Structure

```
src-tauri/src/platform/
  mod.rs          # cfg-based re-exports
  linux.rs        # LinuxProbe — /proc filesystem
  darwin.rs       # DarwinProbe — libproc + lsof
  windows.rs      # No-op stubs (session scanning via WSL daemon)
  types.rs        # Shared types (ProcessInfo, ActivityState)
```

### mod.rs

```rust
mod types;
pub use types::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;
```

No trait — just matching function signatures. The compiler ensures the API contract at build time. This is simpler than `dyn Trait` and has zero runtime cost.

**Windows note**: On Windows, CLI tools run inside WSL2, not as native Windows processes. The daemon (a Linux binary in WSL) handles session scanning using `linux.rs`. The `windows.rs` stubs return `None`/empty — the app's direct `scan_sessions()` fallback compiles but produces empty results, which is correct behavior (the daemon path provides the real data).

---

## 2. Platform API Surface

Functions that each platform must implement:

### Process Detection

```rust
/// Get the working directory of a process.
pub fn process_cwd(pid: u32) -> Option<PathBuf>

/// Get the TTY/pts path for a process's stdin.
pub fn process_tty(pid: u32) -> Option<String>
```

**Linux**: Read `/proc/{pid}/cwd` and `/proc/{pid}/fd/0` symlinks.
**macOS**: Use `libproc::proc_pidpath()` for binary path, `libproc::proc_pidinfo()` with `PROC_PIDVNODEPATHINFO` for cwd. TTY via `libproc::proc_pidfdinfo()`.

### IO Activity Detection

```rust
/// Read cumulative bytes read by a process (for IO hysteresis).
pub fn process_rchar(pid: u32) -> Option<u64>
```

**Linux**: Parse `rchar:` from `/proc/{pid}/io`.
**macOS**: Use `proc_pid_rusage()` from `libproc` crate → `ri_diskio_bytesread`.

### TCP Socket Detection

```rust
/// Check if a process has any ESTABLISHED TCP connections to port 443.
pub fn has_established_443(pid: u32) -> bool
```

**Linux**: Cross-reference `/proc/{pid}/fd/` socket inodes with `/proc/{pid}/net/tcp`.
**macOS**: `lsof -i TCP:443 -a -p {pid} -s TCP:ESTABLISHED -t` (shell out). Or use `proc_pidfdinfo()` with `PROC_PIDFDSOCKETINFO`.

### Types

```rust
pub struct ProcessInfo {
    pub pid: u32,
    pub cwd: Option<PathBuf>,
    pub tty: Option<String>,
}
```

---

## 3. Daemon Launcher Abstraction

The daemon lifecycle differs fundamentally between platforms:

| Aspect | Windows/WSL | macOS | Linux (dev) |
|--------|------------|-------|-------------|
| Binary location | `~/.local/bin/taurhaus-daemon` in WSL | `~/.local/bin/taurhaus-daemon` native | Same |
| Spawn mechanism | `wsl.exe -d {distro} -- {binary}` | Direct `Command::new(binary)` | Direct |
| Keep-alive | Long-lived wsl.exe parent (WSL#4649) | Daemon self-daemonizes or app keeps handle | Same as macOS |
| Console hiding | `CREATE_NO_WINDOW` flag | Not needed | Not needed |

### Abstraction

```rust
// daemon/launcher.rs keeps the existing Windows code behind cfg(target_os = "windows")
// Add new block:

#[cfg(target_os = "macos")]
fn try_start_daemon(port: u16, log_path: &Path) -> Result<(), std::io::Error> {
    let home = std::env::var("HOME").map_err(std::io::Error::other)?;
    let binary = format!("{home}/.local/bin/taurhaus-daemon");
    // Direct spawn — no WSL wrapper needed
    Command::new(&binary)
        .args(["--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}
```

The daemon binary itself is cross-platform Rust — it compiles natively for macOS. No WSL indirection.

---

## 4. Terminal Integration

**Source of truth**: `src-tauri/src/terminal.rs`

### Unified Decision Tree (all platforms)

```
handle_terminal(intent)
  ├── FocusOnly { emulator }
  │   └── Resolve emulator → is it running? → activate it
  │
  └── EnsureOpen { emulator, tmux_session, ... }
      ├── "custom" → run user's command template with placeholders
      └── Resolve emulator → is it running + tmux has client?
          ├── Yes → activate (no duplicate tab/window)
          └── No  → launch with `tmux attach-session -t <session>`
```

The tree is **identical on every platform**. Only the concrete emulator options and detection mechanisms differ:

| Platform | Emulators | Default | Detection | Launch |
|----------|-----------|---------|-----------|--------|
| Windows | Windows Terminal, Custom | `windows_terminal` | PowerShell `Get-Process` + `EnumWindows` (WinUI 3 quirk) | `wt.exe -w taurhaus new-tab -- wsl.exe -d {distro} -- tmux attach` |
| macOS | iTerm2, Ghostty, Terminal.app, Custom | `iterm2` | AppleScript `application "X" is running` + `tmux list-clients` | AppleScript (`create tab`/`do script`) or CLI (`ghostty -e`) |
| Linux | (no-op) | — | — | User manages their own terminal |

### macOS: `MacEmulator` Enum

```rust
enum MacEmulator { ITerm2, Ghostty, TerminalApp }

impl MacEmulator {
    fn from_setting(pref: &str) -> Self;   // resolve setting → concrete emulator
    fn is_running(self) -> bool;           // AppleScript check
    fn activate(self) -> Result<(), String>;  // bring to front
    fn launch_with_tmux(self, session: &str) -> Result<(), String>;  // open + attach
}
```

Key invariant: we **always respect the user's emulator preference**. We never fall through to a different terminal just because it happens to be running. `tmux list-clients` (not `pgrep`) determines attachment state.

---

## 5. Daemon Install (macOS)

On macOS, there's no WSL — the daemon runs natively. The wizard flow simplifies:

| Step | Windows | macOS |
|------|---------|-------|
| Bundle binary | Linux ELF in resources | macOS binary in resources |
| Check exists | `wsl -d {distro} -- test -f ~/.local/bin/...` | `test -f ~/.local/bin/...` |
| Install | Copy via `wsl -d {distro} -- cp /mnt/...` | Direct `fs::copy()` |
| Permissions | `wsl -d {distro} -- chmod +x` | Direct `fs::set_permissions()` |

The `check_daemon_install_status` and `install_daemon` commands need `cfg` blocks for macOS-native paths.

---

## 6. File System Roots

```rust
// commands/projects.rs get_system_roots()

#[cfg(target_os = "macos")]
fn get_system_roots() -> Vec<SystemRoot> {
    vec![
        SystemRoot { path: "/".into(), label: "Macintosh HD".into(), kind: "local".into() },
        SystemRoot { path: dirs::home_dir().unwrap().display().to_string(), label: "Home".into(), kind: "local".into() },
    ]
}
```

---

## 7. File Watching

The `notify` crate handles this transparently:
- **Linux**: inotify
- **macOS**: FSEvents (via `kqueue` or `fsevent`)
- **Windows**: ReadDirectoryChangesW

Error messages should be generic. The current inotify-specific error handling in `lib.rs` should check for platform-generic "too many files" errors.

---

## 8. Build Pipeline

| Target | Build method | Output |
|--------|-------------|--------|
| Windows | `just build-windows` (native via cmd.exe) | NSIS installer |
| macOS | `cargo tauri build` on Mac hardware | .dmg |
| Linux | `cargo tauri build` | AppImage + .deb |

macOS builds require a real Mac (or Mac VM). Cross-compilation is not reliable for Tauri apps due to framework linking, code signing, and notarization requirements.

### macOS-specific build needs:
- Xcode Command Line Tools (for `cc`, frameworks)
- Universal binary: `--target aarch64-apple-darwin` + `--target x86_64-apple-darwin` + `lipo`
- Code signing: Developer ID certificate (optional for beta, required for distribution)
- Notarization: Apple notarization service (required for gatekeeper bypass)

---

## 9. Migration Plan

### Phase 1: Extract LinuxProbe (M02-M06)
Move existing `/proc` code into `platform/linux.rs` without changing behavior.
1. Create `platform/` module with `types.rs` and `linux.rs`
2. Extract `process_cwd()` and `process_tty()` from `process.rs`
3. Extract `process_rchar()` from `proc_io.rs`
4. Extract `has_established_443()` from `proc_io.rs`
5. Genericize inotify error messages in `lib.rs`

### Phase 2: Implement DarwinProbe (M07-M09)
Write macOS equivalents using `libproc` and `lsof`.
1. Implement `process_cwd()` via libproc
2. Implement `process_rchar()` via proc_pid_rusage
3. Implement `has_established_443()` via lsof or libproc

### Phase 3: macOS Integration (M10-M11)
1. Add `cfg(target_os = "macos")` blocks to daemon launcher
2. Add Terminal.app / iTerm2 integration

### Phase 4: Build & Test (M12-M18)
1. macOS bundle config (icon, .dmg settings)
2. Set up remote Mac build environment
3. Build, run, test on macOS
4. Universal binary (arm64 + x86_64)

---

## 10. Dependencies

### Existing (already in Cargo.toml)
- `notify` — cross-platform file watching
- `dirs` — cross-platform home directory

### New (macOS only)
- `libproc` — macOS process inspection (`#[cfg(target_os = "macos")]`)
  - Crate: `libproc = "0.14"` (well-maintained, thin wrapper over Apple APIs)

### Build-time only
- `lipo` — universal binary creation (macOS command line tool)
- Xcode CLT — C compiler + framework headers

---

## 11. Implementation Checklist

- [ ] Create `src-tauri/src/platform/mod.rs` with cfg dispatch
- [ ] Create `src-tauri/src/platform/types.rs` with shared types
- [ ] Create `src-tauri/src/platform/linux.rs` — extract from process.rs + proc_io.rs
- [ ] Update `session_scanner/process.rs` to call `platform::process_cwd()` etc.
- [ ] Update `session_scanner/proc_io.rs` to call `platform::process_rchar()` etc.
- [ ] Genericize inotify error messages in `lib.rs`
- [ ] Create `src-tauri/src/platform/darwin.rs` — macOS implementations
- [ ] Add `cfg(target_os = "macos")` blocks to `daemon/launcher.rs`
- [ ] Add `cfg(target_os = "macos")` to `terminal.rs`
- [ ] Add macOS roots to `commands/projects.rs`
- [ ] Add macOS daemon install path to `commands/daemon.rs`
- [ ] Configure macOS bundle in `tauri.conf.json`
- [ ] Test on real macOS hardware
