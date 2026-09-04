/**
 * Pulled-row bridge scenarios: the Shell body junction (rail · gutter ·
 * panel), so the one thing the sidebar-only fixtures cannot show — the
 * pulled material continuing across the frame gutter into the main panel —
 * is on the record in both themes, scrolled, clipped, and held.
 */

function createProject({ id, name, branch = 'main', isDirty = false } = {}) {
  return {
    id,
    name,
    path: `/projects/${id}`,
    activityState: 'active',
    branch,
    isDirty,
  }
}

/** A short list that fits the rail: no scrollbar, the pulled row is flush. */
function shortList() {
  return [
    createProject({ id: 'atlas', name: 'Atlas Service' }),
    createProject({ id: 'meridian', name: 'Meridian UI', branch: 'feat/drawer-rail' }),
    createProject({ id: 'quarry', name: 'Quarry Tools' }),
    createProject({ id: 'lantern', name: 'Lantern Docs', isDirty: true }),
    createProject({ id: 'foundry', name: 'Foundry Ops' }),
  ]
}

/** A list long enough to overflow the rail at 640px: the classic scrollbar
 * takes its 8px lane and the bridge's in-rail cover comes into play. */
function longList() {
  return Array.from({ length: 30 }, (_, index) =>
    createProject({
      id: `depot-${String(index).padStart(2, '0')}`,
      name: `Depot ${String(index).padStart(2, '0')}`,
      branch: index === 12 ? 'feat/pulled-row-bridge' : 'main',
    })
  )
}

function createScenario({
  name,
  theme,
  projects,
  selectedIndex = 1,
  scrollTo = null,
  props = {},
  forceJsTracking = false,
  expected,
}) {
  return {
    name,
    theme,
    projects,
    selectedProject: projects[selectedIndex] ?? null,
    daemonStatus: 'connected',
    scrollTo,
    props,
    forceJsTracking,
    expected: { bridge: true, lane: false, ...expected },
  }
}

export const sidebarBridgeScenarios = [
  createScenario({
    name: 'bridge_flush_dark',
    theme: 'dark',
    projects: shortList(),
    selectedIndex: 1,
    expected: { bridge: true, lane: false },
  }),
  createScenario({
    name: 'bridge_flush_light',
    theme: 'light',
    projects: shortList(),
    selectedIndex: 1,
    expected: { bridge: true, lane: false },
  }),
  createScenario({
    name: 'bridge_scrolled_lane_dark',
    theme: 'dark',
    projects: longList(),
    selectedIndex: 12,
    scrollTo: 240,
    expected: { bridge: true, lane: true },
  }),
  createScenario({
    name: 'bridge_scrolled_lane_light',
    theme: 'light',
    projects: longList(),
    selectedIndex: 12,
    scrollTo: 240,
    expected: { bridge: true, lane: true },
  }),
  createScenario({
    // The pulled row scrolled half out at the list's top edge: the bridge
    // must clip on the same line the scroll container clips the row.
    name: 'bridge_clipped_top_dark',
    theme: 'dark',
    projects: longList(),
    selectedIndex: 12,
    scrollTo: 500,
    expected: { bridge: true, lane: true, clippedTop: true },
  }),
  createScenario({
    // Utility surface open: the row demotes to held and the footer key wears
    // the material — with no bridge for either (the footer-key ruling).
    name: 'bridge_held_settings_dark',
    theme: 'dark',
    projects: shortList(),
    selectedIndex: 1,
    props: { settingsOpen: true },
    expected: { bridge: false, lane: false },
  }),
  createScenario({
    // The JS tracking fallback (engines without scroll-driven animations)
    // must land the strip on the same pixels as the compositor path.
    name: 'bridge_js_tracking_dark',
    theme: 'dark',
    projects: longList(),
    selectedIndex: 12,
    scrollTo: 240,
    forceJsTracking: true,
    expected: { bridge: true, lane: true },
  }),
]
