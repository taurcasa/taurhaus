# Phase 3F: Visual System

> The shared visual vocabulary for taurhaus. Every color, text style, spacing value, and component in this document is a named token. View specs reference tokens, not raw values. Built from the [View Designs](phase-3e-views.md) and validated against the [Information Architecture](phase-3d-architecture.md).

---

## 1. Color Semantics

### 1a: Color Needs Inventory

From the view specs, taurhaus needs color for:

- **Activity states** (4 states): Active, Recent, Stale, Dormant — derived from git activity, the primary scanning signal in V-01 and V-02.
- **Working tree status** (2 states): Clean, Dirty — binary signal on every project.
- **Interactive states**: Default, hover, focus, active, selected, disabled — standard for all interactive elements.
- **Feedback**: Success (handoff imported), error (load failure, path not found), warning (stale index, parse error), info (instructional empty states).
- **Actions**: Primary (register, save), secondary (cancel, back), destructive (remove project, delete relationship).
- **Surfaces**: Backgrounds, cards, sidebar, overlays, borders, dividers.
- **Text**: Multiple contrast levels for hierarchy (primary, secondary, tertiary, disabled).

### 1b: Semantic Color Map

| Category | Meaning | Usage |
|----------|---------|-------|
| **Brand** | taurhaus identity, primary actions, selected state | Primary buttons, active tab indicator, selected sidebar item, links |
| **Neutral** | Structure, text, borders, surfaces | Body text, dividers, backgrounds, cards, disabled state |
| **Success** | Positive outcome, active/healthy state | Activity state "Active", handoff imported confirmation, clean working tree |
| **Warning** | Caution, attention needed, aging | Activity state "Stale", stale index indicator, parse warnings |
| **Danger** | Error, destructive action | Remove project button, load errors, path-not-found errors |
| **Info** | Informational, neutral highlight | Activity state "Recent", instructional empty states, tips |
| **Muted** | De-emphasized, inactive | Activity state "Dormant", disabled controls, tertiary text |

### 1c: Color Scales

**Brand — Teal**
A muted teal distinguishes taurhaus from Claude Code's palette. Grounded, productive, not flashy.

| Token | Value | Usage |
|-------|-------|-------|
| brand-50 | `#F0FDFA` | Selected row background, active section tint |
| brand-100 | `#CCFBF1` | Hover background on brand elements |
| brand-200 | `#99F6E4` | Border accent on active/selected items |
| brand-500 | `#14B8A6` | Standard — links, icons, active indicators |
| brand-600 | `#0D9488` | Button fill, active tab indicator |
| brand-700 | `#0F766E` | Button hover state |
| brand-900 | `#134E4A` | Dark text on brand background |

**Neutral — Zinc-based**
Warm gray that avoids blue or purple tints. Works for long-duration use.

| Token | Value | Usage |
|-------|-------|-------|
| neutral-0 | `#FFFFFF` | Pure white — card backgrounds, content area |
| neutral-50 | `#FAFAFA` | App background, sidebar background |
| neutral-100 | `#F4F4F5` | Hover backgrounds, subtle fills |
| neutral-200 | `#E4E4E7` | Borders, dividers |
| neutral-300 | `#D4D4D8` | Disabled borders, subtle accents |
| neutral-400 | `#A1A1AA` | Placeholder text, disabled text |
| neutral-500 | `#71717A` | Tertiary text, icons |
| neutral-600 | `#52525B` | Secondary text |
| neutral-700 | `#3F3F46` | Primary text (secondary weight) |
| neutral-800 | `#27272A` | Primary text (headings) |
| neutral-900 | `#18181B` | Strongest text |
| neutral-950 | `#09090B` | Near-black for high emphasis |

**Success — Green**

| Token | Value | Usage |
|-------|-------|-------|
| success-50 | `#F0FDF4` | Success background tint |
| success-500 | `#22C55E` | Active state dot, success icons |
| success-600 | `#16A34A` | Success text, clean status indicator |
| success-700 | `#15803D` | Success text on light background |

**Warning — Amber**

