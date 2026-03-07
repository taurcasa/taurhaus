# Screenshot shot list

Date: 2026-03-07
Task: #597
Input: [readme-content-plan.md](/home/mstie/projects/taurhaus/docs/readme-content-plan.md)

## Guidance

- Dark mode is the primary presentation mode.
- Prefer real product screenshots over generated composites.
- Use clean but believable data; avoid empty states unless the screenshot is explicitly about onboarding.
- Use one coherent demo workspace across shots where possible.

## Shot 1: Hero overview

- **View name**: Main shell hero
- **What should be visible**: Full taurhaus window with populated sidebar, at least 6 projects across activity groups, live session indicators on 2-3 projects, Overview tab open for one active project, visible README card, recent commits, sessions block, and relationship block
- **Crop spec**: Full window
- **Mode**: Dark mode
- **Resolution**: 1600x1000
- **Purpose**: Establish the product as a dense, real desktop workspace that supervises many AI-assisted projects at once

## Shot 2: Sidebar + live session supervision

- **View name**: Sidebar session activity state
- **What should be visible**: Left sidebar with mixed Active/Recent/Stale groups, at least one project with grouped team indicators, at least one standalone session indicator, visible branch/dirty state, and a HoverCard preview open on hover
- **Crop spec**: Focused crop on sidebar plus hover card
- **Mode**: Dark mode
- **Resolution**: 1200x900
- **Purpose**: Demonstrate that taurhaus is a live supervision tool, not just a static project list

## Shot 3: Task board and historical work context

- **View name**: Task board active/history state
- **What should be visible**: Tasks tab with populated In Progress / Pending / Completed columns, mixed Claude/Codex/Gemini tasks, one selected task with detail panel showing dependencies and enriched context
- **Crop spec**: Main panel crop
- **Mode**: Dark mode
- **Resolution**: 1400x900
- **Purpose**: Show that taurhaus aggregates ongoing work and not just repository metadata

## Shot 4: Search overlay across projects

- **View name**: Cross-project search
- **What should be visible**: Search overlay open with a non-trivial query, grouped results for documents, sessions, and commits from multiple projects, clear keyboard-oriented overlay layout
- **Crop spec**: Centered overlay with enough background shell visible to orient the reader
- **Mode**: Dark mode
- **Resolution**: 1200x800
- **Purpose**: Prove the “recover context fast” story across projects and content types

## Shot 5: Mesh setup with templates/customization

- **View name**: Mesh setup and team composition
- **What should be visible**: Mesh tab in setup state, role/preset-driven roster or customizer flow, visible agent cards with mixed tools/models/projects, availability gate already passed, clear “this is how you assemble a team” composition state
- **Crop spec**: Main panel crop
- **Mode**: Dark mode
- **Resolution**: 1400x900
- **Purpose**: Show that taurhaus supports deliberate team composition, not just manual pane spawning

## Shot 6: Mesh runtime canvas

- **View name**: Mesh runtime with active team
- **What should be visible**: Mesh runtime canvas with lead + multiple agents, mixed statuses (active/idle/offline if visually useful), runtime controls, and a node detail card or runtime bar visible
- **Crop spec**: Full main panel
- **Mode**: Dark mode
- **Resolution**: 1600x1000
- **Purpose**: Demonstrate taurhaus’s strongest differentiator: visible multi-agent coordination and operational control

## Shot 7: Mesh recovery / resume state

- **View name**: Resume Team or degraded recovery affordance
- **What should be visible**: Mesh state that clearly shows cold-restart or offline recovery UI, such as a Resume Team banner or offline member recovery affordance
- **Crop spec**: Main panel crop
- **Mode**: Dark mode
- **Resolution**: 1400x900
- **Purpose**: Show that taurhaus handles real-world recovery and not just ideal-path launches

## Shot 8: Files or Git context inspection

- **View name**: Code/commit inspection
- **What should be visible**: Either Files tab with syntax-highlighted code preview and file tree, or Git tab with commit list plus diff view, depending on which reads more clearly in README layout
- **Crop spec**: Main panel crop
- **Mode**: Dark mode
- **Resolution**: 1400x900
- **Purpose**: Reinforce that taurhaus gives immediate project context without leaving the app

## Optional Shot 9: First-run or settings confidence shot

- **View name**: Onboarding or settings
- **What should be visible**: Either the first-run wizard project discovery flow or Settings with scan directories / thresholds / terminal configuration visible
- **Crop spec**: Main panel crop
- **Mode**: Light mode if contrast is better, otherwise dark
- **Resolution**: 1200x850
- **Purpose**: Only use if the README needs stronger setup confidence or contributor-facing install clarity

## Generated asset recommendation

Generated assets are optional, not required.

Recommended only if we want a stronger header treatment:

- **Asset name**: README banner accent
- **Spec**: Wide abstract banner in taurhaus dark-teal visual language, no invented fake UI, subtle references to panels/session flow/agent topology
- **Placement**: Top of README above or below the hero screenshot, but never instead of a real screenshot
- **Resolution**: 1600x640
- **Purpose**: Add polish without weakening product credibility

Recommendation:

- use real screenshots as the primary proof
- only ask `asset-generator` for a supporting banner if the final README layout feels too visually abrupt
