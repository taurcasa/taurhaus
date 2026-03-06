# Glass / Depth Exploration For Main Content Panel

Date: 2026-03-06  
Task: #424

## Short answer

Subtle depth could improve taurhaus. Full glassmorphism probably would not.

Recommendation:

- **Do not** make the main panel meaningfully translucent.
- **Do** add a restrained depth treatment to the main panel edge.
- **Do** add a very low-contrast material gradient/texture to the dark teal frame.

This keeps the shell calm and premium without making a developer tool feel soft, blurry, or decorative.

## Why true glass is the wrong default

The current shell already has the right structural idea:

- dark teal frame
- floating sidebar
- floating main panel
- strong content readability

Real frosted glass usually pays off when there is rich imagery or layered motion behind it. taurhaus does not have that. Behind the main panel is mostly a flat dark teal frame. That means backdrop blur would add:

- softness
- extra compositing
- a “designed effect”

but not much actual information or depth.

For an ultrawide side-panel tool, that is the wrong trade.

## What could work

### 1. Opaque panel with edge depth

Keep the main content panel visually opaque, but give it a slightly more dimensional edge treatment:

- faint top highlight
- subtle inner border
- deeper outer shadow in dark mode
- barely warm/cool gradient so the panel feels like a surface, not a flat fill

This reads as depth, not glass.

### 2. Material frame treatment

The dark teal frame can carry more of the “premium surface” feel than the content panel.

Good direction:

- a soft vertical gradient
- a gentle vignette toward the corners
- optional ultra-subtle noise texture

Bad direction:

- visible grain
- literal leather pattern
- glossy hotspots
- anything the user notices before they notice the content

The frame should feel richer only when the user is not looking for it.

## Recommendation

Use **depth without translucency**.

### Main panel

Apply a restrained “sealed panel” treatment:

- dark mode: nearly opaque charcoal surface with slight top lift
- light mode: warm white with a faint cool edge and slightly stronger shadow than today

Avoid:

- strong backdrop blur
- obvious transparency
- saturated teal glows around the content area
- glassy highlights large enough to compete with text

### Frame

Apply a subtle material field to `bg-brand-950`:

- top slightly lighter than bottom
- corners slightly darker
- optional 1-2% opacity noise layer

This is the better place to add tactility because it is peripheral UI chrome, not reading surface.

## Rough CSS direction

These are design targets, not implementation requirements.

### Dark mode main panel

```css
--panel-main-bg-dark: rgba(9, 14, 17, 0.96);
--panel-main-border-dark: rgba(255, 255, 255, 0.06);
--panel-main-inner-highlight-dark: rgba(255, 255, 255, 0.035);
--panel-main-shadow-dark:
  0 10px 24px rgba(0, 0, 0, 0.24),
  0 2px 6px rgba(0, 0, 0, 0.18);
--panel-main-gradient-dark:
  linear-gradient(180deg, rgba(255,255,255,0.02) 0%, rgba(255,255,255,0) 18%);
```

### Light mode main panel

```css
--panel-main-bg-light: rgba(255, 255, 255, 0.94);
--panel-main-border-light: rgba(19, 78, 74, 0.08);
--panel-main-inner-highlight-light: rgba(255, 255, 255, 0.75);
--panel-main-shadow-light:
  0 10px 24px rgba(15, 23, 42, 0.08),
  0 2px 6px rgba(15, 23, 42, 0.06);
--panel-main-gradient-light:
  linear-gradient(180deg, rgba(240,253,250,0.72) 0%, rgba(255,255,255,0) 16%);
```

### Frame material field

```css
--frame-surface:
  radial-gradient(120% 90% at 50% 0%, rgba(255,255,255,0.035) 0%, transparent 42%),
  radial-gradient(140% 120% at 50% 100%, rgba(0,0,0,0.16) 0%, transparent 48%),
  linear-gradient(180deg, #0c3532 0%, #0a2e2b 48%, #082725 100%);
```

Optional noise layer:

```css
opacity: 0.015 to 0.025
background-size: 160px 160px
mix-blend-mode: soft-light
```

That is enough. More than that will read as decoration.

## What I would not do

### Heavy blur

`backdrop-filter: blur(12px)` or similar is too much here. It pushes the shell toward consumer-app glass instead of operator-tool clarity.

### Strong transparency

If the panel becomes visibly see-through, the content loses authority and the frame starts leaking into the reading surface.

### Bright teal rim-light

The brand teal should remain an accent, not a glowing perimeter around the main content.

## Before / after sketch

Current:

```text
┌ teal frame ───────────────────────────────────────────────┐
│ ┌ sidebar ┐  ┌ main panel ──────────────────────────────┐ │
│ │         │  │ flat fill, clean, readable              │ │
│ │         │  │                                         │ │
│ └─────────┘  └─────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

Recommended:

```text
┌ material teal frame, soft gradient + faint corner falloff ┐
│ ┌ sidebar ┐  ┌ main panel ───────────────────────────────┐ │
│ │         │  │ opaque reading surface                    │ │
│ │         │  │ + faint top inner highlight               │ │
│ │         │  │ + subtle outer shadow                     │ │
│ └─────────┘  └───────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

Rejected direction:

```text
main panel = translucent blur card floating over visible teal field
result = softer, flashier, less precise
```

## Verdict

Subtle depth is worth exploring.

But the winning version is not “glass.” It is:

- a slightly more dimensional opaque main panel
- a richer but still quiet teal frame surface

That adds polish without making taurhaus feel noisy, glossy, or less scannable.