| Token | Value | Usage |
|-------|-------|-------|
| warning-50 | `#FFFBEB` | Warning background tint |
| warning-500 | `#F59E0B` | Stale state indicator, warning icons |
| warning-600 | `#D97706` | Warning text |
| warning-700 | `#B45309` | Warning text on light background |

**Danger — Red**

| Token | Value | Usage |
|-------|-------|-------|
| danger-50 | `#FEF2F2` | Error background tint |
| danger-500 | `#EF4444` | Error icons, destructive button background |
| danger-600 | `#DC2626` | Error text, destructive button fill |
| danger-700 | `#B91C1C` | Destructive button hover |

**Info — Blue**

| Token | Value | Usage |
|-------|-------|-------|
| info-50 | `#EFF6FF` | Info background tint |
| info-500 | `#3B82F6` | Recent state indicator, info icons |
| info-600 | `#2563EB` | Info text, informational elements |

### 1d: Activity State Color Mapping

| State | Color token | Icon/shape | Redundant cue |
|-------|-----------|------------|---------------|
| Active | `success-500` | ● filled circle | Group header "Active" |
| Recent | `info-500` | ○ ring circle | Group header "Recent" |
| Stale | `warning-500` | ◌ dashed circle | Group header "Stale" |
| Dormant | `neutral-400` | ◌ faded circle | Group header "Dormant" |

Color alone never carries the state — each has a distinct icon shape and is grouped under a labeled header. Accessible without color perception.

### 1e: Working Tree Status

| Status | Color token | Icon | Label |
|--------|-----------|------|-------|
| Clean | `neutral-400` (no emphasis) | — (no indicator) | No visual — clean is the default/expected state |
| Dirty | `warning-500` | ◐ half-filled dot or "M" badge | Tooltip: "Uncommitted changes" |

Dirty is the exception that needs attention. Clean is default — no indicator reduces noise.

### 1f: Contrast Verification

Key text-on-background combinations:

| Text | Background | Ratio | WCAG AA? |
|------|-----------|:-----:|:--------:|
| neutral-900 on neutral-0 | `#18181B` / `#FFFFFF` | 17.1:1 | Yes |
| neutral-700 on neutral-0 | `#3F3F46` / `#FFFFFF` | 10.4:1 | Yes |
| neutral-600 on neutral-0 | `#52525B` / `#FFFFFF` | 7.2:1 | Yes |
| neutral-500 on neutral-0 | `#71717A` / `#FFFFFF` | 4.7:1 | Yes |
| neutral-400 on neutral-0 | `#A1A1AA` / `#FFFFFF` | 3.0:1 | No (use only for large text or non-essential decoration) |
| brand-600 on neutral-0 | `#0D9488` / `#FFFFFF` | 4.6:1 | Yes (AA for normal text) |
| brand-900 on brand-50 | `#134E4A` / `#F0FDFA` | 9.7:1 | Yes |
| danger-600 on neutral-0 | `#DC2626` / `#FFFFFF` | 4.6:1 | Yes |
| neutral-0 on brand-600 | `#FFFFFF` / `#0D9488` | 4.6:1 | Yes |

**Note**: `neutral-400` fails AA for normal text. Reserved for placeholder text, disabled text, and decorative elements only. All informational text uses `neutral-500` or darker.

### 1g: Dark Mode Considerations

v1 ships with light mode. Dark mode support is architecturally prepared:
- All colors are referenced by semantic token, not raw value.
- Dark mode inverts the neutral scale (dark backgrounds, light text).
- Brand, success, warning, danger, info colors may need desaturation for dark backgrounds.
- Implementation: CSS custom properties mapped to tokens. Dark mode overrides the property values.

---

## 2. Typography Scale

### 2a: Font Families

| Family | Font | Fallback | Usage |
|--------|------|----------|-------|
| **Sans** | Geist Sans | `-apple-system, system-ui, sans-serif` | All UI text — navigation, labels, body, headings |
| **Mono** | Geist Mono | `'SF Mono', 'JetBrains Mono', monospace` | Commit hashes, file paths, code snippets, session IDs |

Geist Sans is specified in the design brief for consistency with MIR. Geist Mono for code/data provides visual pairing.

### 2b: Type Scale

