<script>
  import {
    coordinationAddAgent,
    coordinationDisbandTeam,
    coordinationListTeams,
    coordinationRemoveMember,
  } from '../ipc.js'
  import MeshSetupForm from './MeshSetupForm.svelte'
  import MeshInitProgress from './MeshInitProgress.svelte'
  import MeshAvailabilityGate from './MeshAvailabilityGate.svelte'
  import MeshTeamRoster from './MeshTeamRoster.svelte'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    onAddAgent: onAddAgentProp = () => {},
    onDisband: onDisbandProp = () => {},
    onRemoveAgent: onRemoveAgentProp = () => {},
    onFocusPane: onFocusPaneProp = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const selectScheme = $derived(dark ? '[color-scheme:dark]' : '[color-scheme:light]')
  const fieldTone = $derived(
    dark
      ? 'border-zinc-700/80 text-zinc-100 placeholder:text-zinc-600 focus:border-brand-500'
      : 'border-zinc-300 text-zinc-900 placeholder:text-zinc-400 focus:border-brand-500'
  )
  const neutralGhost = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/70'
      : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-200'
  )
  const formFieldBase =
    'w-full bg-transparent border-b rounded-none px-1 py-1.5 text-sm transition-colors focus:outline-none'
  const primaryCta = 'h-8 inline-flex items-center rounded-md bg-brand-600 px-3 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50'
  const chevronSvg = $derived(
    dark
      ? `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath d='M3 4l2 2 2-2' fill='none' stroke='%2371717a' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`
      : `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'%3E%3Cpath d='M3 4l2 2 2-2' fill='none' stroke='%2352525b' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`
  )
  const inlineSelect = $derived(
    dark
      ? `appearance-none bg-zinc-800/80 text-xs text-zinc-300 rounded px-1.5 py-1 pr-4 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none ${selectScheme} cursor-pointer`
      : `appearance-none bg-zinc-200/80 text-xs text-zinc-700 rounded px-1.5 py-1 pr-4 border-none focus:ring-1 focus:ring-brand-500 focus:outline-none ${selectScheme} cursor-pointer`
  )
  const cleanupBadgeCurrent = $derived(
    dark
      ? 'border border-success-500/40 bg-success-500/10 text-success-300'
      : 'border border-success-300 bg-success-100 text-success-700'
  )
  const cleanupBadgeOther = $derived(
    dark
      ? 'border border-zinc-600 bg-zinc-800 text-zinc-300'
      : 'border border-zinc-300 bg-zinc-100 text-zinc-600'
  )
  const cleanupDisbandTone = $derived(
    dark
      ? 'text-danger-300 hover:text-danger-200 hover:bg-danger-500/10'
      : 'text-danger-600 hover:text-danger-700 hover:bg-danger-50'
  )

  let mode = $state('setup')
  let teamName = $state('')
  let loading = $state(false)
  let errorMessage = $state('')
  let showInitProgress = $state(false)
  let initializeRequest = $state(null)
  let runtimeMessage = $state('')
  let showAddAgentForm = $state(false)
  let addAgentSubmitting = $state(false)
  let addAgentProgress = $state(null)
  let addAgentError = $state('')
  let addAgentName = $state('')
  let addAgentTool = $state('codex')
  let addAgentModel = $state('gpt-5.3-codex')
  let addAgentProjectId = $state('')
  let addAgentDescription = $state('')
  let rosterRefreshNonce = $state(0)
  let removingMembers = $state(new Set())
  let removeMemberPending = $state('')
  let showRemoveMemberConfirm = $state(false)
  let disbanding = $state(false)
  let showDisbandConfirm = $state(false)
  let discoveredTeams = $state([])
  let discoveryWarnings = $state([])
  let showCleanupPanel = $state(false)
  let cleanupError = $state('')
  let cleanupTargetTeam = $state('')
  let showCleanupConfirm = $state(false)
  let runtimeMessageTimer = null
  let errorMessageTimer = null

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3-codex', 'gpt-5-mini'],
    gemini: ['gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  function normalizeProjectOption(project) {
    if (typeof project === 'string') {
      return { id: project, label: project }
    }
    if (project && typeof project === 'object') {
      const id = project.path || project.id || project.name || ''
      const label = project.name || project.path || project.id || 'Unnamed project'
      return { id, label }
    }
    return { id: '', label: '' }
  }

  const projectOptions = $derived(
    (availableProjects ?? []).map(normalizeProjectOption).filter((project) => project.id)
  )

  const canSubmitAddAgent = $derived(
    !addAgentSubmitting &&
      addAgentName.trim().length > 0 &&
      addAgentTool.trim().length > 0 &&
      addAgentModel.trim().length > 0 &&
      addAgentProjectId.trim().length > 0
  )

  function normalizeTeamName(team) {
    return team?.teamName ?? team?.team_name ?? ''
  }

  function normalizeLeadPath(team) {
    return team?.leadProjectPath ?? team?.lead_project_path ?? null
  }

  function coerceTeams(response) {
    if (Array.isArray(response)) return response
    return response?.teams ?? []
  }

  function coerceWarnings(response) {
    if (Array.isArray(response)) return []
    return Array.isArray(response?.warnings) ? response.warnings : []
  }

  function normalizeLinuxPath(path) {
    let value = String(path || '').trim()
    if (!value) return ''
    value = value.replace(/\\/g, '/')
    value = value.replace(/\/+/g, '/')
    while (value.length > 1 && value.endsWith('/')) {
      value = value.slice(0, -1)
    }
    return value
  }

  function wslUncToLinux(path) {
    const normalized = String(path || '').trim().replace(/\//g, '\\')
    const lower = normalized.toLowerCase()
    let prefix = ''
    if (lower.startsWith('\\\\wsl$\\')) {
      prefix = '\\\\wsl$\\'
    } else if (lower.startsWith('\\\\wsl.localhost\\')) {
      prefix = '\\\\wsl.localhost\\'
    } else {
      return null
    }

    const remainder = normalized.slice(prefix.length)
    const firstSeparator = remainder.indexOf('\\')
    if (firstSeparator === -1) return null

    const afterDistro = remainder.slice(firstSeparator)
    if (!afterDistro || afterDistro === '\\') return '/'
    return normalizeLinuxPath(afterDistro)
  }

  function windowsDriveToLinux(path) {
    const match = String(path || '').trim().match(/^([a-zA-Z]):[\\/](.*)$/)
    if (!match) return null
    const [, drive, rest] = match
    return normalizeLinuxPath(`/mnt/${drive.toLowerCase()}/${rest}`)
  }

  function normalizeProjectPath(path) {
    const raw = String(path || '').trim()
    if (!raw) return ''
    return wslUncToLinux(raw) ?? windowsDriveToLinux(raw) ?? normalizeLinuxPath(raw)
  }

  function isSameProjectPath(left, right) {
    const leftNormalized = normalizeProjectPath(left)
    const rightNormalized = normalizeProjectPath(right)
    if (!leftNormalized || !rightNormalized) return false
    return leftNormalized === rightNormalized
  }

  function teamMatchesProject(team, currentProjectPath) {
    return isSameProjectPath(normalizeLeadPath(team), currentProjectPath)
  }

  function sanitizeTeamNameForTestId(name) {
    return String(name || '')
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, '-')
  }

  async function refreshTeamDiscovery(currentProjectPath, isCancelled = () => false) {
    const response = await coordinationListTeams()
    if (isCancelled()) return
    const teams = coerceTeams(response)
    discoveredTeams = teams
    discoveryWarnings = coerceWarnings(response)
    const matchingTeam = teams.find((team) => teamMatchesProject(team, currentProjectPath))
    if (isCancelled()) return
    if (matchingTeam) {
      teamName = normalizeTeamName(matchingTeam)
      mode = 'runtime'
    } else if (!showInitProgress) {
      teamName = ''
      mode = 'setup'
    }
  }

  function handleInitialize(request) {
    initializeRequest = request
    showInitProgress = true
  }

  function handleProgressBack() {
    showInitProgress = false
  }

  function handleProgressSuccess(result) {
    teamName = result?.teamName ?? initializeRequest?.teamName ?? teamName
    mode = 'runtime'
    showInitProgress = false
    runtimeMessage = result?.openedExisting
      ? 'Opened existing team.'
      : 'Team initialized successfully.'
  }

  function modelsForTool(tool) {
    return modelOptionsByTool[tool] ?? ['default']
  }

  function resetAddAgentForm() {
    addAgentName = ''
    addAgentTool = 'codex'
    addAgentModel = modelsForTool('codex')[0]
    addAgentProjectId = projectOptions[0]?.id ?? ''
    addAgentDescription = ''
    addAgentError = ''
  }

  function updateAddAgentTool(tool) {
    addAgentTool = tool
    addAgentModel = modelsForTool(tool)[0] ?? ''
  }

  function openAddAgentForm() {
    runtimeMessage = ''
    showAddAgentForm = true
    if (!addAgentProjectId) {
      addAgentProjectId = projectOptions[0]?.id ?? ''
    }
  }

  async function submitAddAgent() {
    if (!canSubmitAddAgent || !teamName) return
    addAgentSubmitting = true
    addAgentError = ''
    addAgentProgress = { status: 'running', report: null }
    try {
      const report = await coordinationAddAgent({
        teamName,
        agent: {
          name: addAgentName.trim(),
          cliTool: addAgentTool,
          model: addAgentModel,
          projectId: addAgentProjectId,
          description: addAgentDescription.trim() || null,
        },
      })
      addAgentProgress = { status: 'succeeded', report }
      onAddAgentProp(report)
      rosterRefreshNonce += 1
      runtimeMessage = `Agent '${report?.memberName ?? addAgentName.trim()}' added.`
      showAddAgentForm = false
      resetAddAgentForm()
    } catch (err) {
      const message = err?.message || 'Failed to add agent.'
      addAgentError = message
      addAgentProgress = { status: 'failed', report: null, message }
    } finally {
      addAgentSubmitting = false
    }
  }

  function handleRuntimeDisband() {
    if (!teamName || disbanding) return
    showDisbandConfirm = true
  }

  async function confirmRuntimeDisband() {
    if (!teamName || disbanding) return
    disbanding = true
    try {
      const result = await coordinationDisbandTeam(teamName)
      mode = 'setup'
      runtimeMessage = result?.alreadyDisbanded
        ? 'Team was already disbanded.'
        : 'Team disbanded and active sessions were stopped.'
      teamName = ''
      showInitProgress = false
      showAddAgentForm = false
      showDisbandConfirm = false
      showRemoveMemberConfirm = false
      removeMemberPending = ''
      initializeRequest = null
      addAgentProgress = null
      onDisbandProp(result)
      await refreshTeamDiscovery(projectPath)
    } catch (err) {
      errorMessage = err?.message || 'Failed to disband team.'
    } finally {
      disbanding = false
    }
  }

  function requestRemoveMember(memberName) {
    if (!teamName || !memberName || removingMembers.has(memberName) || disbanding) return
    removeMemberPending = memberName
    showRemoveMemberConfirm = true
  }

  async function confirmRuntimeRemoveMember() {
    if (!teamName || !removeMemberPending || removingMembers.has(removeMemberPending)) return
    const memberName = removeMemberPending
    removingMembers = new Set([...removingMembers, memberName])
    showRemoveMemberConfirm = false
    removeMemberPending = ''
    try {
      const report = await coordinationRemoveMember(teamName, memberName)
      const warningCount = Array.isArray(report?.warnings) ? report.warnings.length : 0
      runtimeMessage = warningCount > 0
        ? `Removed '${memberName}' with ${warningCount} warning${warningCount === 1 ? '' : 's'}.`
        : `Removed '${memberName}'.`
      onRemoveAgentProp(report)
      rosterRefreshNonce += 1
    } catch (err) {
      errorMessage = err?.message || `Failed to remove member '${memberName}'.`
    } finally {
      const next = new Set(removingMembers)
      next.delete(memberName)
      removingMembers = next
    }
  }

  function requestCleanupDisband(team) {
    cleanupError = ''
    cleanupTargetTeam = normalizeTeamName(team)
    showCleanupConfirm = Boolean(cleanupTargetTeam)
  }

  async function confirmCleanupDisband() {
    if (!cleanupTargetTeam) return
    try {
      await coordinationDisbandTeam(cleanupTargetTeam)
      runtimeMessage = `Team "${cleanupTargetTeam}" disbanded.`
      showCleanupConfirm = false
      cleanupTargetTeam = ''
      await refreshTeamDiscovery(projectPath)
    } catch (err) {
      cleanupError = err?.message || 'Failed to disband selected team.'
    }
  }

  $effect(() => {
    if (runtimeMessageTimer) clearTimeout(runtimeMessageTimer)
    if (!runtimeMessage) return
    runtimeMessageTimer = setTimeout(() => {
      runtimeMessage = ''
    }, 5000)
    return () => {
      if (runtimeMessageTimer) clearTimeout(runtimeMessageTimer)
    }
  })

  $effect(() => {
    if (errorMessageTimer) clearTimeout(errorMessageTimer)
    if (!errorMessage) return
    errorMessageTimer = setTimeout(() => {
      errorMessage = ''
    }, 8000)
    return () => {
      if (errorMessageTimer) clearTimeout(errorMessageTimer)
    }
  })

  $effect(() => {
    const currentProjectPath = projectPath
    let cancelled = false

    loading = true
    errorMessage = ''
    runtimeMessage = ''
    showAddAgentForm = false
    addAgentProgress = null
    addAgentError = ''
    disbanding = false
    showDisbandConfirm = false
    showCleanupPanel = false
    cleanupError = ''
    cleanupTargetTeam = ''
    showCleanupConfirm = false
    removeMemberPending = ''
    showRemoveMemberConfirm = false
    removingMembers = new Set()
    teamName = ''
    mode = 'setup'
    discoveredTeams = []
    discoveryWarnings = []

    refreshTeamDiscovery(currentProjectPath, () => cancelled)
      .catch((err) => {
        if (cancelled) return
        errorMessage = err?.message || 'Failed to load Mesh setup state'
      })
      .finally(() => {
        if (!cancelled) {
          loading = false
        }
      })

    return () => {
      cancelled = true
    }
  })
