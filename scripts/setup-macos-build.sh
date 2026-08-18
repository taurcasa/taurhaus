#!/bin/bash
# ===========================================================================
# Scaleway Mac mini — taurhaus build environment setup
# ===========================================================================
#
# Run on a fresh Scaleway Mac mini M1/M2/M4 to install all prerequisites
# for building and testing taurhaus.
#
# Usage:
#   ssh m1@<IP> 'bash -s' < scripts/setup-macos-build.sh
#   # or:
#   scp scripts/setup-macos-build.sh m1@<IP>:~ && ssh m1@<IP> bash setup-macos-build.sh
#
# Takes ~10-15 minutes on first run.
# ===========================================================================

set -euo pipefail

echo "=== taurhaus macOS build environment setup ==="
echo "Started: $(date)"
echo ""

# -------------------------------------------------------------------
# 1. Xcode Command Line Tools (CLang, system headers, git)
# -------------------------------------------------------------------
echo "--- [1/7] Xcode Command Line Tools ---"
if xcode-select -p &>/dev/null; then
    echo "Already installed at $(xcode-select -p)"
else
    echo "Installing Xcode CLT (this may take a few minutes)..."
    xcode-select --install 2>/dev/null || true
    # Wait for installation to complete
    until xcode-select -p &>/dev/null; do
        echo "  Waiting for Xcode CLT installation..."
        sleep 10
    done
    echo "Xcode CLT installed"
fi
echo ""

# -------------------------------------------------------------------
# 2. Homebrew
# -------------------------------------------------------------------
echo "--- [2/7] Homebrew ---"
if command -v brew &>/dev/null; then
    echo "Already installed: $(brew --version | head -1)"
else
    echo "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Add to path for this session
    eval "$(/opt/homebrew/bin/brew shellenv)"
    echo "Homebrew installed"
fi