| Token | Size | Weight | Line Height | Letter Spacing | Usage |
|-------|------|--------|-------------|----------------|-------|
| `text-heading-1` | 20px | 600 | 1.3 | -0.01em | View titles (project name in V-02 header) |
| `text-heading-2` | 16px | 600 | 1.4 | 0 | Section headers (Latest Session, Recent Activity, Relationships) |
| `text-heading-3` | 14px | 600 | 1.4 | 0 | Group headers (activity state groups in V-01), card titles |
| `text-body` | 14px | 400 | 1.5 | 0 | Primary content — session summaries, descriptions, doc content |
| `text-body-medium` | 14px | 500 | 1.5 | 0 | Emphasized body — next steps, open questions, project name in sidebar |
| `text-small` | 13px | 400 | 1.5 | 0 | Secondary content — branch names, dates, commit messages |
| `text-caption` | 12px | 400 | 1.4 | 0.01em | Timestamps, file metadata, helper text, tertiary info |
| `text-label` | 11px | 600 | 1.0 | 0.05em | Uppercase labels — section overlines, badge text, tab labels |
| `text-mono` | 13px | 400 | 1.5 | 0 | Commit hashes, file paths, code, session IDs |
| `text-mono-small` | 12px | 400 | 1.4 | 0 | Inline code, abbreviated hashes in commit entries |

### 2c: Scale-to-Hierarchy Mapping

| Hierarchy level | Tokens used | View examples |
|----------------|-------------|---------------|
| **Primary** (seen in 1-2s) | `text-heading-1`, `text-body-medium`, `text-heading-2` | V-02: project name (`heading-1`), session summary (`body-medium`), section headers (`heading-2`) |
| **Secondary** (seen on focus) | `text-body`, `text-small`, `text-heading-3` | V-02: commit messages (`text-small`), relationship entries (`text-body`), group headers (`heading-3`) |
| **Tertiary** (on interaction) | `text-caption`, `text-mono-small` | V-02: timestamps (`caption`), commit hashes (`mono-small`), file metadata (`caption`) |

### 2d: Rendered Content Typography

For V-03 (Document Viewer), rendered markdown uses a separate scale optimized for reading:

| Element | Size | Weight | Line Height | Notes |
|---------|------|--------|-------------|-------|
| H1 | 24px | 700 | 1.25 | Rare in docs, but supported |
| H2 | 20px | 600 | 1.3 | Primary section dividers |
| H3 | 16px | 600 | 1.4 | Sub-sections |
| Body | 15px | 400 | 1.6 | Reading-optimized. Slightly larger and more spacious than UI body. |
| Code inline | 14px mono | 400 | — | Background tint, slight padding |
| Code block | 14px mono | 400 | 1.5 | Full-width block with background |
| Blockquote | 15px | 400 italic | 1.6 | Left border accent |

Line length constrained to max 720px (`max-width: 48rem`) for readability (≈60-75 characters at 15px).

---

## 3. Spacing System

### 3a: Base Unit

**4px base** with 8px-centric scale. Fine details use 2px and 4px. Content spacing uses multiples of 8.

### 3b: Spacing Scale

| Token | Value | Usage |
|-------|-------|-------|
| `space-0` | 0px | Flush elements |
| `space-0.5` | 2px | Fine adjustments — icon badge offset, tight inline gaps |
| `space-1` | 4px | Icon-to-label gap, inline element pairs |
| `space-2` | 8px | Intra-group — items within a card, list item padding vertical |
| `space-3` | 12px | List item padding horizontal, form field gap |
| `space-4` | 16px | Card padding, inter-group spacing |
| `space-5` | 20px | Between form sections |
| `space-6` | 24px | Section gap within a view |
| `space-8` | 32px | Major section separation |
| `space-10` | 40px | View-level padding |
| `space-12` | 48px | Large region gaps |

### 3c: Application Principles

**Intra-group** (related items): `space-1` to `space-2` (4-8px)
- List item: text elements spaced by `space-1`
- Card: items within a card spaced by `space-2`

**Inter-group** (between groups): `space-4` to `space-6` (16-24px)
- Between session card and git activity section: `space-6`
- Between git activity and relationships: `space-6`

**Section-level** (major divisions): `space-8` to `space-10` (32-40px)
- Between view header and content: `space-8`
- Page-level margins: `space-10`

