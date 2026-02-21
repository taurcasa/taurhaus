# Sidebar Session Indicator — Status Report

## Current State

The session indicator system is functional but fails the core UX requirement: **at a glance, detect whether a Claude Code session needs my attention (idle) or is working (active).**

The current implementation repurposed the existing 14px jump-to-terminal icon with an opacity pulse animation. The icon is too small and too subtle — it's barely visible, let alone glanceable. This was a poor design choice that doesn't serve the user's primary use case: glancing at the sidebar while working in Claude Code to spot sessions that need interaction.

## What Works (Infrastructure)

The data pipeline is solid and should not need changes:

- **Session polling**: `sessionStore.svelte.js` polls every 500ms, keyed by normalized project path
- **WSL path matching**: `\\wsl.localhost\` ↔ `\\wsl$\` normalization works correctly
- **Daemon connectivity**: Sessions flow from WSL daemon → Tauri backend → frontend store
- **Three session states detected**: `active` (Claude working), `idle` (waiting for user input), `null` (no session)
- **Reactivity**: Svelte 5 `$state` in the session store triggers re-renders correctly
- **Row tint**: Subtle `bg-white/[0.03]` background when any session exists (in code, not visually reviewed)
- **Context menu**: Session-aware menu items (stop, restart, launch) work correctly

## What Failed (Visual Design)

Multiple approaches were tried and rejected:

| Approach | What it did | Why it failed |
|----------|-------------|---------------|
| 200-level vs 300-level dot brightness | Brighter dot color when session present | Imperceptible at 7px scale |
| Opacity pulse on dot | Dot breathes between 100% and 30% opacity | Alone, can't distinguish active from idle |
| Opacity dim on dot for idle | Dimmer dot = idle session | Wrong hierarchy — dimmer reads as "less important," opposite of intent |
| Ring around dot (box-shadow) | 2px white ring on dot for idle | Looks like a broken/frozen animation |
| Separate `>_` icon with pulse (current) | Terminal prompt icon appears for sessions, pulses when active | Too small (14px), too subtle, barely visible — fails the "glance" test |

## The Core Problem

The sidebar needs to communicate **two independent dimensions** simultaneously:

1. **Git activity state** (4 levels): active, recent, stale, dormant — how recently was code pushed/changed
2. **Claude Code session state** (3 levels): active (working), idle (needs user input), no session

These are independent axes. A dormant project can have an active Claude session. An active project can have no session. The current 7px dot tries to encode git state via color, which works. But encoding session state on the same dot (or on a tiny nearby icon) doesn't produce enough visual contrast to be glanceable.

## Key UX Requirement

> "With a glance I wanna at least have the chance to see okay, that session is idle now because then I have to interact with it."

This is the **most important** state to communicate. "Session idle" = "I need to switch context and provide input." It must be noticeable peripherally while the user is focused on Claude Code in the terminal. Not flashy, not attention-grabbing, but reliably detectable in peripheral vision.

## Design Constraints

- Dark teal sidebar background (`bg-brand-950`, hex `#0A2E2B`)
- 34px row height, 252px sidebar width
- 7px dot currently used for git activity state (color-coded)
- Compact "dense but calm" aesthetic — no flashy animations
- Must work for colorblind users (don't rely solely on hue)
- Tailwind v4 + CSS only (no JS animation libraries)
- `prefers-reduced-motion` support required
- Sits alongside Claude Code terminal — peripheral visibility matters

## Open Design Questions

1. **How to make session state glanceable**: The idle state especially needs to be detectable in peripheral vision. What visual property (size, brightness, motion, position, shape) communicates this best at sidebar scale?

2. **One element or two**: Should one element encode both dimensions (git + session), or should they be separate elements on the row? Separate elements are clearer but take more horizontal space in an already compact row.

3. **Icon choice for session indicator**: If using a separate element — what icon? The user mentioned using actual model logos (Claude logo, etc.) to indicate which agent is running. This enables future extensibility for other AI agents but raises questions about icon legibility at small sizes and color scheme compatibility.

4. **Hover information**: The user requested a hover information concept for the sidebar — tooltips or inline labels showing session details (state, duration, session ID) on mouseover. This is separate from but related to the indicator design. Not yet designed or attempted.

5. **What "idle" looks like**: Every approach so far made idle look either broken (ring), dim (less important), or invisible (too subtle). What visual treatment reads as "waiting for you" without reading as "broken" or "unimportant"?

## Files Involved

| File | Role |
|------|------|
| `src/Shell.svelte` | Sidebar template, `dotClassFor()`, `hasSession()`, `isSessionActive()`, `rowTintFor()` |
| `src/app.css` | Animation keyframes (`session-indicator-pulse`), `prefers-reduced-motion` |
| `src/lib/sessionStore.svelte.js` | Session polling, path normalization, reactive state |
| `src/lib/sidebar.test.js` | Tests for dot class, session indicator, row tint logic |
| `src/lib/ipc.js` | `listClaudeSessions()` IPC + mock data |
| `src-tauri/src/commands/command_center.rs` | Tauri IPC command, daemon RPC, WSL path conversion |
| `src-tauri/src/session_scanner/` | Session detection (ps + tmux parsing), tmux control |
| `src-tauri/src/daemon/server.rs` | Daemon RPC handler for `list_claude_sessions` |

## What Needs to Happen

1. **Design the visual system** for communicating session state at glanceable scale — this needs proper design thinking, not code iteration
2. **Define the hover information system** for the sidebar — what shows on mouseover, how it's styled, what data it includes
3. **Implement the chosen design** with TDD, verify with automated DOM inspection (Playwright) before asking for visual review
4. **Remove diagnostic logging** from `sessionStore.svelte.js` (line 57 console.warn) once resolved
