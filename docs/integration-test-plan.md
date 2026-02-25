# Multi-CLI Integration Test Plan

> Test that taurhaus correctly detects, tracks, and displays Codex and Gemini sessions working on real projects.

## Strategy

Give both Codex and Gemini the **same project spec** but in **separate directories** (separate git repos). While they work, monitor taurhaus's detection of:
- Session appearance/disappearance in sidebar
- Active vs idle state transitions
- Task board entries (Codex `update_plan`, Gemini `TODO.md`)
- Process detection via `/proc`
- HoverCard details
- Context menu actions (stop/restart)

## Test Project: `tapcount`

A tiny Node.js CLI that counts word frequency across text files.

```
tapcount ./docs --top 20 --ignore-common
```

**Why this project:** Small enough for a single session, has 4 clear features that should generate task/plan entries, produces files on disk we can verify.

### Spec (given to both tools)

```markdown
# tapcount

A Node.js CLI tool that counts word frequency across text files in a directory.

## Features

1. **Directory scanning** — Recursively find all `.txt` and `.md` files in a given directory
2. **Word counting** — Parse each file, normalize words (lowercase, strip punctuation), count occurrences
3. **Top-N filtering** — Show the top N most frequent words (default: 10)
4. **Common word exclusion** — `--ignore-common` flag to skip English stop words (the, a, is, etc.)

## CLI Interface

```
tapcount <directory> [options]

Options:
  --top <n>          Show top N words (default: 10)
  --ignore-common    Skip common English stop words
  --format <fmt>     Output format: table (default), json, csv
  --help             Show help
```

## Example Output

```
$ tapcount ./docs --top 5
Word        Count
─────────────────
function      47
component     38
state         31
render        28
props         24
```

## Technical Requirements

- Node.js 20+, no external dependencies (use built-in fs, path, readline)
- Entry point: `bin/tapcount.js` with `#!/usr/bin/env node` shebang
- Main logic in `src/scanner.js`, `src/counter.js`, `src/formatter.js`
- Include a small `test/` directory with sample `.txt` files for manual testing
- Add a `TODO.md` with remaining tasks (this helps us track progress)

## Acceptance Criteria

- [ ] `tapcount ./test` prints a word frequency table
- [ ] `--top 5` limits output to 5 words
- [ ] `--ignore-common` removes stop words from results
- [ ] `--format json` outputs valid JSON
- [ ] `--format csv` outputs valid CSV
- [ ] Handles empty directories gracefully
- [ ] Handles binary files (skips them) gracefully
```

## Setup Steps

### 1. Create two project directories

```bash
mkdir -p ~/projects/tapcount-codex
mkdir -p ~/projects/tapcount-gemini
cd ~/projects/tapcount-codex && git init && npm init -y
cd ~/projects/tapcount-gemini && git init && npm init -y
```

### 2. Register both in taurhaus

Open taurhaus, add both directories as projects.

### 3. Launch sessions

In tmux:
```bash
# Pane 1: Codex
cd ~/projects/tapcount-codex
codex --yolo