**Ratio**: Intra-group to inter-group is at least 1:3 (e.g., 4px within vs. 16px between). This ensures proximity communicates grouping per Gestalt principles.

### 3d: Fixed Dimensions

| Element | Width | Height | Source |
|---------|-------|--------|--------|
| Sidebar | 240px (1280px viewport) / 280px (2560px) | Full height | V-01 layout |
| File tree panel | 200px | Full main area height | V-03 layout |
| Search overlay | 600px | Dynamic (max 60% viewport) | V-04 layout |
| Registration modal | 480px | Dynamic | Modal spec |
| Tab bar | Full main area width | 40px | V-02/V-03 |
| Project header (V-02) | Full main area width | ~72px | V-02 layout |

---

## 4. Component Vocabulary

### C-01: Button

**Variants:**

| Variant | Background | Text | Border | Usage |
|---------|-----------|------|--------|-------|
| Primary | `brand-600` | `neutral-0` | none | Register project, Save, primary actions |
| Secondary | `neutral-0` | `neutral-700` | `neutral-200` | Cancel, Back, secondary actions |
| Ghost | transparent | `neutral-600` | none | Inline actions, "View all", "Add notes" |
| Destructive | `danger-600` | `neutral-0` | none | Remove project (in confirmation dialog) |
| Icon-only | transparent | `neutral-500` | none | Settings icon, Add (+), Close (×) |

**States** (all variants):
- Default → Hover (darken bg or add bg tint) → Active (press, darken further) → Focus (focus ring `brand-500` 2px offset) → Disabled (opacity 0.5, no pointer events)

**Sizes:**

| Size | Height | Padding H | Font | Usage |
|------|--------|-----------|------|-------|
| Small | 28px | `space-2` (8px) | `text-caption` 12px | Inline actions, tag badges |
| Medium | 36px | `space-3` (12px) | `text-body` 14px | Standard buttons |
| Large | 44px | `space-4` (16px) | `text-body` 14px | Primary CTA in first-run |

### C-02: Sidebar Item (Project List Item)

**Anatomy:**
```
┌──────────────────────────┐
│ ● Project Name     main  │  40px height
│                          │  padding: space-2 vertical, space-3 horizontal
└──────────────────────────┘
```

- Left: Activity state dot (8px circle), `space-2` gap
- Center: Project name (`text-body-medium`), truncated with ellipsis
- Right: Branch name (`text-mono-small`, `neutral-500`), truncated
- Dirty indicator: dot fill changes to half-filled or distinct icon

**States:**
- Default: `neutral-50` bg (matches sidebar)
- Hover: `neutral-100` bg
- Selected: `brand-50` bg, `brand-600` left border (3px), name text `neutral-900`
- Focus (keyboard): Focus ring `brand-500`, 2px offset inset

### C-03: Group Header

**Anatomy:**
```
── Active ────────────────  24px height
```
- `text-label` uppercase, `neutral-500` color
- Right-aligned: collapse/expand chevron (`neutral-400`)
- Bottom divider: `neutral-200`, 1px
- Top spacing: `space-4`, bottom spacing: `space-1`

### C-04: Session Card

**Variants:**

**Current (expanded):**
```
┌──────────────────────────────────────────────┐
│  LATEST SESSION                   2026-02-16 │  header: text-label + text-caption
│                                              │
│  Summary text here in body weight, spanning  │  text-body
│  multiple lines as needed for the content.   │
│                                              │
│  Next Steps:                                 │  text-heading-3
│  • Step one description                      │  text-body
│  • Step two description                      │
│                                              │
│  Open Questions:                             │  text-heading-3
│  • Question one                              │  text-body
│                                              │
│  [Add notes]              [View full session]│  Ghost buttons
└──────────────────────────────────────────────┘
```
- Background: `neutral-0` (white card on `neutral-50` background)
- Border: `neutral-200` 1px, radius 8px
- Padding: `space-4`
- Shadow: subtle `0 1px 3px rgba(0,0,0,0.08)` — card elevation

**Historical (compact):**
```
│ 2026-02-15  Completed Phase 3C user jou...   │  Single row, 40px height
```
- `text-caption` date, `text-small` summary, `neutral-600` text
- Hover: `neutral-100` bg. Click expands to current-variant layout.

