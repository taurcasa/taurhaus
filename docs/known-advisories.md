# Known Cargo Audit Advisories

Last reviewed: 2026-02-25

## Accepted Risks

### RUSTSEC-2026-0002 — `lru` 0.12.5 unsound `IterMut`

- **Severity:** Warning (unsound)
- **Dependency chain:** tantivy 0.22.1 → lru 0.12.5
- **Issue:** `IterMut` violates Stacked Borrows by invalidating an internal pointer
- **Impact on taurhaus:** Low. We don't use `lru::LruCache::iter_mut()` directly. The affected code path is internal to tantivy's index writer, which we only interact with through tantivy's high-level API (search and indexing). No user-controlled data reaches the unsound `IterMut`.
- **Fix available:** tantivy 0.25.0 is available but requires a major API migration (0.22 → 0.25). This is out of scope for a refactoring pass.
- **Action:** Accept risk. Revisit when tantivy is next updated for features.

### RUSTSEC-2026-0008 — `git2` 0.19.0 unsound `Buf` deref

- **Severity:** Warning (unsound)
- **Dependency chain:** direct dependency (git2 0.19.0)
- **Issue:** Potential undefined behavior when dereferencing Buf struct
- **Impact on taurhaus:** Low. We use git2 for read-only git operations (log, diff, blame). The `Buf` struct is used internally by libgit2 for buffer management.
- **Action:** Accept risk. Monitor for git2 patch release.

### GTK3 / glib / fxhash / instant / proc-macro-error / unic — unmaintained

- **Severity:** Warning (unmaintained)
- **Dependency chain:** All transitive via Tauri 2.x (wry, tao, webkit2gtk)
- **Impact on taurhaus:** None actionable. These are Tauri framework dependencies for the Linux WebView backend. We don't use GTK3 APIs directly.
- **Action:** No action needed. Will be resolved when Tauri 3.x migrates to GTK4.
