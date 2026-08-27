import AccountHost from './hosts/AccountHost.svelte'
import HoverCardHost from './hosts/HoverCardHost.svelte'
import MeshCanvasHost from './hosts/MeshCanvasHost.svelte'
import MeshNodeDetailHost from './hosts/MeshNodeDetailHost.svelte'
import MeshTeamBuilderHost from './hosts/MeshTeamBuilderHost.svelte'
import ModelSelectHost from './hosts/ModelSelectHost.svelte'
import RosterDesignAHost from './hosts/RosterDesignAHost.svelte'
import RosterDesignBHost from './hosts/RosterDesignBHost.svelte'
import RosterDesignCHost from './hosts/RosterDesignCHost.svelte'
import ShellPopupsHost from './hosts/ShellPopupsHost.svelte'
import SidebarHost from './hosts/SidebarHost.svelte'
import { configureVisualHostState } from './mockState.js'

import { accountScenarios } from '../test/visual/fixtures/account.fixtures.js'
import { hoverCardScenarios } from '../test/visual/fixtures/hoverCard.fixtures.js'
import { meshCanvasScenarios } from '../test/visual/fixtures/meshCanvas.fixtures.js'
import { meshNodeDetailScenarios } from '../test/visual/fixtures/meshNodeDetail.fixtures.js'
import { meshTeamBuilderScenarios } from '../test/visual/fixtures/meshTeamBuilder.fixtures.js'
import { modelSelectScenarios } from '../test/visual/fixtures/modelSelect.fixtures.js'
import { rosterDesignScenarios } from '../test/visual/fixtures/rosterDesigns.fixtures.js'
import { shellPopupsScenarios } from '../test/visual/fixtures/shellPopups.fixtures.js'
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
  {
    id: 'account',
    label: 'Account',
    component: AccountHost,
    scenarios: accountScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'shell-popups',
    label: 'Shell popups (account chooser / chip menu / account submenu)',
    component: ShellPopupsHost,
    scenarios: shellPopupsScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'model-select',
    label: 'ModelSelect',
    component: ModelSelectHost,
    scenarios: modelSelectScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'roster-design-a',
    label: 'Roster Design A — "The Bench"',
    component: RosterDesignAHost,
    scenarios: rosterDesignScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'roster-design-b',
    label: 'Roster Design B — "Spotlight"',
    component: RosterDesignBHost,
    scenarios: rosterDesignScenarios,
    applyMocks: applyNoopMocks,
  },
  {
    id: 'roster-design-c',
    label: 'Roster Design C — "The Split"',
    component: RosterDesignCHost,
    scenarios: rosterDesignScenarios,
    applyMocks: applyNoopMocks,
  },
]

export function getRegistryEntry(componentId) {
  return visualRegistry.find((entry) => entry.id === componentId) ?? visualRegistry[0]
}