### C-05: Commit Entry

**Anatomy:**
```
│ a1b2c3  Add phase-3d-architecture.md    2h  │  36px height
```
- Hash: `text-mono-small`, `neutral-500`
- Message: `text-small`, `neutral-700`, truncated
- Date: `text-caption`, `neutral-500`, right-aligned
- Hover: `neutral-100` bg
- Spacing: `space-1` between items, `space-2` padding vertical

### C-06: File Tree Item

**Anatomy:**
```
│  ▾ docs/                 │  Directory: 32px height
│    phase-3d-arch...      │  File: 32px height
```
- Indentation: `space-4` (16px) per depth level
- Directory: chevron + folder icon + name (`text-small`, `neutral-700`)
- File: type icon + name (`text-small`, `neutral-600`)
- Selected: `brand-50` bg, `brand-600` text
- Hover: `neutral-100` bg

### C-07: Search Result Item

**Anatomy:**
```
┌──────────────────────────────────────────┐
│ 📄 taurhaus › docs/phase-3d-archite...  │  text-body-medium
│ ...entity inventory with journey refs... │  text-small, neutral-600
└──────────────────────────────────────────┘
```
- Height: ~56px (2 lines)
- Type badge: icon (📄 doc, 💬 session, ● commit) + `text-caption` label
- Project: `text-caption`, `neutral-500`
- Path/title: `text-body-medium`, `neutral-800`
- Snippet: `text-small`, `neutral-600`, match highlights in `brand-600` bold
- Hover: `neutral-100` bg
- Keyboard selected: `brand-50` bg

### C-08: Tab Bar

**Anatomy:**
```
┌─────────────┬──────────┐
│ ▸ Overview  │  Files   │
└─────────────┴──────────┘
```
- Tab height: 40px
- Text: `text-label` uppercase, `neutral-500` default, `neutral-900` active
- Active indicator: `brand-600` bottom border (2px)
- Hover: `neutral-600` text
- Background: `neutral-0` (card surface)
- Padding: `space-4` horizontal per tab

### C-09: Section Header

**Anatomy:**
```
LATEST SESSION                      2026-02-16
```
- Text: `text-label` uppercase, `neutral-500`
- Right-aligned metadata: `text-caption`, `neutral-400`
- Bottom spacing: `space-3`
- No border (spacing creates separation)

### C-10: Relationship Entry

**Anatomy:**
```
│ → taurui (provides design to)        [⋮] │  40px height
```
- Arrow: direction indicator (`→` or `←`), `neutral-400`
- Project name: `text-body-medium`, `brand-600` (clickable link)
- Type: `text-small`, `neutral-500`, parenthetical
- Description (optional): `text-caption`, `neutral-500`, below type line. When present, item height increases to 56px.
- Actions: kebab menu icon, revealed on hover
- Hover: `neutral-100` bg

### C-11: Empty State

**Anatomy:**
```
┌──────────────────────────────┐
│                              │
│      [Icon or illustration]  │
│                              │
│      Message text here       │  text-body, neutral-500
│      explaining what's empty │
│                              │
│      [Primary action]        │  Button/primary or Ghost
│                              │
└──────────────────────────────┘
```
- Centered in available space
- Icon: `neutral-300`, 48px
- Message: `text-body`, `neutral-500`, max 2-3 lines
- Action: context-appropriate button

### C-12: Badge

**Variants:**

| Variant | Background | Text | Usage |
|---------|-----------|------|-------|
| Active | `success-50` | `success-700` | Activity state label |
| Recent | `info-50` | `info-600` | Activity state label |
| Stale | `warning-50` | `warning-700` | Activity state label |
| Dormant | `neutral-100` | `neutral-500` | Activity state label |
| Tag | `neutral-100` | `neutral-600` | Project type tags |
| Type | `brand-50` | `brand-700` | Search result type badge |

- Height: 20px
- Padding: `space-1` vertical, `space-2` horizontal
- Font: `text-caption`
- Border-radius: 4px

### C-13: Input

**Variants:**

