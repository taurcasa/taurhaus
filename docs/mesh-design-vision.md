# Design Vision: Mesh/Team/Template/Role UI

This document outlines the design vision for unifying team management, template browsing, and role configuration within the Mesh tab of taurhaus.

## 1. Current State Analysis: Screenshot Critique

Based on the provided snapshots of the runtime and setup environments:

### Screenshot 225908/225947 (Runtime View)
- **Visual Hierarchy**: The toolbar is too dense and lacks differentiation between branding ("taurhaus-team") and status ("3 Active"). The "+ Add Agent" button is too prominent, competing with the team status.
- **Layout Problems**: Significant dead space on the canvas when fewer than 4 agents are present. The overflow menu (225947) appears disconnected from its trigger point.
- **Interaction Gaps**: It's not immediately clear that node detail popovers are transient or how to pin them.
- **Polish Gaps**: The connection lines between nodes are simple paths; they lack the "active pulse" or glow that indicates live communication.

### Screenshot 225928 (Add Agent Flow)
- **Interaction Gaps**: The form in the slide-over is generic. It doesn't allow for role-based selection, forcing the user to re-input Tool/Model/Description even if they want a common role (e.g., "Reviewer").
- **Visual Polish**: Form inputs lack the distinct "taurhaus" brand feeling (needs softer shadows and better typography).

### Screenshot 230025 (Setup Mode - taurora)
- **Layout Problems**: The "Customize Team" slide-over is overwhelming. Showing full agent cards *within* a slide-over that already has global fields (Team Name/Description) creates a nested list problem that feels cramped.
- **Interaction Gaps**: Validation errors (e.g., "Team name is required") are just text labels. They should use a more integrated "red alert" border or icon to draw attention.

### Screenshot 230132 (Light Mode Empty State)
- **Layout Problems**: The preset cards are inconsistent in width. The "Research + D..." card being cut off without a clear horizontal scroll indicator is a usability failure.
- **Visual Hierarchy**: The primary CTA should be "Browse Catalog" or "Build Custom", but the preset cards dominate the screen.
- **Interaction Gaps**: In the History tab, the "Revert selected" button is visible even when no commit is selected, leading to potential confusion.

## 2. Gap Analysis

| Feature | Backend | IPC Layer | Frontend UI |
|---------|---------|-----------|-------------|
| List Roles/Presets | ✅ | ✅ | ✅ (Browser) |
| Create/Edit Role | ✅ | ✅ | ❌ |
| Edit Behavioral Contract | ✅ | ✅ | ❌ |
| Create/Edit Preset | ✅ | ✅ | ❌ |
| Capture Live Agent as Role| ✅ | ✅ | ❌ |
| Save Setup as Preset | ✅ | ✅ | ❌ |
| History Revert | ✅ | ✅ | ✅ (History tab) |

**Key Finding**: The UI is currently a "read-only" consumer of templates. We need to unlock the full CRUD capabilities of the backend.

## 3. User Journeys & Flow Diagrams

### Journey A: New User First Team
`Empty State` → `Pick Preset (e.g. Full Stack)` → `Auto-populate Canvas (Setup Mode)` → `Customize (Optional)` → `Initialize` → `Runtime`

### Journey B: Experienced User Custom Team
`Empty State` → `Build Custom` → `Drag Roles from Catalog to Canvas Slots` → `Assign Projects` → `Initialize` → `Runtime`

### Journey C: Template Management
`Catalog` → `Create Role (Editor)` → `Set Behavioral Contract` → `Save` → `Assign to New/Existing Preset`

## 4. ASCII Wireframes

### A. Template Catalog (Roles Tab)
The catalog provides a high-density management interface for atomic agent roles.

