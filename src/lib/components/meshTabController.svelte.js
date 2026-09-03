import { untrack } from 'svelte'
import {
  composeTeam,
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetLiveTeamStatus,
  coordinationGetProjectMeshSnapshot,
  coordinationRemoveMember,
  coordinationResumeMember,
  coordinationResumeTeam,
  coordinationSwitchTeamAccount,
  onCoordinationResumeTeamProgress,
  getTeamPreset,
  listRoleTemplates,
  listTeamPresets,
  upsertRoleTemplate,
  upsertTeamPreset,
} from '../ipc.js'
import {
  normalizeProjectMeshSnapshot,
  normalizeResumeTeamProgressEvent,
  normalizeResumeTeamReport,
} from '../ipc/coordinationResponses.js'
import { clearMeshCache, getMeshCacheEntry, setMeshCache } from '../meshCache.svelte.js'
import {
  normalizeTool,
  resolveRoleModel,
  resolveRoleReasoningEffort,
} from '../meshDefaults.js'
import { EMPTY_MODEL_CATALOG, defaultEffortFor, defaultModelFor } from '../modelCatalog.js'
import { normalizeProjectOption } from '../projectOptions.js'
import {
  buildCapturedRoleTemplate,
  buildInitializationRequest,
  buildTeamConfigFromPreset,
  buildTeamConfigFromRuntimeStatus,
  contractHasRules,
  createLead,
  inferTeamName,
  normalizeBehavioralContract,
  slugifyRoleId,
} from './meshTabUtils.js'
import {
  buildRuntimeAgentName,
  createAgentFromRole,
  createLeadFromRole,
  emptyBuilderConfig,
  mergePresetCatalog,
  normalizeRoleKind,
} from './meshBuilderUtils.js'
import { refreshRuntimeTeamConfigWorkflow } from './meshTabGateWorkflow.js'
import {
  createMeshTabPublicApi,
  createMeshTabRefs,
  createMeshTabState,
  QUICK_PRESETS,
} from './meshTabControllerState.svelte.js'
import { createMeshTabGate } from './meshTabGate.svelte.js'
import { createMeshTabInit } from './meshTabInit.svelte.js'
import { autoDismissNotice } from './meshTabNotifications.js'
import { createMeshTabRuntime } from './meshTabRuntime.svelte.js'
import { createMeshTabSetup } from './meshTabSetup.svelte.js'