# Pane 2: Gemini
cd ~/projects/tapcount-gemini
gemini
```

Paste the spec from above into each tool's prompt.

### 4. Monitor daemon logs

```bash
# Pane 3: Daemon logs
# (tail the daemon output or run with verbose flag)
```

## Monitoring Checklist

### Phase 1: Session Detection (within 30s of launch)

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 1 | Codex process appears in `ps` output | Daemon log shows Codex PID + cwd | |
| 2 | Gemini process appears in `ps` output | Daemon log shows Gemini PID + cwd | |
| 3 | Codex logo appears on tapcount-codex sidebar entry | Green Codex logo visible | |
| 4 | Gemini logo appears on tapcount-gemini sidebar entry | Green Gemini logo visible | |
| 5 | HoverCard shows session details for Codex | Hover over tapcount-codex | |
| 6 | HoverCard shows session details for Gemini | Hover over tapcount-gemini | |

### Phase 2: Active State (while tools are generating code)

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 7 | Codex logo is green (active) while writing | Logo color = success-300 | |
| 8 | Gemini logo is green (active) while writing | Logo color = success-300 | |
| 9 | Logo pulse animation plays during active state | Opacity pulse on active logos | |
| 10 | Session duration timer is counting up | HoverCard shows increasing duration | |

### Phase 3: Idle State (between tool actions / thinking pauses)

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 11 | Codex logo turns amber after JSONL mtime > 5s idle | Logo color = warning-300 | |
| 12 | Gemini logo turns amber after JSON mtime > 5s idle | Logo color = warning-300 | |
| 13 | IO-based activity detection catches "thinking" phases | Codex shows active even when JSONL not written (API call in progress) | |
| 14 | State hysteresis prevents flickering | No rapid green/amber toggles | |

### Phase 4: Task Detection

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 15 | Codex `update_plan` entries appear on task board | Navigate to tapcount-codex → Tasks tab | |
| 16 | Plan steps show correct status (pending/completed) | Check statuses match Codex's plan | |
| 17 | Gemini `TODO.md` entries appear on task board | Navigate to tapcount-gemini → Tasks tab | |
| 18 | Checkbox state (checked/unchecked) is reflected | Compare TODO.md content with board | |
| 19 | Task board refreshes as tools check off items | Watch board update in real time | |

### Phase 5: Session File Artifacts

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 20 | Codex JSONL file exists in `~/.codex/sessions/YYYY/MM/DD/` | `ls ~/.codex/sessions/2026/02/23/` | |
| 21 | First line of JSONL has correct `cwd` for tapcount-codex | `head -1` the JSONL file | |
| 22 | Gemini session JSON exists in `~/.gemini/tmp/{hash}/chats/` | Compute SHA256 of path, check dir | |
| 23 | Codex session ID matches what taurhaus displays | Compare filename UUID with HoverCard | |

### Phase 6: Session Lifecycle (stop and restart)

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 24 | Context menu "Stop" sends correct signal to Codex | Right-click → Stop, check tmux pane | |
| 25 | Context menu "Stop" sends correct signal to Gemini | Right-click → Stop, check tmux pane | |
| 26 | Logo disappears after session ends | Sidebar entry updates | |
| 27 | Session statistics are recorded (duration, active %) | Check session history in taurhaus | |
| 28 | Context menu "Launch Codex" starts new session | Right-click → Launch Codex | |
| 29 | Context menu "Launch Gemini" starts new session | Right-click → Launch Gemini | |
| 30 | Relaunched session is detected correctly | Logo reappears, state tracking works | |

### Phase 7: Edge Cases

| # | What to verify | How to check | Pass? |
|---|----------------|--------------|-------|
| 31 | Both tools on SAME project detected separately | (Future test: run both on one project) | |
| 32 | Codex resume (`codex resume --last`) updates existing session | Resume after idle period | |
| 33 | Path normalization handles trailing slashes | Check cwd matching | |
| 34 | 30s Codex cache doesn't cause stale reads | Verify session appears within reasonable time | |

## Expected File Locations

After sessions run, verify these exist:

```
# Codex
~/.codex/sessions/2026/02/23/rollout-*.jsonl  → cwd matches tapcount-codex

# Gemini (compute hash first)
echo -n "/home/mstie/projects/tapcount-gemini" | sha256sum
~/.gemini/tmp/{hash}/chats/session-*.json
```

## Notes

- Daemon polls every 500ms — allow 1-2 seconds for state changes to propagate
- Codex path cache has 30s TTL — new session might take up to 30s to appear if a previous scan cached "no session"
- Active threshold is 5 seconds — brief pauses between file writes will show as idle (expected)
- IO-based detection reads `/proc/PID/io` — this only works in WSL/Linux, not native Windows
