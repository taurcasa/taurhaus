<script>
  import { normalizeTool } from '../meshDefaults.js'
  import { getModelCatalogContext } from '../context/ModelCatalogContext.js'
  import { EMPTY_MODEL_CATALOG, roleDeclaredEffort } from '../modelCatalog.js'
  import { normalizeProjectOption } from '../projectOptions.js'
  import { themeTokens } from '../themeTokens.js'
  import { getToolIcon } from '../toolLogos.js'
  import { toolAccent, toolCounts, toolLabel, tools } from '../toolRegistry.js'
  import MeshCanvas from './MeshCanvas.svelte'
import MeshNodeDetail from './MeshNodeDetail.svelte'
  import MeshRuntimeBar from './MeshRuntimeBar.svelte'
  import ModelSelect from './ModelSelect.svelte'
  import SlideOver from './SlideOver.svelte'

  let {
    dark = false,
    teamName = '',
    teamConfig = null,
    selectedNode = null,
    selectedNodeId = null,
    teamRuntimeState = 'none',
    isResumingTeam = false,
    resumeProgress = null,
    availableProjects = [],
    addAgentOpen = false,
    addAgentDraft = null,
    canSubmitAddAgent = false,
    roleTemplates = [],
    loadingRoles = false,
    captureRoleDraft = null,
    canSaveCapturedRole = false,
    modelCatalog = null,
    onNodeClick = () => {},
    onOpenAddAgent = () => {},
    onRequestDisband = () => {},
    onResumeTeam = () => {},
    onCloseNode = () => {},
    onResumeSelected = () => {},
    onStopSelected = () => {},
    onFocusSelectedPane = () => {},
    onCaptureRole = () => {},
    onCloseAddAgent = () => {},
    onAddAgentRoleChange = () => {},
    onToggleAddAgentLock = () => {},
    onUpdateAddAgentField = () => {},
    onSubmitAddAgent = () => {},
    onCloseCaptureRole = () => {},
    onCaptureRoleName = () => {},
    onCaptureRoleId = () => {},
    onToggleCaptureRoleFlag = () => {},
    onSubmitCaptureRole = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const modelCatalogContext = getModelCatalogContext()
  const catalog = $derived(modelCatalog ?? modelCatalogContext?.catalog ?? EMPTY_MODEL_CATALOG)
  const toolOptions = tools()
  let nodeDetailAnchor = $state(null)
  let detailOpenPerf = $state(null)
  const detailNode = $derived.by(() => {
    if (!selectedNode || typeof selectedNode !== 'object') return null
    return {
      name: selectedNode.name,
      role: selectedNode.role,
      tool: selectedNode.tool ?? selectedNode.cliTool ?? selectedNode.cli_tool,
      model: selectedNode.model ?? selectedNode.modelName ?? selectedNode.model_name,
      reasoningEffort: selectedNode.reasoningEffort ?? selectedNode.reasoning_effort ?? null,
      status: selectedNode.status ?? selectedNode.sessionStatus ?? selectedNode.session_status,
      projectId: selectedNode.projectId ?? selectedNode.project_id,
      isCrossProject: selectedNode.isCrossProject ?? selectedNode.is_cross_project ?? false,
      projectLabel: selectedNode.projectLabel ?? selectedNode.project_label ?? '',
      description: selectedNode.description ?? '',
      roleName: selectedNode.roleName ?? selectedNode.role_name ?? '',
      focusArea: selectedNode.focusArea ?? selectedNode.focus_area ?? '',
      contextSummary: selectedNode.contextSummary ?? selectedNode.context_summary ?? '',
      behaviorSummary: selectedNode.behaviorSummary ?? selectedNode.behavior_summary ?? '',
      paneId: selectedNode.paneId ?? selectedNode.pane_id ?? '',
      sessionId: selectedNode.sessionId ?? selectedNode.session_id ?? '',
      sessionState: selectedNode.sessionState ?? selectedNode.session_status ?? '',
    }
  })
  const canFocusSelectedPane = $derived.by(
    () => Boolean(selectedNode?.paneId ?? selectedNode?.pane_id)
  )
  const fieldTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )
  const selectScheme = $derived(dark ? '[color-scheme:dark]' : '[color-scheme:light]')

  const projectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
  )
  let roleSearchQuery = $state('')
  let activeRoleToolFilter = $state('all')
  let activeRoleKindFilter = $state('agent')
  let addAgentWasOpen = $state(false)

  const normalizedRoleTemplates = $derived.by(() =>
    (roleTemplates ?? [])
      .filter((role) => role && (role.roleId || role.role_id || role.name))
      .map((role) => {
        const tool = normalizeTool(role.cliTool ?? role.cli_tool ?? 'codex')
        return {
          ...role,
          roleId: String(role.roleId ?? role.role_id ?? ''),
          name: String(role.name ?? role.roleId ?? role.role_id ?? 'Unnamed role'),
          kind: String(role.kind ?? 'agent').trim().toLowerCase() === 'lead' ? 'lead' : 'agent',
          cliTool: tool,
          model: String(role.model ?? role.defaults?.model ?? ''),
          summary: String(
            role.behaviorSummary ??
            role.behavior_summary ??
            role.contextSummary ??
            role.context_summary ??
            role.instructions ??
            ''
          ).trim(),
        }
      })
  )
  const filteredRoleTemplates = $derived.by(() => {
    const query = roleSearchQuery.trim().toLowerCase()
    return normalizedRoleTemplates.filter((role) => {
      if (activeRoleToolFilter !== 'all' && role.cliTool !== activeRoleToolFilter) return false
      if (activeRoleKindFilter !== 'all' && role.kind !== activeRoleKindFilter) return false
      if (!query) return true
      return (
        role.name.toLowerCase().includes(query) ||
        role.roleId.toLowerCase().includes(query) ||
        role.cliTool.toLowerCase().includes(query) ||
        role.model.toLowerCase().includes(query)
      )
    })
  })
  const visibleLeadRoleTemplates = $derived(filteredRoleTemplates.filter((role) => role.kind === 'lead'))
  const visibleAgentRoleTemplates = $derived(filteredRoleTemplates.filter((role) => role.kind === 'agent'))
  const visibleRoleCount = $derived(filteredRoleTemplates.length)
  const roleToolCounts = $derived.by(() =>
    toolCounts(normalizedRoleTemplates, (role) => role.cliTool)
  )
  const roleKindCounts = $derived.by(() => ({
    all: normalizedRoleTemplates.length,
    lead: normalizedRoleTemplates.filter((role) => role.kind === 'lead').length,
    agent: normalizedRoleTemplates.filter((role) => role.kind === 'agent').length,
  }))
  const selectedRuntimeRole = $derived.by(() => {
    const roleId = String(addAgentDraft?.roleId ?? '').trim()
    if (!roleId) return null
    return normalizedRoleTemplates.find((role) => role.roleId === roleId) ?? null
  })

  function filterButtonTone(active) {
    if (active) {
      return dark
        ? 'border-brand-400/50 bg-brand-500/18 text-zinc-100 shadow-[0_0_0_1px_rgba(45,212,191,0.14)]'
        : 'border-brand-400/60 bg-brand-50 text-brand-900 shadow-[0_0_0_1px_rgba(15,118,110,0.08)]'
    }
    return dark
      ? 'border-white/[0.08] bg-white/[0.03] text-zinc-400 hover:bg-white/[0.06]'
      : 'border-zinc-200 bg-white text-zinc-600 hover:bg-zinc-50'
  }

  function roleMedallionTone(tool) {
    switch (toolAccent(tool)) {
      case 'emerald':
        return dark
          ? 'border-amber-400/35 bg-amber-500/12 text-amber-200'
          : 'border-amber-300/70 bg-amber-50 text-amber-800'
      case 'violet':
        return dark
          ? 'border-sky-400/35 bg-sky-500/12 text-sky-200'
          : 'border-sky-300/70 bg-sky-50 text-sky-800'
      default:
        return dark
          ? 'border-emerald-400/35 bg-emerald-500/12 text-emerald-200'
          : 'border-emerald-300/70 bg-emerald-50 text-emerald-800'
    }
  }

  function roleChipTone(role) {
    if (role.kind === 'lead') {
      return dark
        ? 'border-danger-400/35 bg-danger-500/10 text-danger-200'
        : 'border-danger-300/70 bg-danger-50 text-danger-800'
    }
    return dark
      ? 'border-white/[0.08] bg-white/[0.05] text-zinc-300'
      : 'border-zinc-200 bg-zinc-50 text-zinc-700'
  }

  function runtimeRoleCardTone(role) {
    if (selectedRuntimeRole?.roleId === role.roleId) {
      return dark
        ? 'border-brand-400/50 bg-brand-500/10 shadow-[0_0_0_1px_rgba(45,212,191,0.14)]'
        : 'border-brand-400/60 bg-brand-50 shadow-[0_0_0_1px_rgba(15,118,110,0.08)]'
    }
    return dark
      ? 'border-white/[0.08] bg-white/[0.03] hover:bg-white/[0.05]'
      : 'border-zinc-200 bg-white hover:bg-zinc-50'
  }

  function toggleRoleToolFilter(tool) {
    activeRoleToolFilter = activeRoleToolFilter === tool ? 'all' : tool
  }

  function toggleRoleKindFilter(kind) {
    activeRoleKindFilter = activeRoleKindFilter === kind ? 'all' : kind
  }

  $effect(() => {
    if (addAgentOpen && !addAgentWasOpen) {
      roleSearchQuery = ''
      activeRoleToolFilter = 'all'
      activeRoleKindFilter = 'agent'
    }
    addAgentWasOpen = addAgentOpen
  })

  function nowMs() {
    if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
      return performance.now()
    }
    return Date.now()
  }

  function logDetailPerf(stage, perf) {
    if (!perf?.startedAt) return
    if (!globalThis.__TAURHAUS_MESH_DETAIL_PERF__) return
    const elapsedMs = Number((nowMs() - perf.startedAt).toFixed(1))
    console.debug('[mesh.perf] runtime-node-detail', {
      stage,
      elapsedMs,
      nodeId: perf.nodeId,
    })
  }

  function handleRuntimeNodeClick(nodeId) {
    const nextNodeId = nodeId === null || nodeId === undefined ? null : String(nodeId)
    if (nextNodeId && String(selectedNodeId) !== nextNodeId) {
      detailOpenPerf = {
        nodeId: nextNodeId,
        startedAt: nowMs(),
        renderedLogged: false,
        visibleLogged: false,
      }
    } else {
      detailOpenPerf = null
    }
    onNodeClick(nodeId)
  }

  function handleDetailAnchorChange(anchor) {
    nodeDetailAnchor = anchor
  }

  function handleDetailVisible() {
    const perf = detailOpenPerf
    if (!perf?.startedAt || perf.visibleLogged) return
    logDetailPerf('visible', perf)
    detailOpenPerf = {
      ...perf,
      visibleLogged: true,
    }
  }

  $effect(() => {
    if (selectedNode) return
    nodeDetailAnchor = null
    detailOpenPerf = null
  })

  $effect(() => {
    const perf = detailOpenPerf
    if (!perf?.startedAt || perf.renderedLogged) return
    if (!detailNode) return
    logDetailPerf('rendered', perf)
    detailOpenPerf = {
      ...perf,
      renderedLogged: true,
    }
  })
