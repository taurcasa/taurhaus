# Command Center — UI Design

> Design decisions for the command center UI, grounded in TaurUI knowledge base research and aligned with the existing taurhaus visual system.

**Companion doc**: [`command-center.md`](command-center.md) — functional requirements and detection strategy.

---

## Design Context

### Existing Sidebar Structure (`Shell.svelte:717-743`)

```
[dot] project-name    main [dirty]
 7px   13px text      10px   5px
       truncate       mono
```

- Project items: 34px tall buttons, 252px sidebar width
- Activity dots: 7px, `success-300` / `info-300` / `warning-300` / `zinc-400`
- Selected: `bg-white/[0.08]` + 2px `brand-400` accent bar
- Groups: ACTIVE / RECENT / STALE / DORMANT (10px uppercase, `text-white/20`)
- Dirty indicator: 5px `warning-400` dot at trailing edge
- Context menu: existing `ContextMenu.svelte` component with keyboard nav, viewport-aware positioning, two-click destructive confirmation
- Dark teal sidebar `bg-brand-950` in both light and dark mode

### Available Tokens (`app.css`)

| Semantic | 300 (bright, dark bg) | 400 (standard) | 500 (saturated) |
|----------|----------------------|-----------------|------------------|
| Success (green) | `success-300` | `success-400` | `success-500` |
| Warning (amber) | `warning-300` | `warning-400` | `warning-500` |
| Danger (red) | `danger-400` | `danger-500` | `danger-600` |
| Info (blue) | `info-300` | `info-400` | `info-500` |

All colors already defined — no new tokens needed.

---

## 1. Session Status Indicators (Sidebar)

### Pattern: Dot Override with Pulse

When a Claude Code session is detected for a project, the existing 7px activity dot is **overridden** with a session-state-aware dot. Session state is more actionable than activity state — if Claude is running, the user cares about *that*.

| Session State | Dot Visual | Color | Rationale |
|---------------|-----------|-------|-----------|
| **Active** (Claude working) | 7px dot + CSS pulse glow ring (2s cycle) | `success-300` + `success-400/40` ring | Green = running/healthy. Pulse draws the eye to genuinely actionable items. |
| **Idle** (waiting for input) | 7px dot, static, bright | `warning-300` | Amber = needs attention. This IS the core use case: "I see idle, I click to jump." |
| **Inactive** (no session) | Existing activity dot | `dotColor` map unchanged | No override — activity state shows normally. |

### Why Not a Second Dot

At 34px item height and 252px sidebar width, two dots would crowd the layout. The activity dot serves as the single status position — it shows the most immediately actionable state.

### Pulse Animation

- **Type**: Radial glow ring expanding from the dot, fading out
- **Duration**: 2s cycle (`animation: pulse-session 2s ease-in-out infinite`)
- **Intensity**: Subtle — `box-shadow: 0 0 0 Npx rgba(success-400, 0.4)` expanding from 0 to 6px
- **Not a blink**: Smooth radial expansion, not on/off flashing
- **"Don't cry wolf"** (TaurUI `attention-signals.md`): Active sessions genuinely warrant animation — this is the primary signal the user is looking for

### State Priority

When session ends → dot reverts to activity-state color immediately (no transition delay). Session state always overrides activity state when present.

### Accessibility

- Green (active) vs amber (idle) differ in both hue AND brightness — distinguishable for deuteranopia
- Pulse animation respects `prefers-reduced-motion`: static dot with brighter glow instead of animation

---

## 2. Jump-to-Session Inline Icon

### Pattern: Trailing Action Icon

A small terminal/navigate icon appears on project rows that have an active or idle session. This gives the core "jump to session" action a **one-click path** without opening the context menu.

```
[●] taurhaus    main [↗] [•]
 dot  name     branch jump dirty
      13px      10px  12px  5px
```

| Property | Value |
|----------|-------|
| Icon | `↗` or small terminal glyph, 12px |
| Visibility | Only when session is active or idle |
| Color | `text-white/30` default, `text-white/60` on hover |
| Position | Between branch label and dirty dot |
| Click | Triggers `navigate_to_session` IPC |
| Transition | `opacity-0` → `opacity-100` (no layout shift) |

### Why Inline

Per TaurUI `contextual-menu.md`: "If there are only 1-2 actions per item, show them as inline icon buttons." The jump-to-session is THE primary action for session-active projects. One click beats right-click → menu → select.

### Click Behavior

