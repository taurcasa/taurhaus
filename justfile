# taurhaus development recipes

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

# Build for Windows (cross-compile from Linux)
build-windows:
    npm run tauri build -- --target x86_64-pc-windows-msvc

# Sync build to Windows machine
sync-windows:
    @echo "Windows sync not yet configured"

# Take screenshot of current app state
screenshot:
    @echo "Screenshot recipe not yet configured"

# Security audit (integration tasks + phase boundaries)
security-audit:
    cd src-tauri && cargo audit 2>/dev/null || echo "cargo-audit not installed — run: cargo install cargo-audit"
