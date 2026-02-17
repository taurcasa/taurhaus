# taurhaus development recipes

# Project paths
project   := justfile_directory()
win_dir   := "/mnt/d/taurhaus_build"
win_drive := "D:\\taurhaus_build"

# Run frontend dev server only
dev-frontend:
    npm run dev

# Run full Tauri dev (frontend + backend)
dev:
    npm run dev:tauri

# Run all checks (quality gate)
check: lint test
    @echo "All checks passed."

# Lint everything
lint:
    cd src-tauri && cargo clippy --all-targets -- -D warnings
    npm run check

# Run all tests
test: test-rust test-frontend

# Run Rust tests
test-rust:
    cd src-tauri && cargo test

# Run frontend tests
test-frontend:
    npm run test

# Run frontend tests in watch mode
test-watch:
    npm run test:watch

# Run E2E tests (when available)
test-e2e:
    @echo "E2E tests not yet configured"

# Reset database (delete SQLite file)
db-reset:
    @echo "DB reset not yet configured — SQLite module pending"

# Run database migrations
db-migrate:
    @echo "DB migrations not yet configured — SQLite module pending"

# Build for Linux
build-linux:
    npm run tauri build

# ── Windows Build (via WSL2 interop) ─────────────────────────────────────────

# Sync source to Windows build directory
sync-windows:
    @echo "▸ Syncing source to {{win_dir}}…"
    rsync -a --delete \
        --exclude='node_modules' \
        --exclude='target' \
        --exclude='dist' \
        --exclude='.git' \
        {{project}}/ {{win_dir}}/
    @echo "✓ Sync complete"

# Build Windows NSIS installer (syncs first, builds natively on Windows)
build-windows: sync-windows
    @echo "Note: cmd.exe may print 'UNC paths are not supported'. This is harmless."
    @echo "▸ Installing frontend dependencies on Windows…"
    cmd.exe /c "cd /d {{win_drive}} && npm install"
    @echo ""
    @echo "▸ Building Windows NSIS installer (cargo tauri)…"
    cmd.exe /c "cd /d {{win_drive}} && cargo tauri build --bundles nsis"
    @echo ""
    @echo "✓ Windows build complete:"
    @ls -lh {{win_dir}}/src-tauri/target/release/bundle/nsis/*.exe 2>/dev/null || echo "  (no installer found)"

# Take screenshot of current app state
screenshot:
    @echo "Screenshot recipe not yet configured"

# Security audit (integration tasks + phase boundaries)
security-audit:
    cd src-tauri && cargo audit 2>/dev/null || echo "cargo-audit not installed — run: cargo install cargo-audit"