| Variant | Usage |
|---------|-------|
| Text | Form fields (path, name, description) |
| Search | Sidebar filter, search overlay input |

**Anatomy:**
- Height: 36px (medium)
- Padding: `space-2` vertical, `space-3` horizontal
- Font: `text-body`
- Border: `neutral-200` 1px, radius 6px
- Placeholder: `neutral-400`

**States:**
- Default → Focus (`brand-500` border, ring) → Error (`danger-500` border) → Disabled (bg `neutral-100`, text `neutral-400`)

### C-14: Modal Overlay

**Anatomy:**
- Backdrop: `neutral-950` at 50% opacity
- Card: `neutral-0`, radius 12px, shadow `0 4px 24px rgba(0,0,0,0.15)`
- Header: `text-heading-2`, padding `space-4`
- Content: padding `space-4`
- Footer: padding `space-4`, border-top `neutral-200`
- Max width: specified per modal (480px for registration)

### C-15: Tooltip

- Background: `neutral-800`
- Text: `neutral-0`, `text-caption`
- Padding: `space-1` vertical, `space-2` horizontal
- Radius: 4px
- Delay: 500ms before showing
- Arrow pointing to trigger element

### C-16: Skeleton Loader

- Background: `neutral-100` with shimmer animation
- Shapes: rectangle for text lines, circle for avatars/icons, rectangle for cards
- Matches target component dimensions
- Shimmer: left-to-right gradient sweep, 1.5s duration, infinite

### C-17: Confirmation Dialog

- Uses C-14 (Modal Overlay) as container
- Content: Warning icon (`warning-500`) + descriptive text
- Actions: Destructive button (primary, danger variant) + Cancel (secondary)
- Focus: Cancel button receives initial focus (prevents accidental confirmation)

### C-18: Progress Bar

- Height: 8px
- Track: `neutral-200`
- Fill: `brand-500`
- Radius: 4px
- Text above: `text-small`, "X / Y projects"

---

## 5. Information Density

### 5a: Target Density

**Compact.** taurhaus shares screen real estate with Claude Code on an ultrawide monitor. The primary viewport is a 1280px side panel. Density must maximize information per viewport while maintaining readability.

| Factor | taurhaus assessment | Implication |
|--------|-------------------|-------------|
| User expertise | Power user, daily use | Compact appropriate |
| Task type | Scanning (J-01), triage, lookup (J-03) | Dense list, readable content |
| Data volume | 30-50 projects, 10-20 commits, file trees | Must scan efficiently |
| Session length | Hours, always open | Avoid fatigue — compact but not cramped |
| Primary device | Desktop ultrawide, 1280px panel | Compact appropriate |

### 5b: Density Parameters

| Parameter | Value | Notes |
|-----------|-------|-------|
| Sidebar item height | 40px | Fits ~30 items visible at 1440px height minus header/footer (~1340px usable ÷ 40px ≈ 33 items) |
| Commit entry height | 36px | Compact single-line entries |
| File tree item height | 32px | Dense tree navigation |
| Session history item height | 40px | Date + summary excerpt |
| Card padding | 16px (`space-4`) | Comfortable reading within compact layout |
| Section gap | 24px (`space-6`) | Clear separation without waste |
| Body font size | 14px | Legible on desktop at normal viewing distance |
| Mono font size | 13px | Slightly smaller for data/code — scanned, not read in bulk |

### 5c: Density Validation Against Journeys

**J-01 (Orient — scan 30-50 projects):** At 40px per item with ~33 visible, the user can see the entire typical project list (30-50) in one scroll or less. The top ~30 items are visible without scrolling in most cases. **Pass.**

**J-02 (Resume — read session + scan commits):** The latest session card takes ~200-300px. Recent commits (10 entries × 36px = 360px) fit below. At 1440px viewport minus header (72px) and tabs (40px), ~1328px is available for content. Session + commits + some relationships fit without scrolling past the fold for typical content. **Pass.**

**J-03 (Reference docs — find and read files):** File tree at 32px per item shows ~40 visible items. Content area at ~800px width with 15px font provides good readability. **Pass.**

---

## 6. Motion System

### 6a: Duration Tokens