</script>

<div class="px-4 pt-2 pb-4 space-y-3">
  <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-runtime">
    <MeshRuntimeBar
      {teamName}
      lead={teamConfig?.lead ?? null}
      agents={teamConfig?.agents ?? []}
      teamRuntimeState={teamRuntimeState}
      runtimeSnapshotFreshness={teamConfig?.runtimeSnapshotFreshness ?? null}
      {dark}
      actionsDisabled={isResumingTeam}
      onAddAgent={onOpenAddAgent}
      onDisband={onRequestDisband}
      onResumeTeam={onResumeTeam}
    />

    {#if resumeProgress?.items?.length}
      <section
        class="rounded-xl border px-4 py-3 {dark ? 'border-white/10 bg-white/[0.03]' : 'border-brand-200/70 bg-white/80'}"
        data-testid="mesh-runtime-resume-progress"
      >
        <div class="mb-2 flex items-center justify-between gap-3">
          <div class="min-w-0">
            <h3 class="text-sm font-semibold {dark ? 'text-zinc-100' : 'text-zinc-900'}" data-testid="mesh-runtime-resume-header">
              {#if isResumingTeam}
                Resuming {resumeProgress?.currentIndex || 0} of {resumeProgress?.memberCount || resumeProgress?.items?.length || 0} members
              {:else}
                Latest resume result
              {/if}
            </h3>
            {#if resumeProgress?.activeMemberName}
              <p class="mt-0.5 text-[11px] {dark ? 'text-zinc-400' : 'text-zinc-500'}" data-testid="mesh-runtime-resume-subtitle">
                {resumeProgress.activeMemberName} · {resumeProgress.activeStageLabel || 'Working'}
              </p>
            {:else if resumeProgress?.summaryMessage}
              <p class="mt-0.5 text-[11px] {dark ? 'text-zinc-400' : 'text-zinc-500'}" data-testid="mesh-runtime-resume-subtitle">
                {resumeProgress.summaryMessage}
              </p>
            {/if}
          </div>
          <span class="text-[11px] uppercase tracking-wide {dark ? 'text-zinc-400' : 'text-zinc-500'}">
            {isResumingTeam ? 'In progress' : 'Completed'}
          </span>
        </div>

        <ul class="space-y-1.5">
          {#each resumeProgress.items as item}
            <li class="flex items-start justify-between gap-3 text-xs" data-testid={`mesh-runtime-resume-item-${item.memberName}`}>
              <div class="flex min-w-0 items-center gap-2">
                <span class="mt-0.5 inline-flex h-4 w-4 items-center justify-center rounded-full text-[10px] font-bold {item.status === 'succeeded' ? (dark ? 'bg-success-500/20 text-success-300' : 'bg-success-100 text-success-700') : item.status === 'failed' ? (dark ? 'bg-danger-500/20 text-danger-300' : 'bg-danger-100 text-danger-700') : (dark ? 'bg-zinc-800 text-zinc-300' : 'bg-zinc-100 text-zinc-600')}">
                  {item.status === 'succeeded' ? '✓' : item.status === 'failed' ? '×' : '…'}
                </span>
                <span class="truncate {dark ? 'text-zinc-100' : 'text-zinc-900'}">{item.memberName}</span>
              </div>
              <span class="shrink-0 text-right {item.status === 'failed' ? (dark ? 'text-danger-300' : 'text-danger-700') : item.status === 'running' ? (dark ? 'text-brand-300' : 'text-brand-700') : dark ? 'text-zinc-400' : 'text-zinc-500'}">
                {item.message}
              </span>
            </li>
          {/each}
        </ul>

        {#if resumeProgress?.footerMessage}
          <p class="mt-3 border-t pt-2 text-[11px] {dark ? 'border-white/8 text-zinc-400' : 'border-brand-100 text-zinc-500'}" data-testid="mesh-runtime-resume-footer">
            {resumeProgress.footerMessage}
          </p>
        {/if}
      </section>
    {/if}

    <div class="relative" data-testid="mesh-runtime-canvas-frame">
      <MeshCanvas
        lead={teamConfig?.lead ?? null}
        agents={teamConfig?.agents ?? []}
        mode="runtime"
        {dark}
        {selectedNodeId}
        onNodeClick={handleRuntimeNodeClick}
        onAddClick={() => {}}
        onDetailAnchorChange={handleDetailAnchorChange}
        onDismissDetail={onCloseNode}
      />

      {#if detailNode}
        <MeshNodeDetail
          node={detailNode}
          mode="runtime"
          {dark}
          modelCatalog={catalog}
          anchor={nodeDetailAnchor}
          onVisible={handleDetailVisible}
          actions={{
            onResume: onResumeSelected,
            resumeDisabled: isResumingTeam,
            onStop: onStopSelected,
            stopDisabled: isResumingTeam,
            onFocusPane: canFocusSelectedPane ? onFocusSelectedPane : null,
            onCapture: onCaptureRole,
            captureDisabled: isResumingTeam,
            onClose: onCloseNode,
          }}
        />
      {/if}
    </div>
  </div>
</div>

<SlideOver
  open={addAgentOpen}
  title="Add Agent"
  width={560}
  {dark}
  onClose={onCloseAddAgent}
>
  {#snippet children()}
    <section class="space-y-5 animate-in fade-in slide-in-from-bottom-1 duration-200" data-testid="mesh-add-agent-form">
      <p class="text-sm {t.textMuted} px-1">
        Pick an agent role from the catalog, keep the live mesh in view, and hot-add one member to
        <span class="font-medium {t.textSecondary}">{teamName}</span>.
      </p>

      <div class="space-y-3 rounded-xl border p-3 transition-all {dark ? 'bg-brand-500/[0.03] border-brand-500/20 border-l-2 border-l-brand-500' : 'bg-brand-50/50 border-brand-200 border-l-2 border-l-brand-500'}">
        <div class="flex items-start justify-between gap-3">
          <div>
            <p class="text-[10px] font-bold uppercase tracking-[0.2em] text-brand-500">Role Catalog</p>
            <p class="mt-1 text-xs {t.textMuted}">
              Use the same role catalog filters as the builder. Lead roles stay visible for reference but cannot be hot-added here.
            </p>
          </div>
          {#if addAgentDraft?.roleId}
            <button
              type="button"
              class="shrink-0 rounded-lg border px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.08em] {dark ? 'border-white/[0.08] bg-white/[0.03] text-zinc-300 hover:bg-white/[0.06]' : 'border-zinc-200 bg-white text-zinc-600 hover:bg-zinc-50'}"
              onclick={() => onAddAgentRoleChange('')}
              disabled={addAgentDraft?.submitting}
              data-testid="mesh-add-agent-clear-role"
            >
              Clear
            </button>
          {/if}
        </div>

        <label class="block">
          <span class="sr-only">Search runtime add-agent roles</span>
          <input
            class="h-10 w-full rounded-xl border px-3 text-sm outline-none {fieldTone}"
            placeholder="Search roles by name, id, or tool"
            value={roleSearchQuery}
            oninput={(event) => {
              roleSearchQuery = event.currentTarget.value
            }}
            data-testid="mesh-add-agent-role-search"
          />
        </label>

        <div class="space-y-2">
          <div class="flex items-center justify-between">
            <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Quick Filters</p>
            <span class="text-[10px] {t.textMuted}">{visibleRoleCount} visible</span>
          </div>
          <div class="flex flex-wrap gap-2">
            <button
              class="inline-flex h-9 items-center gap-2 rounded-xl border px-3 text-[11px] font-semibold transition {filterButtonTone(activeRoleToolFilter === 'all')}"
              type="button"
              onclick={() => {
                activeRoleToolFilter = 'all'
              }}
              data-testid="mesh-add-agent-filter-tool-all"
            >
              <span>All tools</span>
              <span class="text-[10px] {t.textMuted}">{roleToolCounts.all}</span>
            </button>
            {#each toolOptions as descriptor (descriptor.id)}
              {@const tool = descriptor.id}
              <button
                class="inline-flex h-9 items-center gap-2 rounded-xl border px-3 text-[11px] font-semibold transition {filterButtonTone(activeRoleToolFilter === tool)}"
                type="button"
                onclick={() => toggleRoleToolFilter(tool)}
                data-testid={`mesh-add-agent-filter-tool-${tool}`}
              >
                <span class="inline-flex h-5 w-5 items-center justify-center rounded-full border {roleMedallionTone(tool)}">
                  <svg class="h-3 w-3" viewBox={getToolIcon(tool, 'sidebarSmall').viewBox} fill="currentColor" aria-hidden="true">
                    <path d={getToolIcon(tool, 'sidebarSmall').path}></path>
                  </svg>
                </span>
                <span>{descriptor.label}</span>
                <span class="text-[10px] {t.textMuted}">{roleToolCounts[tool]}</span>
              </button>
            {/each}
          </div>
          <div class="flex flex-wrap gap-2">
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[10px] font-bold uppercase tracking-[0.12em] transition {filterButtonTone(activeRoleKindFilter === 'all')}"
              type="button"
              onclick={() => {
                activeRoleKindFilter = 'all'
              }}
              data-testid="mesh-add-agent-filter-kind-all"
            >
              All roles
              <span class="text-[10px] normal-case tracking-normal {t.textMuted}">{roleKindCounts.all}</span>
            </button>
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[10px] font-bold uppercase tracking-[0.12em] transition {filterButtonTone(activeRoleKindFilter === 'agent')}"
              type="button"
              onclick={() => toggleRoleKindFilter('agent')}
              data-testid="mesh-add-agent-filter-kind-agent"
            >
              Agent
              <span class="text-[10px] normal-case tracking-normal {t.textMuted}">{roleKindCounts.agent}</span>
            </button>
            <button
              class="inline-flex h-8 items-center gap-2 rounded-full border px-3 text-[10px] font-bold uppercase tracking-[0.12em] transition {filterButtonTone(activeRoleKindFilter === 'lead')}"
              type="button"
              onclick={() => toggleRoleKindFilter('lead')}
              data-testid="mesh-add-agent-filter-kind-lead"
            >
              Lead
              <span class="text-[10px] normal-case tracking-normal {t.textMuted}">{roleKindCounts.lead}</span>
            </button>
          </div>
        </div>

        {#if loadingRoles}
          <div class="rounded-[18px] border border-dashed p-5 text-center {dark ? 'border-white/[0.08] bg-white/[0.03]' : 'border-zinc-200 bg-white/80'}" data-testid="mesh-add-agent-role-loading">
            <p class="text-[12px] font-semibold {t.textPrimary}">Loading role catalog...</p>
            <p class="mt-1 text-[11px] {t.textSecondary}">Fetching the latest role templates for this runtime add.</p>
          </div>
        {:else if visibleRoleCount === 0}
          <div class="rounded-[18px] border border-dashed p-5 text-center {dark ? 'border-white/[0.08] bg-white/[0.03]' : 'border-zinc-200 bg-white/80'}" data-testid="mesh-add-agent-empty-results">
            <p class="text-[12px] font-semibold {t.textPrimary}">No roles match these filters</p>
            <p class="mt-1 text-[11px] {t.textSecondary}">Clear a tool or kind filter, or widen the search query.</p>
          </div>
        {:else}
          <div class="max-h-[320px] space-y-3 overflow-y-auto pr-1" data-testid="mesh-add-agent-role-catalog">
            {#if activeRoleKindFilter !== 'agent' && visibleLeadRoleTemplates.length > 0}
              <section data-testid="mesh-add-agent-role-section-leads">
                <div class="mb-2 flex items-center justify-between">
                  <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Leads</p>
                  <span class="text-[10px] {t.textMuted}">{visibleLeadRoleTemplates.length}</span>
                </div>
                <div class="grid gap-2 sm:grid-cols-2">
                  {#each visibleLeadRoleTemplates as role (role.roleId)}
                    <button
                      class="relative flex min-h-[108px] flex-col gap-2 overflow-hidden rounded-[18px] border p-2.5 text-left opacity-75 transition {runtimeRoleCardTone(role)}"
                      type="button"
                      disabled
                      aria-disabled="true"
                      data-testid={`mesh-add-agent-role-card-${role.roleId}`}
                    >
                      <div class="flex items-start justify-between gap-2">
                        <span class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border {roleMedallionTone(role.cliTool)}">
                          <svg class="h-4 w-4" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                            <path d={getToolIcon(role.cliTool).path}></path>
                          </svg>
                        </span>
                        <span class="rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] {roleChipTone(role)}">
                          Lead only
                        </span>
                      </div>
                      <div class="min-w-0">
                        <p class="truncate text-[12px] font-semibold {t.textPrimary}">{role.name}</p>
                        <p class="mt-1 text-[10px] font-medium uppercase tracking-[0.12em] {t.textMuted}">
                          {toolLabel(role.cliTool)} · {role.model}
                        </p>
                        <p class="mt-2 text-[11px] leading-4 {t.textSecondary}">
                          {role.summary || 'Direction-setting lead role.'}
                        </p>
                        <p class="mt-2 text-[10px] font-medium text-danger-500">
                          Lead roles can’t be hot-added through Add Agent.
                        </p>
                      </div>
                    </button>
                  {/each}
                </div>
              </section>
            {/if}

            {#if activeRoleKindFilter !== 'lead' && visibleAgentRoleTemplates.length > 0}
              <section data-testid="mesh-add-agent-role-section-agents">
                <div class="mb-2 flex items-center justify-between">
                  <p class="text-[10px] font-bold uppercase tracking-[0.2em] {t.textMuted}">Agents</p>
                  <span class="text-[10px] {t.textMuted}">{visibleAgentRoleTemplates.length}</span>
                </div>
                <div class="grid gap-2 sm:grid-cols-2">
                  {#each visibleAgentRoleTemplates as role (role.roleId)}
                    <button
                      class="relative flex min-h-[108px] flex-col gap-2 overflow-hidden rounded-[18px] border p-2.5 text-left transition {runtimeRoleCardTone(role)}"
                      type="button"
                      onclick={() => onAddAgentRoleChange(role.roleId)}
                      disabled={addAgentDraft?.submitting}
                      aria-pressed={selectedRuntimeRole?.roleId === role.roleId}
                      data-testid={`mesh-add-agent-role-card-${role.roleId}`}
                    >
                      <div class="flex items-start justify-between gap-2">
                        <span class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border {roleMedallionTone(role.cliTool)}">
                          <svg class="h-4 w-4" viewBox={getToolIcon(role.cliTool).viewBox} fill="currentColor" aria-hidden="true">
                            <path d={getToolIcon(role.cliTool).path}></path>
                          </svg>
                        </span>
                        <span class="rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] {roleChipTone(role)}">
                          Agent
                        </span>
                      </div>
                      <div class="min-w-0">
                        <p class="truncate text-[12px] font-semibold {t.textPrimary}">{role.name}</p>
                        <p class="mt-1 text-[10px] font-medium uppercase tracking-[0.12em] {t.textMuted}">
                          {toolLabel(role.cliTool)} · {role.model}
                        </p>
                        <p class="mt-2 text-[11px] leading-4 {t.textSecondary}">
                          {role.summary || 'Execution-focused specialist role.'}
                        </p>
                      </div>
                    </button>
                  {/each}
                </div>
              </section>
            {/if}
          </div>
        {/if}

        {#if selectedRuntimeRole}
          <div class="rounded-xl border p-3 {dark ? 'border-brand-400/30 bg-brand-500/8' : 'border-brand-200 bg-white/80'}" data-testid="mesh-add-agent-selected-role">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[10px] font-bold uppercase tracking-[0.2em] text-brand-500">Selected Role</p>
                <p class="mt-1 truncate text-sm font-semibold {t.textPrimary}">{selectedRuntimeRole.name}</p>
                <p class="mt-1 text-[10px] font-medium uppercase tracking-[0.12em] {t.textMuted}">
                  {toolLabel(selectedRuntimeRole.cliTool)} · {selectedRuntimeRole.model}
                </p>
              </div>
              <span class="rounded-full border px-2 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] {roleChipTone(selectedRuntimeRole)}">
                {selectedRuntimeRole.kind}
              </span>
            </div>
            {#if selectedRuntimeRole.summary}
              <p class="mt-2 text-[11px] leading-5 {t.textSecondary}">
                {selectedRuntimeRole.summary}
              </p>
            {/if}
          </div>
        {/if}
      </div>

      <div class="space-y-4">
        <div class="space-y-1.5">
          <label for="mesh-add-agent-name-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Agent Name</label>
          <input
            id="mesh-add-agent-name-input-field"
            class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {fieldTone}"
            placeholder="e.g. backend-dev"
            value={addAgentDraft?.name ?? ''}
            oninput={(event) => onUpdateAddAgentField('name', event.currentTarget.value)}
            disabled={addAgentDraft?.submitting}
            data-testid="mesh-add-agent-name-input"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <div class="flex items-center justify-between px-1">
              <label for="mesh-add-agent-tool-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted}">Tool</label>
              {#if addAgentDraft?.roleId}
                <button
                  type="button"
                  class="h-5 w-5 flex items-center justify-center rounded-md transition-all {addAgentDraft.isLocked ? 'text-brand-500 bg-brand-500/10' : 'text-zinc-400 hover:bg-black/5 dark:hover:bg-white/5'}"
                  onclick={onToggleAddAgentLock}
                  title={addAgentDraft.isLocked ? 'Unlock to edit' : 'Lock fields'}
                  data-testid="mesh-add-agent-unlock-toggle"
                >
                  {#if addAgentDraft.isLocked}
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                  {:else}
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></svg>
                  {/if}
                </button>
              {/if}
            </div>
            <div class="relative">
              <select
                id="mesh-add-agent-tool-select-input"
                class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
                value={addAgentDraft?.tool ?? 'codex'}
                onchange={(event) => onUpdateAddAgentField('tool', event.currentTarget.value)}
                disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
                data-testid="mesh-add-agent-tool-select"
              >
                <option value="claude">Claude</option>
                <option value="codex">Codex</option>
                <option value="gemini">Gemini</option>
              </select>
              <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
              </div>
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="mesh-add-agent-model-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Model</label>
            <ModelSelect
              id="mesh-add-agent-model-select-input"
              tool={addAgentDraft?.tool ?? 'codex'}
              model={addAgentDraft?.model}
              reasoningEffort={addAgentDraft?.reasoningEffort ?? null}
              inheritedEffort={roleDeclaredEffort(selectedRuntimeRole)}
              {catalog}
              {dark}
              disabled={Boolean(addAgentDraft?.submitting || addAgentDraft?.isLocked)}
              inputClass={`${fieldTone} ${selectScheme}`}
              testId="mesh-add-agent-model-select"
              onchange={(next) => {
                onUpdateAddAgentField('model', next.model)
                onUpdateAddAgentField('reasoningEffort', next.reasoningEffort)
              }}
            />
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="mesh-add-agent-project-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Project Binding</label>
          <div class="relative">
            <select
              id="mesh-add-agent-project-select-input"
              class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme}"
              value={addAgentDraft?.projectId ?? ''}
              onchange={(event) => onUpdateAddAgentField('projectId', event.currentTarget.value)}
              disabled={addAgentDraft?.submitting}
              data-testid="mesh-add-agent-project-select"
            >
              <option value="">Select project</option>
              {#each projectOptions as project}
                <option value={project.id}>{project.label}</option>
              {/each}
            </select>
            <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
            </div>
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="mesh-add-agent-description-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Description</label>
          <textarea
            id="mesh-add-agent-description-input-field"
            class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none {fieldTone} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
            rows="3"
            placeholder="Specific goals for this agent..."
            value={addAgentDraft?.description ?? ''}
            oninput={(event) => onUpdateAddAgentField('description', event.currentTarget.value)}
            disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
            data-testid="mesh-add-agent-description-input"
          ></textarea>
        </div>
      </div>

      {#if addAgentDraft?.error}
        <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-danger-500 text-center" data-testid="mesh-add-agent-error">{addAgentDraft.error}</p>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-3 pt-4 border-t {t.keyline}">
        <button
          class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
          type="button"
          onclick={onCloseAddAgent}
          disabled={addAgentDraft?.submitting}
          data-testid="mesh-add-agent-cancel"
        >
          Cancel
        </button>
        <button
          class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
          type="button"
          onclick={onSubmitAddAgent}
          disabled={!canSubmitAddAgent}
          data-testid="mesh-add-agent-submit"
        >
          {addAgentDraft?.submitting ? 'Adding...' : 'Add Agent'}
        </button>
      </div>
    </section>
  {/snippet}
</SlideOver>

<SlideOver
  open={Boolean(captureRoleDraft)}
  title="Capture as Role"
  width={360}
  {dark}
  onClose={onCloseCaptureRole}
>
  {#snippet children()}
    <section class="space-y-4 animate-in fade-in slide-in-from-bottom-1 duration-200" data-testid="mesh-capture-role-form">
      <p class="text-sm {t.textMuted} px-1">
        Save the selected runtime member as a reusable catalog role.
      </p>

      <div class="space-y-4">
        <div class="space-y-1.5">
          <label for="mesh-capture-role-name-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">New Role Name</label>
          <input
            id="mesh-capture-role-name-input-field"
            class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {dark ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20' : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'}"
            value={captureRoleDraft?.name ?? ''}
            oninput={(event) => onCaptureRoleName(event.currentTarget.value)}
            disabled={captureRoleDraft?.submitting}
            data-testid="mesh-capture-role-name-input"
          />
        </div>

        <div class="space-y-1.5">
          <label for="mesh-capture-role-id-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Role ID</label>
          <input
            id="mesh-capture-role-id-input-field"
            class="h-9 w-full rounded-lg border px-3 text-sm transition-all outline-none {dark ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20' : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'}"
            value={captureRoleDraft?.roleId ?? ''}
            oninput={(event) => onCaptureRoleId(event.currentTarget.value)}
            disabled={captureRoleDraft?.submitting}
            data-testid="mesh-capture-role-id-input"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <label for="mesh-capture-role-tool-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Tool</label>
            <input
              id="mesh-capture-role-tool-input-field"
              class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
              value={captureRoleDraft?.tool ?? ''}
              readonly
              disabled
              data-testid="mesh-capture-role-tool-input"
            />
          </div>
          <div class="space-y-1.5">
            <label for="mesh-capture-role-model-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Model</label>
            <input
              id="mesh-capture-role-model-input-field"
              class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
              value={captureRoleDraft?.model ?? ''}
              readonly
              disabled
              data-testid="mesh-capture-role-model-input"
            />
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="mesh-capture-role-description-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Current Description</label>
          <textarea
            id="mesh-capture-role-description-input-field"
            class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
            rows="3"
            value={captureRoleDraft?.description ?? ''}
            readonly
            disabled
            data-testid="mesh-capture-role-description-input"
          ></textarea>
        </div>
      </div>

      <div class="space-y-2 p-3 rounded-xl border {t.keyline} {dark ? 'bg-white/[0.02]' : 'bg-brand-50/30'}">
        <label class="group flex items-center gap-3 cursor-pointer">
          <div class="relative flex items-center justify-center">
            <input
              type="checkbox"
              checked={Boolean(captureRoleDraft?.includeInstructions)}
              onchange={() => onToggleCaptureRoleFlag('includeInstructions')}
              disabled={captureRoleDraft?.submitting}
              class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
              data-testid="mesh-capture-role-include-instructions"
            />
            <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          </div>
          <span class="text-xs font-medium {t.textSecondary} group-hover:text-brand-500 transition-colors">Include current instructions</span>
        </label>

        <label class="group flex items-center gap-3 cursor-pointer">
          <div class="relative flex items-center justify-center">
            <input
              type="checkbox"
              checked={Boolean(captureRoleDraft?.includeBehavioralContract)}
              onchange={() => onToggleCaptureRoleFlag('includeBehavioralContract')}
              disabled={captureRoleDraft?.submitting}
              class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
              data-testid="mesh-capture-role-include-behavioral-contract"
            />
            <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          </div>
          <span class="text-xs font-medium {t.textSecondary} group-hover:text-brand-500 transition-colors">Include behavioral contract</span>
        </label>
      </div>

      {#if captureRoleDraft?.error}
        <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-danger-500 text-center" data-testid="mesh-capture-role-error">{captureRoleDraft.error}</p>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-3 pt-4 border-t {t.keyline}">
        <button
          class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
          type="button"
          onclick={onCloseCaptureRole}
          disabled={captureRoleDraft?.submitting}
          data-testid="mesh-capture-role-cancel"
        >
          Cancel
        </button>
        <button
          class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
          type="button"
          onclick={onSubmitCaptureRole}
          disabled={!canSaveCapturedRole}
          data-testid="mesh-capture-role-save"
        >
          {captureRoleDraft?.submitting ? 'Saving...' : 'Save to Catalog'}
        </button>
      </div>
    </section>
  {/snippet}
</SlideOver>