export function createMeshTabController({
  getProjectPath,
  getIsVisible = () => true,
  getBackgroundWorkEnabled = () => true,
  getAvailableProjects,
  getModelCatalog = () => EMPTY_MODEL_CATALOG,
  onAddAgent,
  onDisband,
  onRemoveAgent,
  onFocusPane,
}) {
  const state = createMeshTabState(QUICK_PRESETS)
  const refs = createMeshTabRefs()

  const gate = createMeshTabGate({
    state,
    refs,
    deps: {
      buildTeamConfigFromRuntimeStatus,
      coordinationGetLiveTeamStatus,
      coordinationGetProjectMeshSnapshot,
      getBackgroundWorkEnabled,
      getIsVisible,
      getMeshCacheEntry,
      getProjectPath,
      inferTeamName,
      normalizeProjectMeshSnapshot,
      refreshRuntimeTeamConfigWorkflow,
      setMeshCache,
      untrack,
    },
  })

  const setup = createMeshTabSetup({
    state,
    refs,
    gate,
    deps: {
      buildCapturedRoleTemplate,
      buildRuntimeAgentName,
      buildTeamConfigFromPreset,
      contractHasRules,
      coordinationAddAgent,
      composeTeam,
      createAgentFromRole,
      createLeadFromRole,
      defaultEffortFor: (tool, model) => defaultEffortFor(getModelCatalog(), tool, model),
      defaultModelFor: (tool) => defaultModelFor(getModelCatalog(), tool),
      emptyBuilderConfig,
      getModelCatalog,
      getAvailableProjects,
      getProjectPath,
      getTeamPreset,
      inferTeamName,
      listRoleTemplates,
      listTeamPresets,
      mergePresetCatalog,
      normalizeBehavioralContract,
      normalizeProjectOption,
      normalizeRoleKind,
      normalizeTool,
      onAddAgent,
      resolveRoleModel,
      resolveRoleReasoningEffort,
      quickPresets: QUICK_PRESETS,
      slugifyRoleId,
      upsertRoleTemplate,
      upsertTeamPreset,
    },
  })

  const init = createMeshTabInit({
    state,
    refs,
    gate,
    setup,
    deps: {
      buildInitializationRequest,
      createLead,
      getModelCatalog,
      getProjectPath,
      inferTeamName,
    },
  })

  const runtime = createMeshTabRuntime({
    state,
    refs,
    gate,
    deps: {
      clearMeshCache,
      coordinationDisbandTeam,
      coordinationRemoveMember,
      coordinationResumeMember,
      coordinationResumeTeam,
      coordinationSwitchTeamAccount,
      onCoordinationResumeTeamProgress,
      getBackgroundWorkEnabled,
      getIsVisible,
      getProjectPath,
      normalizeResumeTeamProgressEvent,
      normalizeResumeTeamReport,
      onDisband,
      onFocusPane,
      onRemoveAgent,
    },
  })

  $effect(() => {
    const projectPath = getProjectPath()
    const isVisible = getIsVisible()
    const backgroundWorkEnabled = getBackgroundWorkEnabled()
    if (!isVisible || !backgroundWorkEnabled) {
      gate.invalidateDiscovery()
      return
    }
    untrack(() => {
      state.mode = 'empty'
      state.teamName = ''
      state.teamConfig = null
      state.slideOver = null
      state.slideOverContext = null
      state.captureRoleDialog = null
      state.selectedNodeId = null
      state.initProgress = null
      state.errorMessage = ''
      state.runtimeMessage = ''
      state.confirmContext = null
      state.availabilityMessage = ''
      gate.clearRuntimeTeamRefresh({ dropInFlight: true })
      gate.ensureHydrated(projectPath, { isVisible, backgroundWorkEnabled })
    })
  })

  $effect(() => {
    if (state.mode !== 'empty' && state.mode !== 'setup') return
    if (!state.teamName.trim()) {
      state.teamName = inferTeamName(getProjectPath())
    }
    if (state.loadingRoles || state.roleTemplates.length > 0) return
    void setup.loadRoleTemplates()
  })

  $effect(() => {
    if (state.mode !== 'empty' && state.mode !== 'setup') return
    if (state.loadingPresets || state.presetsLoaded) return
    void setup.loadTeamPresets()
  })

  $effect(() => {
    if (!state.selectedNodeId) {
      state.captureRoleDialog = null
      return
    }
    if (!state.selectedNode) {
      state.selectedNodeId = null
      state.captureRoleDialog = null
    }
  })

  $effect(() => {
    if (state.teamResumeProgress?.inFlight) {
      if (refs.teamResumeProgressTimer) {
        clearTimeout(refs.teamResumeProgressTimer)
        refs.teamResumeProgressTimer = null
      }
      return
    }

    return autoDismissNotice({
      value: state.teamResumeProgress ? 'completed' : '',
      timeoutMs: 5000,
      getTimer: () => refs.teamResumeProgressTimer,
      setTimer: (timer) => {
        refs.teamResumeProgressTimer = timer
      },
      clearValue: () => {
        state.teamResumeProgress = null
      },
    })
  })

  $effect(() => autoDismissNotice({
    value: state.runtimeMessage,
    timeoutMs: 5000,
    getTimer: () => refs.runtimeMessageTimer,
    setTimer: (timer) => {
      refs.runtimeMessageTimer = timer
    },
    clearValue: () => {
      state.runtimeMessage = ''
    },
  }))

  $effect(() => autoDismissNotice({
    value: state.errorMessage,
    timeoutMs: 8000,
    getTimer: () => refs.errorMessageTimer,
    setTimer: (timer) => {
      refs.errorMessageTimer = timer
    },
    clearValue: () => {
      state.errorMessage = ''
    },
  }))

  $effect(() => () => {
    gate.clearProjectSnapshotRefresh()
  })

  $effect(() => runtime.createRuntimePollingEffect())
  $effect(() => runtime.createResumeTeamProgressEffect())

  return createMeshTabPublicApi({ state, gate, setup, init, runtime })
}
