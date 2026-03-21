import HoverCardHost from './hosts/HoverCardHost.svelte'
import MeshCanvasHost from './hosts/MeshCanvasHost.svelte'
import MeshNodeDetailHost from './hosts/MeshNodeDetailHost.svelte'
import MeshTeamBuilderHost from './hosts/MeshTeamBuilderHost.svelte'
import SidebarHost from './hosts/SidebarHost.svelte'
import { configureVisualHostState } from './mockState.js'

import { hoverCardScenarios } from '../test/visual/fixtures/hoverCard.fixtures.js'
import { meshCanvasScenarios } from '../test/visual/fixtures/meshCanvas.fixtures.js'
import { meshNodeDetailScenarios } from '../test/visual/fixtures/meshNodeDetail.fixtures.js'
import { meshTeamBuilderScenarios } from '../test/visual/fixtures/meshTeamBuilder.fixtures.js'
import { sidebarScenarios } from '../test/visual/fixtures/sidebar.fixtures.js'

export const viewportPresets = [
  { id: 'desktop', label: 'Desktop 1920x1080', width: 1920, height: 1080 },
  { id: 'laptop', label: 'Laptop 1366x768', width: 1366, height: 768 },
  { id: 'narrow', label: 'Narrow 1024x768', width: 1024, height: 768 },
]

function applyNoopMocks() {
  configureVisualHostState({})
}

function applyHoverCardMocks(scenario) {
  configureVisualHostState({
    ipc: {
      getLatestSession: scenario?.ipc?.getLatestSession ?? null,
      getRecentCommits: scenario?.ipc?.getRecentCommits ?? [],
      getRelationships: scenario?.ipc?.getRelationships ?? [],
    },
  })
}

function applySidebarMocks(scenario) {
  configureVisualHostState({
    sessionStore: scenario?.sessionStore ?? {
      sessionsByProject: {},
      sessionByProject: {},
    },
  })
}

export const visualRegistry = [
  {
    id: 'mesh-canvas',
    label: 'MeshCanvas',
    component: MeshCanvasHost,
    scenarios: meshCanvasScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'hover-card',
    label: 'HoverCard',
    component: HoverCardHost,
    scenarios: hoverCardScenarios,
    applyMocks: applyHoverCardMocks,
  },
  {
    id: 'mesh-node-detail',
    label: 'MeshNodeDetail',
    component: MeshNodeDetailHost,
    scenarios: meshNodeDetailScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'mesh-team-builder',
    label: 'MeshTeamBuilder',
    component: MeshTeamBuilderHost,
    scenarios: meshTeamBuilderScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'sidebar',
    label: 'Sidebar',
    component: SidebarHost,
    scenarios: sidebarScenarios,
    applyMocks: applySidebarMocks,
  },
]

export function getRegistryEntry(componentId) {
  return visualRegistry.find((entry) => entry.id === componentId) ?? visualRegistry[0]
}
