# Splash / Boot Screen Design

## Context

**taurhaus** = Taurus + Haus (German for house). The "taur" prefix comes from the family surname **Stier** (German for bull/Taurus) — it's a shared brand prefix across projects (taursult, taurui, taurhaus, etc.). The name means "bull's house" — a strong, grounded home for AI-driven CLI-tool-based development. The coincidence with the Tauri framework name is just that — a coincidence.

This dual meaning (bull + house) opens a powerful logo opportunity: a mark that merges both concepts, e.g. bull horns forming the roofline of a house shape.

**Existing identity elements:**
- Brand color: dark teal (`brand-950: #0A2E2B`) — app frame and sidebar
- Font: Geist (sans) + Geist Mono
- Layout: "Floating Panel" — dark teal frame wraps sidebar + main content panels

**Current startup behavior (no splash):**
1. App window opens → dark teal frame visible immediately
2. `checkFirstRun()` fires (IPC call to backend)
3. Main layout renders with sidebar loading skeleton
4. Bootstrap chain runs in Rust background: daemon → tmux → watchers

**What's missing:**
- No visual feedback during bootstrap (daemon startup can take 2-5s on cold WSL start)
- Brief flash of empty main layout before data populates
- No branding moment — the app just "appears" with loading skeletons
- Daemon status banner appears late and feels disconnected from startup

---

## Design: "Building the House"

### Concept

The logo is a minimal geometric house — clean architectural lines like a blueprint mark. During the splash, **the house builds itself**: strokes draw in sequence, each section tied to a real bootstrap step. The animation IS the progress indicator. When the house is complete, the app is ready.

This is not a decorative delay — every frame of animation corresponds to real work. But because the splash is non-interactive and brief, we lean into visual beauty. The technical app gets a moment of pure polish.

### The Logo

**Style:** Geometric line-art house. Not literal (no windows, no chimney) — more like an architect's mark or a structural diagram. The *idea* of a house expressed in minimal strokes.

**Visual concept:**
```
        /\
       /  \          ← roof: two angled lines meeting at apex
      /    \
     /      \
    |--------|       ← lintel: horizontal line connecting walls to roof
    |        |       ← walls: two vertical lines
    |   __   |       ← door: small centered opening at base
    |__|  |__|
```

Refined to be more abstract/geometric — likely just 5-7 SVG path segments:
1. **Foundation/base line** — horizontal ground line
2. **Left wall** — vertical line rising from base
3. **Right wall** — vertical line rising from base
4. **Left roof slope** — angled line from wall top to apex
5. **Right roof slope** — angled line from apex to wall top
6. **Optional: door or center detail** — small accent that "completes" the home