```text
+-----------------------------------------------------------+
| TEMPLATES                                             [X] |
+-----------------------------------------------------------|
| [ ROLES ]  [ PRESETS ]  [ HISTORY ]                       |
|-----------------------------------------------------------|
| [ Search roles... ]                          [ + CREATE ] |
|-----------------------------------------------------------|
| [Icon] Lead Orchestrator                       [BUILT-IN] |
|        Tool: Claude | Model: Opus                         |
|        Tags: [planning] [orchestration]                   |
|        ( Inspect ) ( Use )                                |
|-----------------------------------------------------------|
| [Icon] Frontend Dev                             [CUSTOM]  |
|        Tool: Codex | Model: gpt-5-mini                    |
|        Tags: [svelte] [tailwind]                          |
|        ( Inspect ) ( Edit ) ( Use ) [ Delete ]            |
+-----------------------------------------------------------+
```
- **Empty State**: "No custom roles yet. Create one or capture from a live team."
- **Actions**: `Inspect` (full detail view), `Use` (add to current setup), `Edit` (open Role Editor), `Delete` (custom only, triggers ConfirmDialog).

### B. Template Catalog (Presets Tab)
Presets are functional assemblies of roles.

```text
+-----------------------------------------------------------+
| TEMPLATES                                             [X] |
+-----------------------------------------------------------|
| [ ROLES ]  [ PRESETS ]  [ HISTORY ]                       |
|-----------------------------------------------------------|
| [ Search presets... ]                        [ + CREATE ] |
|-----------------------------------------------------------|
| Full Stack Dev Team                            [BUILT-IN] |
| Lead + 3 Agents (Claude, Codex, Gemini)                   |
| ( Inspect ) ( Use )                                       |
|-----------------------------------------------------------|
| Backend Sprint Team                             [CUSTOM]  |
| Lead + 2 Agents (Claude, Codex)                           |
| ( Inspect ) ( Edit ) ( Use ) [ Delete ]                   |
+-----------------------------------------------------------+
```
- **Edit Action**: Opens the Team Customizer pre-filled with the preset's composition.

### C. Role Editor (The "Atomic" Builder)
```text
+-----------------------------------------------------------+
| EDIT ROLE: Frontend Specialist                        [X] |
+-----------------------------------------------------------+
| ROLE ID: [ frontend-specialist-custom ]                   |
| TOOL:    [ Codex [V] ]    MODEL: [ gpt-5.3-codex [V] ]    |
+-----------------------------------------------------------+
| INSTRUCTIONS (Markdown Editor with Syntax Highlighting)   |
| +-------------------------------------------------------+ |
| | # Role Goal                                           | |
| | You are a Svelte 5 expert...                          | |
| +-------------------------------------------------------+ |
+-----------------------------------------------------------+
| BEHAVIORAL CONTRACT (Rule-based constraints)             |
| - [X] Always use Tailwind v4 for styling                |
| - [X] Prefer runes over legacy stores                   |
| - [ ] Add Rule: [ _____________________ ] [+]          |
+-----------------------------------------------------------+
| CAPABILITIES                                              |
| [ UI Development [x] ] [ Testing [x] ] [ + Add Tag ]      |
+-----------------------------------------------------------+
| [ PREVIEW/TEST ROLE ]      [ CANCEL ]      [ SAVE ROLE ]  |
+-----------------------------------------------------------+
```

### D. Role-Aware "Add Agent" Flow
Improved hot-add form with role templates.

```text
+------------------------------------------+
| ADD AGENT TO TEAM                    [X] |
+------------------------------------------+
| PICK FROM ROLE (Optional)                |
| [ Select a Role template...        [V] ] |
|------------------------------------------|
| MANUAL CONFIGURATION                     |
| Name:   [ _____________________ ]        |
| Tool:   [ Claude      [V] ]              |
| Model:  [ Sonnet      [V] ]              |
| Project:[ taurhaus    [V] ]              |
| Desc:   [ _____________________ ]        |
|------------------------------------------|
| [ Cancel ]                 [ Add Agent ] |
+------------------------------------------+
```
- **Selection Behavior**: Selecting a role auto-fills all manual fields and locks them (with an "Unlock/Edit" toggle).

