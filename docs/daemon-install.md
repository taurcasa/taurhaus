# Daemon Auto-Install — Design Doc

## Problem

Today, the WSL daemon must be pre-installed manually via `just install-daemon`, which requires a full Rust dev environment. Beta users won't have that. The Windows installer should handle daemon setup automatically.

## Solution

Bundle the pre-built daemon binary inside the Windows app. The FirstRunWizard detects if the daemon is missing or outdated and offers one-click installation. Normal app startups check for version mismatches and offer updates.

---

## 1. Binary Bundling

### How it works

Tauri's `bundle.resources` config copies files into the app's resource directory at install time. On Windows with NSIS, resources land at:

```
C:\Users\{user}\AppData\Local\com.taurhaus.dev\resources\
```

### Build pipeline change

The `just build-windows` recipe already builds the daemon before the Windows app (`install-daemon` dependency). We add one step: copy the Linux ELF binary into a `resources/` directory that Tauri bundles.

```
justfile flow:
  build-daemon (cargo build --release --bin taurhaus-daemon)
    → copy target/release/taurhaus-daemon → src-tauri/resources/taurhaus-daemon
    → sync to Windows build dir
    → cargo tauri build (NSIS bundles the resources/ directory)
```

### tauri.conf.json change

```json
"bundle": {
  "resources": {
    "resources/taurhaus-daemon": "resources/taurhaus-daemon"
  }
}
```

### Version tracking

The bundled daemon is built from the same source tree as the app. Both share `CARGO_PKG_VERSION` (currently `0.3.2`). The daemon reports its version via the ping protocol (`PingResult.version`). For detection without a running daemon, we add a `--version` flag to the daemon binary that prints the version and exits.

---

## 2. Daemon Detection

### IPC command: `check_daemon_status`

Called by the wizard and on app startup. Returns:

```typescript
{
  installed: boolean,       // daemon binary exists in WSL
  version: string | null,   // installed daemon version (from --version flag)
  bundled_version: string,  // version bundled in this app
  needs_update: boolean,    // installed version < bundled version
  wsl_available: boolean,   // wsl.exe exists and a distro is configured
  error: string | null      // human-readable error if detection failed
}
```

### Detection flow

```
1. Check wsl.exe exists (Command::new("wsl").arg("--status"))
   → If not: return { wsl_available: false, error: "WSL not installed" }

2. Detect default distro (wsl -l -q | head -1)
   → If empty: return { wsl_available: true, error: "No WSL distro configured" }

3. Check binary exists: wsl -d {distro} -- test -f ~/.local/bin/taurhaus-daemon
   → If not: return { installed: false, needs_update: false }

4. Get version: wsl -d {distro} -- ~/.local/bin/taurhaus-daemon --version
   → Parse "taurhaus-daemon X.Y.Z" output
   → Compare with bundled version (semver comparison)
   → Return { installed: true, version, needs_update: installed < bundled }
```

---

## 3. Daemon Installation

### IPC command: `install_daemon`

Copies the bundled daemon binary from app resources into WSL. Returns success or structured error.

### Install flow

```
1. Resolve bundled binary path via app.path().resource_dir() / "taurhaus-daemon"
   → Verify file exists (it should, since we bundled it)

2. Resolve WSL home: wsl -d {distro} -- sh -c "echo $HOME"

3. Create target directory: wsl -d {distro} -- mkdir -p $HOME/.local/bin

4. Copy binary: The bundled binary is on the Windows filesystem (C:\Users\...).
   WSL can access it via /mnt/c/... path translation.

   wsl -d {distro} -- cp /mnt/{drive}/{path}/resources/taurhaus-daemon $HOME/.local/bin/

5. Set permissions: wsl -d {distro} -- chmod +x $HOME/.local/bin/taurhaus-daemon

6. Verify: wsl -d {distro} -- $HOME/.local/bin/taurhaus-daemon --version
   → Confirm version matches bundled version
```

### Path translation

The Windows resource path (e.g., `C:\Users\mstie\AppData\Local\com.taurhaus.dev\resources\taurhaus-daemon`) must be translated to a WSL-accessible path (`/mnt/c/Users/mstie/AppData/Local/com.taurhaus.dev/resources/taurhaus-daemon`).

