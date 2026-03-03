# File browser

The file browser lets users navigate project files, view text and markdown with syntax highlighting, preview images, and respond to live filesystem changes.

## Overview

The feature has two tree-style surfaces:

- `FilesTab`: project file tree + content viewer for the selected project.
- `DirectoryBrowser`: folder picker used during project registration.

For file rendering details and pipeline diagrams, see [File rendering pipeline](../file-rendering-pipeline.md).

## Directory tree behavior

### Files tab tree (`FilesTab.svelte`)

- Loads project tree via `get_file_tree(projectId)`.
- Displays a hierarchical tree with directory expand/collapse state in `expandedDirs`.
- Auto-selects a README file on first load when one exists.
- Auto-expands parent directories when navigating to a specific file/line.

Backend tree rules (`src-tauri/src/fs/tree.rs`):

- Uses `ignore::WalkBuilder` with `.gitignore` and `.git/info/exclude` support.
- Excludes `.git` explicitly.
- Does not follow symlinks when walking subdirectories.
- Sorts directories first, then files, case-insensitive.

### Add-project browser tree (`DirectoryBrowser.svelte`)

- Uses `list_directory(path)` lazily per expanded directory.
- Caches loaded children in `treeChildren[path]`.
- Supports navigating up to system roots (`get_system_roots`) and selecting roots/drives.

## File viewer behavior

`FilesTab` classifies a file before deciding whether to call IPC. Rendering paths:

| File class | Frontend action | Backend/API path | UI result |
|---|---|---|---|
| `image` | Try `assetCache.get`, then `read_project_asset` | `commands/files.rs::read_project_asset` | Inline `<img>` preview |
| `markdown` | `read_file` then `MarkdownRenderer` | `commands/files.rs::read_file` | Rendered markdown with highlighted code blocks |
| `text` | `read_file` then `CodeViewer` | `commands/files.rs::read_file` | Syntax highlighted code / plaintext fallback |
| `binary` | No IPC call | N/A | "Binary file — cannot display as text" |
| `pdf` | No IPC call | N/A | "PDF viewer coming soon" |

## File classification

Classification is extension-based (`src/lib/fileClassifier.js`):

- `image`: png/jpg/jpeg/gif/svg/webp/ico/bmp
- `markdown`: md/markdown
- `pdf`: pdf
- `binary`: known non-text sets (archives, databases, media, fonts, compiled artifacts, etc.)
- default: `text`

Why it matters:

- Prevents unnecessary IPC calls for known binary/PDF files.
- Routes markdown to rich rendering and code/text to code viewer.

## Markdown rendering

`MarkdownRenderer.svelte` + `src/lib/markdown.js` pipeline:

1. `renderMarkdown(source, theme)` uses `markdown-it`.
2. `@shikijs/markdown-it` highlights fenced code blocks.
3. `DOMPurify` sanitizes HTML.
4. After render, relative `<img>` sources are resolved through asset cache or `read_project_asset`.
5. Link handling routes external links (`http`, `https`, `mailto`) through `openExternalUrl`, and routes relative links through `onNavigate(path)` for in-app file navigation.

Resilience:

- If Shiki pipeline fails, markdown falls back to plain markdown-it rendering.
- If full render fails, renderer falls back to escaped preformatted text.

## Code viewer and language detection

`CodeViewer.svelte` renders text files and uses Shiki via `highlightCode()`:

- Highlights using selected light/dark code theme.
- Falls back to plaintext if language is unavailable or highlighting fails.
- Supports `scrollToLine` navigation with temporary line highlight.

Backend language detection (`src-tauri/src/fs/reader.rs`):

- Maps common extensions to Shiki IDs (`.rs` -> `rust`, `.py` -> `python`, etc.).
- Passes unknown extensions through as-is.

## Image display and asset cache

Images are loaded as base64 data URIs and cached by project/path.

Cache behavior (`src/lib/assetCache.js`):

- Key: `${projectId}/${relativePath}`
- `get`, `set`, `invalidate`, `invalidateProject`, `clear`

Where cache is used:

- `FilesTab`: direct image file previews.
- `MarkdownRenderer`: embedded markdown images.

Backend constraints:

- `read_project_asset` returns `data:{mime};base64,...`.
- Local provider enforces max asset size of 5 MB.

## Binary and large-file handling

Current behavior for non-displayable files:

- Known binary/PDF extensions are blocked at classification time (no IPC call).
- Text-read failures from backend map to user-visible states: binary decode failure -> `binary`, `>5 MB` text file -> `too-large`, generic read failure -> `error`.

Current UX does not include binary download prompts or hex preview in `FilesTab`; unsupported files show a friendly placeholder message.

## File watching and real-time updates

Watch pipeline:

1. `notify` watcher (`src-tauri/src/fs/watcher.rs`) emits classified events.
2. `.git` internals are debounced (2 seconds) before `GitChanged` events.
3. Regular file events are filtered through per-project `.gitignore` matchers (and reloaded on `.taurhausignore`/`.gitignore` changes).
4. `event_processor.rs` batches watcher events (quiet window `300 ms`, max wait `2 s`).
5. Batched file changes emit one `project-files-changed` event per project.
6. `Shell.svelte` listens for `project-files-changed`, invalidates project image cache for changed image paths, refreshes Overview README when README changed, and sets `fileChangePaths` for `FilesTab`.
7. `FilesTab` consumes `changedPaths`, refreshes tree, and re-reads selected file if it changed.

This keeps file UI responsive while avoiding event storms and repeated expensive work.

## Security and path safety

`src-tauri/src/fs/reader.rs` and provider asset reads enforce:

- reject absolute paths
- reject traversal components (`..`)
- canonicalization checks to prevent symlink escape outside project root
- size limits (text > 5 MB rejected; assets > 5 MB rejected in local provider)

## Key files

| File | Purpose |
|---|---|
| `src/lib/FilesTab.svelte` | Main file tree + content viewer and file-change reaction logic. |
| `src/lib/DirectoryBrowser.svelte` | Lazy-loaded directory picker used in project add flows. |
| `src/lib/CodeViewer.svelte` | Text/code display with Shiki highlighting + line navigation. |
| `src/lib/MarkdownRenderer.svelte` | Markdown rendering, link handling, embedded image resolution. |
| `src/lib/fileClassifier.js` | Extension-based file classification (`image`, `markdown`, `binary`, `pdf`, `text`). |
| `src/lib/assetCache.js` | Project/path-scoped in-memory cache for image data URIs. |
| `src/lib/fileChange.js` | Path matching helpers for `project-files-changed` events. |
| `src-tauri/src/commands/files.rs` | File IPC commands (`get_file_tree`, `read_file`, `get_readme`, `read_project_asset`). |
| `src-tauri/src/fs/tree.rs` | .gitignore-aware file tree traversal and sorting. |
| `src-tauri/src/fs/reader.rs` | Secure text file reading + language detection + size checks. |
| `src-tauri/src/fs/watcher.rs` | Notify watcher, event classification, git debounce, gitignore filtering. |
| `src-tauri/src/event_processor.rs` | Watch-event batching and `project-files-changed` emission to frontend. |

## Related documents

- [File rendering pipeline](../file-rendering-pipeline.md) — detailed classification/rendering/cache pipeline
- [IPC command reference](../architecture/ipc-reference.md) — command signatures and categories