</script>

<section class="flex-1 min-h-0 overflow-y-auto {t.mainBg}" data-testid="mesh-tab">
  <div class="max-w-3xl px-7 pt-4 pb-6 space-y-4">
    {#if loading}
      <p class="text-sm {t.textMuted}" data-testid="mesh-loading">Checking Mesh team state...</p>
    {:else}
      {#if errorMessage}
        <div class="relative overflow-hidden border-l-2 border-danger-400 pl-3 pr-2 py-1 text-xs text-danger-600/95 flex items-center justify-between gap-2" data-testid="mesh-error">
          <span class="min-w-0">{errorMessage}</span>
          <button
            class="text-xs opacity-60 hover:opacity-100 ml-2"
            onclick={() => {
              errorMessage = ''
            }}
            data-testid="mesh-dismiss-error-message"
          >
            ✕
          </button>
          <div class="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-danger-400/50 animate-[shrink_8s_linear_forwards]" style="width: 100%"></div>
        </div>
      {/if}

      {#if runtimeMessage}
        <div class="relative overflow-hidden border-l-2 border-success-400 pl-3 pr-2 py-1 text-xs text-success-600/95 flex items-center justify-between gap-2" data-testid="mesh-runtime-message">
          <span class="min-w-0">{runtimeMessage}</span>
          <button
            class="text-xs opacity-60 hover:opacity-100 ml-2"
            onclick={() => {
              runtimeMessage = ''
            }}
            data-testid="mesh-dismiss-runtime-message"
          >
            ✕
          </button>
          <div class="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-success-400/50 animate-[shrink_5s_linear_forwards]" style="width: 100%"></div>
        </div>
      {/if}

      <MeshAvailabilityGate {dark} {projectPath}>
        {#snippet children(agentWarnings)}
          {#key mode}
            <div class="animate-[meshfade_180ms_ease-out]">
              {#if mode === 'runtime'}
                {#if showAddAgentForm}
                  <section class="pt-4 border-t {t.keyline} space-y-3" data-testid="mesh-add-agent-form">
                    <div class="flex items-center gap-2">
                      <span class="h-3 w-0.5 rounded-full bg-brand-500/80"></span>
                      <p class="text-[11px] uppercase {t.textMuted}">Add Agent</p>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
                      <input
                        class="{formFieldBase} {fieldTone}"
                        placeholder="Agent name"
                        bind:value={addAgentName}
                        data-testid="mesh-add-agent-name-input"
                      />
                      <select
                        class={inlineSelect}
                        style:background-image={chevronSvg}
                        style:background-repeat="no-repeat"
                        style:background-position="right 4px center"
                        value={addAgentTool}
                        onchange={(event) => updateAddAgentTool(event.currentTarget.value)}
                        data-testid="mesh-add-agent-tool-select"
                      >
                        <option value="claude">Claude</option>
                        <option value="codex">Codex</option>
                        <option value="gemini">Gemini</option>
                      </select>
                      <select
                        class={inlineSelect}
                        style:background-image={chevronSvg}
                        style:background-repeat="no-repeat"
                        style:background-position="right 4px center"
                        bind:value={addAgentModel}
                        data-testid="mesh-add-agent-model-select"
                      >
                        {#each modelsForTool(addAgentTool) as model}
                          <option value={model}>{model}</option>
                        {/each}
                      </select>
                      <select
                        class={inlineSelect}
                        style:background-image={chevronSvg}
                        style:background-repeat="no-repeat"
                        style:background-position="right 4px center"
                        bind:value={addAgentProjectId}
                        data-testid="mesh-add-agent-project-select"
                      >
                        <option value="">Select project</option>
                        {#each projectOptions as project}
                          <option value={project.id}>{project.label}</option>
                        {/each}
                      </select>
                    </div>

                    <input
                      class="{formFieldBase} {fieldTone}"
                      placeholder="Description (optional)"
                      bind:value={addAgentDescription}
                      data-testid="mesh-add-agent-description-input"
                    />

                    {#if addAgentError}
                      <p class="text-xs text-danger-500" data-testid="mesh-add-agent-error">{addAgentError}</p>
                    {/if}

                    <div class="flex justify-end gap-2">
                      <button
                        class="rounded-md px-2 py-1 text-[11px] transition-colors {neutralGhost}"
                        onclick={() => {
                          showAddAgentForm = false
                          addAgentError = ''
                        }}
                        disabled={addAgentSubmitting}
                        data-testid="mesh-add-agent-cancel"
                      >
                        Cancel
                      </button>
                      <button
                        class={primaryCta}
                        onclick={submitAddAgent}
                        disabled={!canSubmitAddAgent}
                        data-testid="mesh-add-agent-submit"
                      >
                        <span class={addAgentSubmitting ? 'animate-pulse' : ''}>
                          {addAgentSubmitting ? 'Adding...' : 'Add Agent'}
                        </span>
                      </button>
                    </div>
                  </section>
                {/if}

                {#if addAgentProgress}
                  <section class="pt-2 border-t {t.keyline} space-y-1.5" data-testid="mesh-add-agent-progress">
                    <p class="text-[11px] uppercase {t.textMuted}">
                      Adding agent... {addAgentProgress.status}
                    </p>
                    {#if addAgentProgress.report?.steps?.length}
                      <ul class="space-y-1">
                        {#each addAgentProgress.report.steps as progress}
                          <li class="text-xs {t.textMuted}" data-testid={`mesh-add-agent-step-${progress.step}`}>
                            {progress.step}: {progress.status}
                          </li>
                        {/each}
                      </ul>
                    {/if}
                  </section>
                {/if}

                <MeshTeamRoster
                  {dark}
                  {teamName}
                  refreshNonce={rosterRefreshNonce}
                  {disbanding}
                  removingMembers={[...removingMembers]}
                  onAddAgent={openAddAgentForm}
                  onDisband={handleRuntimeDisband}
                  onRemoveAgent={requestRemoveMember}
                  onFocusPane={onFocusPaneProp}
                />
              {:else}
                {#if showInitProgress}
                  <MeshInitProgress
                    {dark}
                    request={initializeRequest}
                    onsuccess={handleProgressSuccess}
                    onback={handleProgressBack}
                  />
                {:else}
                  <div class="space-y-3">
                    <MeshSetupForm
                      {dark}
                      {projectPath}
                      {availableProjects}
                      preflightWarnings={agentWarnings}
                      oninitialize={handleInitialize}
                    />

                    {#if discoveredTeams.length > 0 || discoveryWarnings.length > 0}
                      <section
                        class="rounded-lg border {t.keyline} p-3 space-y-2"
                        data-testid="mesh-cleanup-panel"
                      >
                        <header class="flex items-center justify-between gap-3">
                          <div class="min-w-0">
                            <p class="text-[11px] uppercase tracking-[0.14em] {t.textMuted}">
                              Team Cleanup
                            </p>
                            <p class="text-xs {t.textSecondary}">
                              Review existing teams before starting a new one.
                            </p>
                          </div>
                          <button
                            class="rounded-md px-2 py-1 text-[11px] transition-colors {neutralGhost}"
                            onclick={() => {
                              showCleanupPanel = !showCleanupPanel
                            }}
                            data-testid="mesh-cleanup-toggle"
                          >
                            {showCleanupPanel ? 'Hide' : 'Show'}{discoveredTeams.length > 0 ? ` (${discoveredTeams.length})` : ''}
                          </button>
                        </header>

                        {#if showCleanupPanel}
                          {#if cleanupError}
                            <p class="text-xs text-danger-500" data-testid="mesh-cleanup-error">
                              {cleanupError}
                            </p>
                          {/if}

                          {#if discoveryWarnings.length > 0}
                            <div class="rounded-md border border-warning-400/30 bg-warning-400/10 px-2 py-1.5" data-testid="mesh-cleanup-warnings">
                              <p class="text-[11px] font-medium text-warning-500">Discovery warnings</p>
                              <ul class="mt-1 space-y-1">
                                {#each discoveryWarnings as warning}
                                  <li class="text-[11px] {t.textMuted}">{warning}</li>
                                {/each}
                              </ul>
                            </div>
                          {/if}

                          {#if discoveredTeams.length === 0}
                            <p class="text-xs {t.textMuted}">
                              No valid teams found.
                            </p>
                          {:else}
                            <div class="space-y-1.5" data-testid="mesh-cleanup-team-list">
                              {#each discoveredTeams as team}
                                {@const currentTeamName = normalizeTeamName(team)}
                                {@const leadPath = normalizeLeadPath(team)}
                                {@const isCurrentProject = teamMatchesProject(team, projectPath)}
                                <article
                                  class="rounded-md border {t.keyline} px-2.5 py-2 flex items-start justify-between gap-2"
                                  data-testid={`mesh-cleanup-team-${sanitizeTeamNameForTestId(currentTeamName)}`}
                                >
                                  <div class="min-w-0 space-y-0.5">
                                    <p class="text-xs font-medium {t.textPrimary}">
                                      {currentTeamName}
                                    </p>
                                    <p class="text-[11px] truncate {t.textMuted}">
                                      {leadPath || 'No lead project path recorded'}
                                    </p>
                                    <span
                                      class={`inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] ${isCurrentProject ? cleanupBadgeCurrent : cleanupBadgeOther}`}
                                    >
                                      {isCurrentProject ? 'Current project' : 'Different project'}
                                    </span>
                                  </div>
                                  <button
                                    class="rounded-md px-2 py-1 text-[11px] transition-colors {cleanupDisbandTone}"
                                    onclick={() => requestCleanupDisband(team)}
                                    data-testid={`mesh-cleanup-disband-${sanitizeTeamNameForTestId(currentTeamName)}`}
                                  >
                                    Disband
                                  </button>
                                </article>
                              {/each}
                            </div>
                          {/if}
                        {/if}
                      </section>
                    {/if}
                  </div>
                {/if}
              {/if}
            </div>
          {/key}
        {/snippet}
      </MeshAvailabilityGate>

      {#if showDisbandConfirm}
        <ConfirmDialog
          {dark}
          bind:open={showDisbandConfirm}
          title="Disband team?"
          message={`Disband team "${teamName}"? This will remove mesh state and stop active agent sessions (panes, daemons, and mesh membership).`}
          confirmLabel="Disband"
          variant="danger"
          onconfirm={confirmRuntimeDisband}
        />
      {/if}

      {#if showRemoveMemberConfirm}
        <ConfirmDialog
          {dark}
          bind:open={showRemoveMemberConfirm}
          title="Remove agent?"
          message={`Remove agent "${removeMemberPending}" from "${teamName}"? This stops managed resources (mesh presence, daemon, and pane) when possible.`}
          confirmLabel="Remove"
          variant="danger"
          onconfirm={confirmRuntimeRemoveMember}
          oncancel={() => {
            removeMemberPending = ''
          }}
        />
      {/if}

      {#if showCleanupConfirm}
        <ConfirmDialog
          {dark}
          bind:open={showCleanupConfirm}
          title="Disband selected team?"
          message={`Disband team "${cleanupTargetTeam}"? This removes mesh state and stops active sessions tied to it.`}
          confirmLabel="Disband"
          variant="danger"
          onconfirm={confirmCleanupDisband}
        />
      {/if}
    {/if}
  </div>
</section>

<style>
  @keyframes shrink {
    from {
      width: 100%;
    }
    to {
      width: 0%;
    }
  }

  @keyframes meshfade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>