- **Project click** (existing): Selects the project in taurhaus (unchanged)
- **Icon click**: Navigates to the terminal session (stopPropagation — doesn't select)
- **No mode confusion**: The icon is a distinct click target, not an overload of the row click

---

## 3. Context Menu — Session Actions

### Pattern: Dynamic Menu Extension

Extend the existing context menu (`ContextMenu.svelte`) with session-related actions. Menu content changes based on session state but structure stays stable.

### Menu Layout — No Active Session

```
┌───────────────────────┐
│  Continue Session      │  ← claude --dangerously-skip-permissions --continue
│  New Session           │  ← claude --dangerously-skip-permissions
│  Resume (pick)...      │  ← claude --dangerously-skip-permissions --resume
│  ───────────────────── │
│  Copy Path             │  (existing)
│  ───────────────────── │
│  Remove from taurhaus  │  (existing, destructive)
└───────────────────────┘
```

### Menu Layout — Active/Idle Session

```
┌───────────────────────┐
│  Open in Terminal   ↗  │  ← Jump to tmux pane + focus Windows Terminal
│  ───────────────────── │
│  Continue Session      │  (disabled — session exists)
│  New Session           │  (disabled — session exists)
│  Resume (pick)...      │  (disabled — session exists)
│  ───────────────────── │
│  Copy Path             │  (existing)
│  ───────────────────── │
│  Restart Session       │  ← Stop + re-launch
│  Stop Session       ⚠  │  ← Destructive, warning color, two-click confirm
└───────────────────────┘
```

### Design Rules

- **"Open in Terminal"**: First item when session exists. Highest-frequency action gets top position (TaurUI `contextual-menu.md`).
- **Disabled launch items**: Shown disabled (not hidden) when a session exists — teaches the user these exist (TaurUI `interaction-states.md`). Tooltip: "Session already active."
- **"Stop Session"**: Last position, `text-danger-400`, uses existing two-click confirmation pattern from "Remove from taurhaus."
- **Separators**: Session actions → general actions → destructive actions. Three groups, stable structure.
- **Keyboard hint**: Show shortcut next to "Open in Terminal" if keyboard shortcut is defined.

---

## 4. Session Launch UX

### Default Action

The primary "Continue Session" action runs:
```
claude --dangerously-skip-permissions --continue
```

This is the user's default workflow — continue from last checkpoint.

### Bootstrap Feedback

When launching a session, taurhaus may need to:
1. Start Windows Terminal (if not running)
2. Start tmux (if not running)
3. Create tmux window + run claude

If any step fails, show a **transient inline error** on the project row (red flash, 3s duration). Don't open a modal — the action was lightweight, the error should be too.

### Success Feedback

On successful launch:
- The session polling (500ms) picks up the new process within ~1 second
- The dot switches to pulsing green
- Windows Terminal comes to foreground
- No additional UI feedback needed — the terminal appearing IS the feedback

---

## 5. Navigation ("Jump to Session") UX

### Action Chain

1. Daemon: `tmux select-window -t {window}` + `tmux select-pane -t {pane}`
2. Tauri: Focus Windows Terminal window (Win32 `SetForegroundWindow` or `wt.exe`)
3. Result: User lands in the exact terminal pane where Claude is running

### Failure Handling

If navigation fails (tmux session gone, terminal closed):
- Show transient inline error on the project row
- Next poll cycle will detect the session is gone and revert the dot
- No modal, no blocking UI

---

## 6. Visual Specifications

### Pulse Animation CSS

```css
@keyframes pulse-session {
  0%, 100% { box-shadow: 0 0 0 0 rgba(var(--color-success-400), 0.4); }
  50% { box-shadow: 0 0 0 6px rgba(var(--color-success-400), 0); }
}

.session-active-dot {
  animation: pulse-session 2s ease-in-out infinite;
}

@media (prefers-reduced-motion: reduce) {
  .session-active-dot {
    animation: none;
    box-shadow: 0 0 4px 2px rgba(var(--color-success-400), 0.3);
  }
}
```

### Jump Icon

- SVG: Arrow up-right (`↗`) or terminal icon, 12×12px viewBox
- `stroke-width: 1.5`, `stroke: currentColor`
- Positioned with `absolute` inside the row button, right-aligned before dirty dot
- `pointer-events: auto` with `stopPropagation` on click

### Z-Index

- Context menu: existing z-index (should be above sidebar content)
- Pulse glow: no z-index needed (uses box-shadow, doesn't overlap neighbors at 34px row height with 6px max glow)

---

## Reference Apps

| App | What to Study |
|-----|---------------|
| **Railway** | Status dots on dark sidebar, green/yellow/red for service state. Very similar to our use case. |
| **VS Code Activity Bar** | Filled/outline icon toggle for active panel — indicator serves double duty. |
| **Tower (Git)** | Branch indicators + ahead/behind badges inline on sidebar items. |
| **Warp Terminal** | Block-based grouping with active block accents. Active vs historical distinction. |

---

## Research Sources (TaurUI Knowledge Base)

| File | What We Used |
|------|-------------|
| `patterns/visualization/attention-signals.md` | Signal hierarchy, dot badges, "don't cry wolf" principle |
| `patterns/visualization/status-progress.md` | Status badge patterns, color semantics |
| `foundations/color-meaning.md` | Semantic color system, dark theme contrast |
| `patterns/interaction/contextual-menu.md` | Dynamic menus, action grouping, destructive action placement |
| `foundations/interaction-states.md` | Disabled states (show don't hide), state combinations |
| `patterns/navigation/view-switching.md` | Badge counts on navigation items |
| `lookbook/developer-tools.md` | Railway, VS Code, Tower, Warp references |