On Windows, this is straightforward: replace `C:\` with `/mnt/c/` and backslashes with forward slashes. Alternatively, use `wslpath` inside WSL to do the conversion.

---

## 4. FirstRunWizard Step

### UX flow

New **Step 2** (between Welcome and Browse), titled "Setup Helper Service":

```
┌─────────────────────────────────────────────────┐
│  Setup Helper Service                           │
│                                                 │
│  taurhaus uses a helper service in WSL to       │
│  watch your projects and detect AI sessions.    │
│                                                 │
│  [■] Daemon not installed                       │
│                                                 │
│       [Install]    [Skip for now]               │
│                                                 │
│  ─── or ───                                     │
│                                                 │
│  [✓] Daemon v0.3.2 installed                    │
│  (auto-proceeds to next step)                   │
└─────────────────────────────────────────────────┘
```

### States

| State | UI | Action |
|-------|-----|--------|
| Checking... | Spinner + "Checking daemon status..." | Auto — runs `check_daemon_status` |
| Not installed | Warning icon + "Install" button | User clicks Install |
| Installing... | Spinner + "Installing daemon..." | Auto — runs `install_daemon` |
| Installed (current) | Green check + version | Auto-proceed after 800ms |
| Installed (outdated) | Amber warning + "Update" button | User clicks Update |
| Install failed | Error message + manual instructions + "Skip" | User reads instructions or skips |
| No WSL | Error + "WSL is required" + link to MS docs | User must install WSL first |

### Skip behavior

If the user clicks "Skip for now", the app runs in degraded mode:
- Local provider fallback handles project data (slower for WSL filesystem)
- File watching may not work for WSL projects
- Banner reminder shown on main app: "Daemon not configured — some features may be limited"

---

## 5. App Startup Update Detection

On every normal app launch (not first-run wizard):

```
1. After projects load (non-blocking), call check_daemon_status in background
2. If needs_update is true:
   → Show banner: "Daemon update available (v0.3.1 → v0.3.2)"
   → "Update now" button
   → "Dismiss" button (don't show again until next app version)
3. User clicks "Update now":
   → Stop running daemon (via shutdown protocol command or pkill)
   → Install new daemon (install_daemon)
   → Restart daemon (try_connect_daemon with auto-start)
   → Dismiss banner on success
4. If update fails:
   → Banner shows error: "Update failed — app continues with current daemon"
```

The banner reuses the existing `daemonStatus` banner pattern in Shell.svelte.

---

## 6. Error States

### No WSL installed

```
Detection: Command::new("wsl").arg("--status") fails or wsl.exe not found
UI: "WSL 2 is required for taurhaus on Windows.
     Install it from: https://learn.microsoft.com/en-us/windows/wsl/install"
Recovery: User installs WSL, restarts app, wizard detects it
```

### WSL installed but no distro

```
Detection: wsl -l -q returns empty
UI: "WSL is installed but no Linux distribution is configured.
     Install one from Microsoft Store (e.g., Ubuntu)."
Recovery: User installs a distro, restarts app
```

### Permission denied during install

```
Detection: cp or chmod command fails with permission error
UI: "Could not install daemon — permission denied.
     Manual install: Run this in WSL terminal:
     cp /mnt/c/.../taurhaus-daemon ~/.local/bin/ && chmod +x ~/.local/bin/taurhaus-daemon"
Recovery: User runs manual command, clicks "Retry" in wizard
```

### Binary exists but wrong architecture

```
Detection: --version returns error or unexpected output
UI: "Daemon binary exists but may be corrupted. Click Install to replace it."
Recovery: Install overwrites with fresh copy
```

### Disk full

```
Detection: cp fails with "No space left on device"
UI: "Not enough disk space in WSL. Free some space and try again."
Recovery: User frees space, clicks "Retry"
```

---

## 7. Implementation Checklist

- [ ] Add `--version` flag to daemon binary (print `taurhaus-daemon {CARGO_PKG_VERSION}` and exit)
- [ ] Add `resources/` directory to `.gitignore` (built artifact, not source)
- [ ] Update `justfile` to copy daemon binary to `src-tauri/resources/` before Windows build
- [ ] Add `bundle.resources` to `tauri.conf.json`
- [ ] Implement `check_daemon_status` IPC command
- [ ] Implement `install_daemon` IPC command
- [ ] Add new wizard step to `FirstRunWizard.svelte`
- [ ] Add update banner to `Shell.svelte`
- [ ] Add E2E regression test for daemon detection