| Token | Duration | Easing | Usage |
|-------|----------|--------|-------|
| `motion-instant` | 0ms | — | Selection highlight, focus ring |
| `motion-fast` | 100ms | `ease-out` | Hover effects, button state changes |
| `motion-normal` | 150ms | `ease-in-out` | Tab switch, content crossfade, list item expand |
| `motion-slow` | 250ms | `ease-in-out` | Modal appear/dismiss, overlay backdrop |
| `motion-deliberate` | 400ms | `ease-in-out` | First-run transitions, progress bar fill |

**Guideline**: Faster than typical recommendations. taurhaus is a companion tool used alongside fast terminal interactions. Sluggish transitions feel wrong in this context.

### 6b: Interaction Assignments

| Interaction | Token | Notes |
|-------------|-------|-------|
| Sidebar item selection | `motion-instant` | Immediate highlight — no delay |
| Tab switch (Overview ↔ Files) | `motion-normal` | Brief crossfade of content |
| Session card expand/collapse | `motion-normal` | Smooth height change |
| Commit entry expand | `motion-normal` | Smooth height change |
| File tree expand/collapse | `motion-fast` | Quick, frequent action |
| Search overlay open | `motion-slow` | Backdrop fade + palette appear |
| Search overlay dismiss | `motion-fast` | Fast exit — user wants to return to work |
| Search results appear | `motion-instant` | Results should feel instantaneous |
| Modal open | `motion-slow` | Backdrop fade + modal scale |
| Modal close | `motion-fast` | Fast exit |
| New session auto-detected | `motion-normal` | Highlight animation on new card |
| Skeleton shimmer | 1.5s continuous | Continuous while loading |
| Tooltip appear | 500ms delay, then `motion-fast` | Prevent tooltip flash on incidental hover |
| Group collapse/expand | `motion-normal` | Smooth height change in sidebar |

### 6c: Reduced Motion Policy

When `prefers-reduced-motion: reduce` is active:
- All duration tokens become `0ms` (instant state changes)
- Opacity transitions remain at `motion-fast` (100ms) — minimal, non-disorienting
- Skeleton shimmer becomes a static `neutral-100` background (no animation)
- Expand/collapse becomes instant height change
- Modal appears instantly without scale animation (opacity-only transition allowed)
- All functionality preserved — only motion is removed

---

## 7. Border & Radius Tokens

| Token | Value | Usage |
|-------|-------|-------|
| `radius-sm` | 4px | Badges, tooltips, small elements |
| `radius-md` | 6px | Inputs, buttons |
| `radius-lg` | 8px | Cards, session card |
| `radius-xl` | 12px | Modals, search overlay |
| `border-default` | 1px solid `neutral-200` | Cards, inputs, dividers |
| `border-active` | 2px solid `brand-600` | Active tab indicator |
| `border-focus` | 2px solid `brand-500` | Focus rings (2px offset) |
| `border-error` | 1px solid `danger-500` | Error state inputs |

---

## 8. Shadow Tokens

| Token | Value | Usage |
|-------|-------|-------|
| `shadow-sm` | `0 1px 2px rgba(0,0,0,0.05)` | Subtle card elevation |
| `shadow-md` | `0 1px 3px rgba(0,0,0,0.08)` | Session card, elevated cards |
| `shadow-lg` | `0 4px 24px rgba(0,0,0,0.15)` | Modals, search overlay |
| `shadow-none` | none | Flat elements, sidebar |

---

## Handoff to Phase 3G

This document provides the implementation vocabulary for Specification:

- **Complete color system** with semantic categories, scales, and contrast verification → token values for every element
- **Typography scale** (10 tokens) with hierarchy mapping → font assignments for every text element
- **Spacing system** (12 tokens) with application principles → padding/margin values for every layout region
- **18 components** (C-01 through C-18) with variants, states, and token assignments → implementation-ready component specs
- **Density parameters** validated against journeys → row heights, font sizes, spacing for compact layout
- **Motion system** (5 duration tokens) with interaction assignments → transition values for every animation
- **Border, radius, and shadow tokens** → visual polish tokens

**Phase 3G will**: Take each view spec from 3E, apply the visual system tokens from this document, and produce implementation-ready specifications with concrete values for every element, every state, and every interaction.
