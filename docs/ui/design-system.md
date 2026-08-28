# Design system

Authoritative reference for taurhaus visual language: design principles, theme tokens, typography, spacing, component patterns, and icon behavior.

## Overview

taurhaus uses a floating-panel shell inside a dark teal frame, with compact spacing tuned for side-panel workflows. Styling is built on Tailwind v4 with custom `@theme` tokens in `src/app.css`, plus shared dark/light class tokens from `src/lib/themeTokens.js`.

This document is for UI contributors implementing or extending components. Use it as the source of truth before adding new visual styles.

## Design philosophy

From `CLAUDE.md`:

- Snappy: no perceptible lag, no layout shifts, minimal blocking states.
- Dense but calm: compact surfaces with deliberate whitespace.
- Floating panels: sidebar and main content are rounded panels inside one frame.
- One dark teal identity: frame and sidebar both use `bg-brand-950`.
- Manila folder tabs: active tab background matches the main content panel.
- Inverse scoop: concave corner where tab pill meets frame.
- Always-visible theme toggle in the titlebar.
- Custom titlebar with draggable non-interactive regions (`data-tauri-drag-region`).

## Theme tokens (`src/app.css`)

### Brand palette

| Token | Hex |
|------|-----|
| `--color-brand-50` | `#F0FDFA` |
| `--color-brand-100` | `#CCFBF1` |
| `--color-brand-200` | `#99F6E4` |
| `--color-brand-400` | `#2DD4BF` |
| `--color-brand-500` | `#14B8A6` |
| `--color-brand-600` | `#0D9488` |
| `--color-brand-700` | `#0F766E` |
| `--color-brand-800` | `#115E59` |
| `--color-brand-900` | `#134E4A` |
| `--color-brand-950` | `#0A2E2B` |

### Status palettes

| Family | Defined shades |
|------|-----------------|
| Success | `50, 300, 400, 500, 600, 700` |
| Warning | `50, 300, 400, 500, 600` |
| Danger | `50, 400, 500, 600` |
| Info | `50, 300, 400, 500, 600` |

Status semantics used in UI:

- Success (green): active/running/connected states (`RUN`, healthy daemon).
- Warning (amber): idle/waiting/reconnecting/dirty states.
- Danger (red): failures, destructive actions, error surfaces.
- Info (blue): secondary informational status (for example project activity classes).

### Typography tokens

| Token | Value |
|------|-------|
| `--font-sans` | `'Geist', ui-sans-serif, -apple-system, system-ui, sans-serif` |
| `--font-mono` | `'Geist Mono', ui-monospace, 'SF Mono', 'JetBrains Mono', 'Cascadia Code', monospace` |

## Dark/light mode model

Dark mode is controlled by a top-level `dark` state in `Shell.svelte` and mirrored to `<html class="dark">` for global CSS behavior (scrollbars). Shared component color decisions should come from `themeTokens(dark)` and be consumed via `$derived`.

`themeTokens` standard groups:

- Text hierarchy (`textPrimary`, `textSecondary`, `textTertiary`, `textMuted`, `textBody`)
- Surfaces (`mainBg`, `cardBg`, `sectionBg`, `listBg`)
- Borders (`keyline`)
- Interaction (`hoverRow`, `listHover`, `listSelected`, `fileBg`)
- Accent/link (`linkColor`, `hashColor`, `questionMark`)
- Forms (`inputBg`, `checkBg`, `labelColor`)

Rule: for reusable UI chrome, use named `$derived` tokens, not inline per-element ternaries. If a color decision is local and one-off, keep it local.

## Layout and spacing standards

Canonical dimensions (from `CLAUDE.md` and `Shell.svelte`):

| Element | Value |
|------|-------|
| Titlebar height | `46px` |
| Sidebar width | `252px` |
| Panel gap | `6px` (`gap-1.5`) |
| Frame padding | `6px` (`p-1.5`) |
| Tab pill height | `36px` |

Shell composition:

- Root frame: `bg-brand-950`.
- Sidebar: `rounded-lg`, dark teal panel (`bg-brand-950`) with subtle white border tint.
- Main panel: rounded panel (`rounded-b-lg rounded-tr-lg`) using `t.mainBg`.

## Core component patterns

### Manila folder tab + inverse scoop

- Active tab pill and main panel share the same background (`t.mainBg`).
- Right edge uses a small square with overflow-hidden and a rounded inner child to create the concave scoop back into `bg-brand-950`.

### Floating panels

- Visual hierarchy is frame -> sidebar/main panels -> internal cards/lists.
- Panels float via rounded corners + subtle borders/shadows, not heavy separators.

### Custom titlebar

- Native OS decorations are disabled.
- Drag is opt-in using `data-tauri-drag-region` on non-interactive titlebar areas.
- Search, theme toggle, and window controls live in the titlebar.
- Settings does **not** live in the titlebar; it is opened from the sidebar footer and replaces the main panel content.

## Icons, logos, and motion

Tool logos are defined in `src/lib/toolLogos.js` as SVG path data:

- Claude, Codex, Antigravity (`agy`), Grok and `unknown` icons with per-icon `viewBox` and `path`, in both a full (`TOOL_ICONS`) and a sidebar-small variant.
- Monochrome `fill="currentColor"` rendering so state color classes drive visual state.
- Shared helper accessors: `getToolIcon(tool)`, `getToolName(tool)`.

Session activity styling:

- `.session-pill-active`: breathing pulse animation (`1.8s`, opacity cycle).
- `.session-pill-idle`: inset + outer amber ring treatment.
- `prefers-reduced-motion: reduce` disables pulse and applies a static emphasis.

Sidebar session patterns:

- single sessions render as `14px` currentColor SVG marks
- mesh-team groups render as either a rail (`<=3` members) or shallow stack (`4+` members)
- the foreground project gets top/bottom brand-color guide lines, separate from the selected-row left accent
- sidebar footer exposes daemon connection state plus settings/manage-project affordances

## Shade gotcha (important)

Custom color namespaces only guarantee shades you define in `@theme`. If you reference a shade class that is not declared, style output may be missing or inconsistent.

Examples to avoid:

- Using `text-warning-700` without `--color-warning-700`
- Using `bg-success-100` without `--color-success-100`

Before introducing a new shade class, add the corresponding `--color-<family>-<shade>` token in `src/app.css`.

## Key files

| File | Purpose |
|------|---------|
| `src/app.css` | `@theme` tokens, global animation, global scrollbar styles |
| `src/lib/themeTokens.js` | Shared dark/light class token map |
| `src/lib/toolLogos.js` | Tool SVG icons and display-name mapping |
| `src/Shell.svelte` | Frame, titlebar, tabs, theme toggle, drag-region behavior |
| `src/lib/Sidebar.svelte` | Sidebar shell, filter, footer, daemon state, settings entry point |
| `src/lib/SidebarProjectList.svelte` | Project rows, grouped tool indicators, foreground-project affordance |
| `src/lib/sessionIndicator.js` | Session badge semantics + active/idle/team grouping classes |

## Related documents

- [Layout and navigation](./layout-and-navigation.md) — shell structure and view transitions
- [CLAUDE.md](../../CLAUDE.md) — design paradigms and layout dimensions