# Ensure brew is on PATH for Apple Silicon
if [[ -f /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
    # Persist for future sessions
    if ! grep -q 'brew shellenv' ~/.zprofile 2>/dev/null; then
        echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
    fi
fi
echo ""

# -------------------------------------------------------------------
# 3. Rust toolchain
# -------------------------------------------------------------------
echo "--- [3/7] Rust toolchain ---"
if command -v rustc &>/dev/null; then
    echo "Already installed: $(rustc --version)"
else
    echo "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "Rust installed: $(rustc --version)"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

# Add universal target for M18 (universal binary)
echo "Adding aarch64 + x86_64 targets..."
rustup target add aarch64-apple-darwin 2>/dev/null || true
rustup target add x86_64-apple-darwin 2>/dev/null || true
echo ""

# -------------------------------------------------------------------
# 4. Node.js (via fnm, matches our dev setup)
# -------------------------------------------------------------------
echo "--- [4/7] Node.js ---"
if command -v node &>/dev/null; then
    echo "Already installed: node $(node --version)"
else
    echo "Installing fnm + Node.js LTS..."
    curl -fsSL https://fnm.vercel.app/install | bash
    # Source fnm for this session
    export PATH="$HOME/.local/share/fnm:$PATH"
    eval "$(fnm env)"
    fnm install --lts
    fnm default lts-latest
    echo "Node.js installed: $(node --version)"
fi
# Ensure fnm is sourced
if command -v fnm &>/dev/null; then
    eval "$(fnm env)" 2>/dev/null || true
fi
echo ""

# -------------------------------------------------------------------
# 5. System dependencies (tmux for terminal integration testing)
# -------------------------------------------------------------------
echo "--- [5/7] System dependencies ---"
brew install tmux 2>/dev/null || echo "tmux already installed"
echo "tmux: $(tmux -V)"
echo ""

# -------------------------------------------------------------------
# 6. Tauri CLI
# -------------------------------------------------------------------
echo "--- [6/7] Tauri CLI ---"
if command -v cargo-tauri &>/dev/null || cargo tauri --version &>/dev/null 2>&1; then
    echo "Already installed: $(cargo tauri --version 2>/dev/null || echo 'present')"
else
    echo "Installing Tauri CLI..."
    cargo install tauri-cli
    echo "Tauri CLI installed: $(cargo tauri --version)"
fi
echo ""

# -------------------------------------------------------------------
# 7. AI CLI tools (for M15 process detection testing)
# -------------------------------------------------------------------
echo "--- [7/9] AI CLI tools ---"

# Claude Code
if command -v claude &>/dev/null; then
    echo "Claude Code already installed: $(claude --version 2>/dev/null || echo 'present')"
else
    echo "Installing Claude Code..."
    npm install -g @anthropic-ai/claude-code
    echo "Claude Code installed"
fi

# Codex CLI
if command -v codex &>/dev/null; then
    echo "Codex CLI already installed"
else
    echo "Installing Codex CLI..."
    npm install -g @openai/codex
    echo "Codex CLI installed"
fi

# Gemini CLI
if command -v gemini &>/dev/null; then
    echo "Gemini CLI already installed"
else
    echo "Installing Gemini CLI..."
    npm install -g @google/gemini-cli
    echo "Gemini CLI installed"
fi
echo ""

# -------------------------------------------------------------------
# 8. Clone and verify build
# -------------------------------------------------------------------
echo "--- [8/9] Clone and verify ---"
REPO_DIR="$HOME/projects/taurhaus"

if [[ -d "$REPO_DIR" ]]; then
    echo "Repo already exists at $REPO_DIR, pulling latest..."
    cd "$REPO_DIR"
    git pull
else
    echo "Cloning taurhaus..."
    mkdir -p "$HOME/projects"
    cd "$HOME/projects"
    git clone https://github.com/taurcasa/taurhaus.git
    cd taurhaus
fi

echo ""
echo "Installing Bun dependencies..."
bun install --frozen-lockfile

echo ""
echo "Creating dev resource placeholder..."
mkdir -p src-tauri/resources
touch src-tauri/resources/taurhaus-daemon

echo ""
echo "Running cargo check (first build will take a while)..."
cd src-tauri
cargo check 2>&1 | tail -5

# -------------------------------------------------------------------
# 9. Verification summary
# -------------------------------------------------------------------
echo ""
echo "=== Setup complete! ==="
echo ""
echo "Versions:"
echo "  macOS:   $(sw_vers -productVersion)"
echo "  Arch:    $(uname -m)"
echo "  Xcode:   $(xcode-select -p)"
echo "  Rust:    $(rustc --version)"
echo "  Cargo:   $(cargo --version)"
echo "  Node:    $(node --version)"
echo "  npm:     $(npm --version)"
echo "  tmux:    $(tmux -V)"
echo "  claude:  $(claude --version 2>/dev/null || echo 'installed')"
echo "  codex:   $(codex --version 2>/dev/null || echo 'installed')"
echo "  gemini:  $(gemini --version 2>/dev/null || echo 'installed')"
echo ""
echo "=== MANUAL STEPS REQUIRED ==="
echo ""
echo "1. If repo is private, set up SSH key or PAT for git:"
echo "   ssh-keygen -t ed25519 -C 'scaleway-mac'"
echo "   cat ~/.ssh/id_ed25519.pub  # Add to GitHub deploy keys"
echo ""
echo "2. API keys for CLI tools (M15 process detection testing):"
echo "   Create DEDICATED TEST KEYS with low spending limits."
echo "   Export in this session only — never write to disk:"
echo ""
echo "   export ANTHROPIC_API_KEY=sk-ant-...  # console.anthropic.com"
echo "   export OPENAI_API_KEY=sk-...         # platform.openai.com"
echo "   export GEMINI_API_KEY=...            # aistudio.google.com"
echo ""
echo "   Keys live in shell memory only. Revoke after testing."
echo "   Do NOT add these to ~/.zshrc, ~/.zprofile, or any file."
echo ""
echo "3. Run the test suite (M18):"
echo "   cd ~/projects/taurhaus/src-tauri && cargo test --lib"
echo "   cd ~/projects/taurhaus && bunx vitest run"
echo ""
echo "4. Build the app (M14):"
echo "   cd ~/projects/taurhaus/src-tauri && cargo tauri build"
echo ""
echo "5. Test process detection (M15):"
echo "   tmux new-session -s taurhaus"
echo "   # Pane 1: cd ~/projects/taurhaus && claude"
echo "   # Pane 2: cd ~/projects/taurhaus && codex"
echo "   # Pane 3: cd ~/projects/taurhaus && gemini"
echo "   # Then run the built app and verify detection"
echo ""
echo "6. Cleanup after testing:"
echo "   # Revoke all 3 test API keys in their web consoles"
echo "   # Delete the Scaleway Mac mini when done"
echo ""
echo "Finished: $(date)"