**Colors:**
- Primary strokes: `brand-400` (#2DD4BF) or `brand-500` (#14B8A6) — bright teal on dark teal background
- Could use a subtle gradient from `brand-600` → `brand-400` as strokes draw (darker at base, brighter at roof)
- Completed house: full brightness. In-progress strokes: slightly dimmed or drawing with a glow trail

**Multi-size requirements:**
- 80-100px on splash (large, animated)
- 22px in titlebar (static, must read as a simple shape)
- 16px Windows taskbar icon (ICO — needs to be recognizable at tiny size)
- 256px Windows installer
- 1024px future macOS icon

### The Build Animation (Implemented)

The logo is a raster image (PNG with transparency) revealed progressively using CSS `clip-path: inset()`. The image reveals from **bottom to top** — the house builds upward from its foundation.

| Animation Phase | Bootstrap Step | Visual |
|----------------|---------------|--------|
| 1. Foundation | Daemon starting (auto at 150ms) | Bottom 32% reveals — feet/base of the pillars |
| 2. Walls rise | Connecting (auto at 500ms) | Bottom 68% reveals — walls and shoulders rise |
| 3. Crown | Daemon connected | Full 100% — keystone and horns crown the top |

**Why clip-path instead of SVG stroke animation:**
- The AI-generated logo has a specific quality that hand-traced SVG couldn't match
- CSS `clip-path` transitions are GPU-accelerated — butter-smooth
- Single image asset, no quality loss, the "building upward" metaphor works perfectly
- Faithful to the original logo at all sizes

**Timing:**
- Phase 1→2 auto-advances on timer (the animation stays ahead of bootstrap)
- Phase 3 waits for real `daemon-status: connected` event from Rust
- Each phase transition: 500ms ease-out CSS transition on clip-path
- Minimum total duration: **800ms** (prevents flash)
- Maximum idle: **15s** before showing error state

**The "done" moment:**
When the roof apex is reached, a brief pulse or glow (100ms) acknowledges completion. The completed house holds for **400ms** — a satisfying beat of "built."

### Layout

```
┌──────────────────────────────────────────────────────────┐
│                                                          │
│                    bg-brand-950                           │
│                                                          │
│                                                          │
│                    ╱ ╲                                    │
│                   ╱   ╲         ← logo: house building   │
│                  |     |           80-100px               │
│                  |__ __|                                  │
│                                                          │
│                  taurhaus                                 │  Geist 18px, white/90
│                                                          │
│             Starting daemon...                           │  Geist 12px, white/30
│                                                          │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

**Elements:**
1. **Animated house logo** — centered, large (80-100px), the star of the show
2. **Wordmark** — "taurhaus" below logo, Geist 18px, `text-white/90`, static (appears immediately with a subtle fade-in)
3. **Status text** — small and very muted (`12px, text-white/30`), below wordmark. Updates per step:
   - "Starting daemon..." → "Connecting..." → "Ready"
   - The text is secondary information — the visual building IS the primary progress
4. **No progress bar** — the building animation replaces it entirely

### Transition to App

When the house is complete:

1. **400ms hold** — admire the completed house
2. **Logo shrinks + moves** to titlebar position (top-left, 22px) over 300ms ease-in-out
3. **Simultaneously**: sidebar and main panels fade in + slide up slightly from below
4. **Teal background stays** — it IS the app frame, so there's no screen replacement
5. **Net effect**: the house "lands" in its permanent home (the titlebar), and the app panels emerge around it

This transition reinforces the metaphor: the house was built, now you're stepping inside.

**Reduced motion alternative:** Logo cross-fades from splash size to titlebar position (no movement). Panels appear with opacity fade only (no slide).

### Error State

If daemon fails to connect (15s timeout):

```
        ╱ ╲
       ╱   ╲
      |     |          ← house stays at whatever phase it reached
      |__ __|             (partial build = visual "something went wrong")

      taurhaus

  Could not start daemon          ← text-danger-400, 13px

  [  Retry  ]   [ Continue anyway ]    ← buttons appear
```

The partial house is itself a visual indicator that something is incomplete. "Continue anyway" enters degraded mode (no sessions, no file watching, read-only project data from SQLite cache).

### Design Principles Applied

**TaurUI `foundations/motion-feedback.md`:**
- "Splash screens that play for 3 seconds" is an anti-pattern for *decorative* delays. Ours is tied to real progress — the animation IS the progress indicator, not a timer.
- Motion serves Purpose 2 (Feedback: action confirmation — bootstrap is progressing) and Purpose 4 (Delight — used sparingly, on a low-frequency interaction that only happens once per launch).

**TaurUI `patterns/feedback/loading-states.md`:**
- "Show what you have, load what you don't." — We show brand identity while loading daemon/projects.
- Minimum display time (800ms) prevents flash.
- "If the operation has distinct phases, show the current phase" — each house section = a phase.

**TaurUI `patterns/visualization/status-progress.md`:**
- Step indicators for linear, few-step processes (our 3-step bootstrap).
- "Completed steps are visually distinct from upcoming" — drawn strokes vs. not-yet-drawn.

### Reference Apps

1. **JetBrains IDEs** — Splash with logo + progress bar + step labels ("Loading plugins...", "Indexing..."). Same concept but with a traditional progress bar. Our building animation replaces the bar with something more memorable.

2. **Tower (Git client)** — Brief branded splash while connecting to repositories. Similar backend-connection flow.

3. **Linear** — Known for polished micro-interactions and transitions. Their app loading uses subtle motion that feels premium without being slow.

4. **Raycast** — Keyboard launcher that feels instant. Their startup is near-zero, but the polish in every micro-interaction is the gold standard for developer tools.

---

## Logo Specification (Implemented)

### The Logo

Generated using Gemini 3 Pro image model from the "Horned Keystone" concept (Gemini 3.1 Pro text model).

**Source**: `docs/logo-candidates/candidate-01-keystone-gemini.jpg`

**Design**: Three interlocking teal shapes on dark teal background:
1. **Left pillar** — L-shaped block (wall + foot), representing the house's left wall
2. **Right pillar** — Mirror of left, representing the house's right wall
3. **Keystone** — Bull's head with curved horns, sitting between the pillars like an architectural keystone

The dual reading is what makes it special: simultaneously a house (pillars = walls, upper angles = roof) and a bull (keystone = head, pillars = shoulders/body). The negative space between the three pieces is essential.

### Formats Generated

| Format | Size | Location | Use |
|--------|------|----------|-----|
| PNG | 1024x1024 | `docs/logo-candidates/logo-1024.png` | Future macOS app icon |
| PNG | 256x256 | `src-tauri/icons/128x128@2x.png` | Windows installer, HiDPI |
| PNG | 200x200 | `public/logo-200.png` | Splash screen logo |
| PNG | 128x128 | `src-tauri/icons/128x128.png` | General use |
| PNG | 48x48 | `docs/logo-candidates/logo-48.png` | ICO source |
| PNG | 32x32 | `src-tauri/icons/32x32.png` | Windows exe icon |
| PNG | 22x22 | `public/logo-22.png` | Titlebar logo |
| PNG | 16x16 | `docs/logo-candidates/logo-16.png` | ICO source |
| ICO | 16/32/48/256 | `src-tauri/icons/icon.ico` | Windows exe and taskbar |

All PNGs have **transparency** (alpha channel). Background removed via color-distance thresholding with anti-aliased edges.

---

## Implementation Checklist

### Splash Screen (`src/lib/SplashScreen.svelte`)
- [x] Splash appears before any IPC calls (gates Shell in App.svelte)
- [x] Bootstrap progress via clip-path reveal (3 phases tied to daemon status)
- [x] Minimum 800ms total display to prevent flash
- [x] Status text updates per bootstrap step ("Starting daemon..." → "Connecting..." → "Ready")
- [x] Error state with retry + "continue anyway" buttons
- [x] Timeout at 15s if daemon never connects
- [x] Partial reveal = visual "incomplete" in error state
- [x] Seamless transition to main layout (crossfade, same bg-brand-950)
- [ ] Logo shrinks/moves to titlebar position on completion (future polish)
- [x] Titlebar drag region works during splash (`data-tauri-drag-region`)
- [x] `prefers-reduced-motion`: disables clip-path and opacity transitions
- [x] Accessible: `aria-live="polite"`, `role="status"` semantics

### Logo
- [x] Raster PNG with transparency (clip-path animated, not SVG stroke)
- [x] Reads clearly at 16px
- [x] All format/size variants generated (16, 22, 32, 48, 128, 200, 256, 1024)
- [x] ICO with bundled sizes (16/32/48/256)
- [x] Alpha channel on all PNGs
- [x] Replace placeholder "t" square in titlebar
- [x] Replace placeholder Tauri icon in Windows exe/installer

### Integration
- [x] Splash shows before Shell renders (App.svelte gates with `shellReady` flag)
- [ ] Daemon status banner suppressed during splash (only for reconnection events after splash)
- [x] `daemon-status` Tauri events drive animation phase 3 (connected → complete)

---

## Further Reading

- TaurUI `patterns/feedback/loading-states.md` — loading state timing thresholds
- TaurUI `patterns/visualization/status-progress.md` — step indicators and progress
- TaurUI `foundations/motion-feedback.md` — animation timing, easing, reduced motion
- TaurUI `lookbook/developer-tools.md` — reference developer desktop tools
- SVG `stroke-dasharray` animation technique — the core of the build animation
