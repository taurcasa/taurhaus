# macOS Build Environment — Provisioning Guide

How to set up a fresh Mac Mini (Scaleway or other) for building taurhaus.

## Previous Environment (Feb 2026)

- **Host**: Scaleway Mac Mini M1 (aarch64-apple-darwin)
- **IP**: 62.210.195.235 (user: m1)
- **macOS**: Sequoia
- **VNC**: port 59010 (must stay logged in — logout kills Aqua session)

## Toolchain Versions (last known working)

| Tool | Version |
|------|---------|
| Rust | 1.93.1 (stable-aarch64-apple-darwin) |
| Rust targets | aarch64-apple-darwin, x86_64-apple-darwin |
| Node | 25.6.1 |
| npm | 11.9.0 |
| Homebrew | custom prefix `~/.homebrew` (no sudo on Scaleway) |
| tmux | via homebrew |

## Homebrew Packages

Core packages installed via `brew install`:
- `tmux` — required for session management
- `node` — JavaScript runtime (alternative: install via fnm)

The rest (ada-url, brotli, c-ares, ca-certificates, fmt, icu4c, libevent, libnghttp2, libnghttp3, libngtcp2, libuv, llhttp, lz4, ncurses, openssl@3, readline, simdjson, sqlite, utf8proc, uvwasi, xz, zstd, hdrhistogram_c) are transitive dependencies.

## Provisioning Steps

### 1. SSH access

```bash
ssh m1@<IP>
```

### 2. Homebrew (no-sudo install)

Scaleway Mac Minis don't have admin rights. Install homebrew to home dir:

```bash
git clone https://github.com/Homebrew/brew.git ~/.homebrew
echo 'export PATH="$HOME/.homebrew/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
brew install tmux node
```

### 3. Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
# Add x86_64 target for universal builds
rustup target add x86_64-apple-darwin
```

### 4. Shell PATH

Ensure `~/.zshrc` has:

```bash
export PATH="$HOME/.homebrew/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
```

The `just` recipes use `zsh -ilc` to get the full login shell environment.

### 5. Project directory

```bash
mkdir -p ~/projects/taurhaus
```

Then from the dev machine:

```bash
just sync-macos   # rsyncs source to the Mac
```

### 6. First build

```bash
just build-macos   # handles npm install, daemon build+codesign, cargo tauri build
```

### 7. VNC (for GUI testing)

Scaleway provides VNC access on a custom port. You MUST stay logged in to the macOS session — logging out kills the Aqua session and GUI apps won't launch.

## macOS-Specific Gotchas

### Code signing (Sequoia+)

Cargo's linker-signed adhoc binaries get rejected by macOS Sequoia after `cp`. Always re-sign after copying:

```bash
codesign --force --sign - path/to/binary
```

The `just build-macos` recipe handles this for the daemon binary. The main app binary is signed by Tauri's bundler.

### TCC consent prompts

`list_directory()` used to do N+1 `read_dir` calls that triggered TCC consent popups on protected folders. Fixed: all directories default to `is_expandable = true`.

### oh-my-zsh interactive prompts

If oh-my-zsh is installed, set `mode auto` to prevent interactive prompts in tmux panes spawned headlessly.

## Justfile Recipes

| Recipe | What it does |
|--------|-------------|
| `just sync-macos` | rsync source to Mac (excludes node_modules/target/.git) |
| `just build-macos` | Full build: sync + npm install + daemon + codesign + cargo tauri build |
| `just build-macos-universal` | Universal binary (arm64 + x86_64) |
| `just test-macos` | Run Rust tests on Mac |
| `just test-macos-e2e` | Run E2E test suite on Mac |

## Updating justfile for new host

If the IP changes, update these lines in `justfile`:

```just
mac_host  := "m1@<NEW_IP>"
mac_dir   := "~/projects/taurhaus"
```

## Saved Artifacts

Build artifacts from the last session are saved in `builds/macos-aarch64/`:
- `taurhaus_0.3.2_aarch64.dmg` — installable disk image
- `taurhaus-daemon-aarch64` — daemon binary