### E. "Capture as Role" Flow
Capturing live agent state into the library.

```text
+------------------------------------------+
| CAPTURE AGENT AS ROLE                [X] |
+------------------------------------------+
| Agent "dev-1" has a unique configuration. |
| Save this as a reusable template?        |
|                                          |
| New Role Name: [ Senior Svelte Dev     ] |
| Role ID:       [ svelte-dev-captured   ] |
|                                          |
| [ ] Include current instructions         |
| [ ] Include behavioral contract          |
|                                          |
| [ Cancel ]             [ Save to Catalog ] |
+------------------------------------------+
```
- **Result**: The new role appears immediately in the Template Catalog's "Roles" tab under [CUSTOM].

### F. Team Customizer (Preset Focus)
Adding the ability to save a configuration as a preset.

```text
+------------------------------------------+
| CUSTOMIZE TEAM                       [X] |
+------------------------------------------+
| TEAM NAME: [ my-feature-team           ] |
| ... (Lead and Agent list) ...            |
|------------------------------------------|
| [ + Add Agent Slot ]                     |
|------------------------------------------|
| [ SAVE AS NEW PRESET ]                   |
|------------------------------------------|
| [ Reset ]                    [ Apply ]   |
+------------------------------------------+
```
- **Save Flow**: Clicking "Save as New Preset" opens a small modal asking for a name/description, then persists the entire composition (lead role + agent slots).

### G. Mesh Runtime (Toolbar & Canvas)
```text
+-----------------------------------------------------------+
| [Logo] my-team  •  [3 ACTIVE][0 IDLE]        [+][Gear][:] |
|-----------------------------------------------------------|
|                                                           |
|          ( Team Lead )                                    |
|         /      |      \                                   |
|   (Dev 1)   (Dev 2)   (Dev 3)                             |
|       |                                                   |
|       +---------------------+                             |
|       | NODE: Dev 1         |                             |
|       | Status: ACTIVE      |                             |
|       | Tool: Claude        |                             |
|       |---------------------|                             |
|       | [ RESUME ][ FOCUS ] |                             |
|       | [ STOP ] [ CAPTURE ]|                             |
|       +---------------------+                             |
+-----------------------------------------------------------+
```

## 5. Visual Language & Style

### 5.1 Light Mode Treatment
While dark mode is the default "hacker" aesthetic, light mode must feel airy and professional.
- **Canvas**: Keep the subtle dot grid, but switch to light grey dots on a `brand-50` background.
- **Nodes**: In light mode, node cards use a pure white background with a crisp 1px `zinc-200` border and a soft `shadow-sm`.
- **Toolbar**: Solid `zinc-50` background with a `zinc-200` bottom border to ground the interface.

### 5.2 Interaction Patterns
- **Stacked Slide-Overs**: Template Browser opens first. Clicking "Edit Role" opens the Role Editor *over* the browser, with a "Back" button to return to the catalog.
- **Drag & Drop**: In Setup Mode, users can drag role icons from the catalog directly onto empty node slots on the canvas.
- **Responsiveness**: Below 800px width, the catalog and editors switch to full-width overlays.

## 6. Edge Cases & Error States

- **No Presets**: "The catalog is empty. Start by creating your first role or importing a preset pack."
- **Validation Errors**: Inputs turn red with a shake animation; a "Validation Summary" appears above the Save button.
- **IPC Failure**: A persistent "toast" message at the top: "Failed to save template. [Retry] [Dismiss]".

## 7. Implementation Strategy

1. **IPC Expansion**: Add `upsert_role`, `delete_role`, `upsert_preset`, `delete_preset`.
2. **Catalog v2**: Implement the high-density grid for presets/roles.
3. **Role Editor**: Build the Markdown-enabled instruction editor.
4. **Capture/Save Flow**: Connect the [CAPTURE] and [SAVE AS PRESET] actions to the backend.
