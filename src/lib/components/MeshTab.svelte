<script>
  import {
    coordinationAddAgent,
    coordinationDisbandTeam,
    coordinationListTeams,
  } from '../ipc.js'
  import MeshSetupForm from './MeshSetupForm.svelte'
  import MeshInitProgress from './MeshInitProgress.svelte'
  import MeshAvailabilityGate from './MeshAvailabilityGate.svelte'
  import MeshTeamRoster from './MeshTeamRoster.svelte'
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    onAddAgent: onAddAgentProp = () => {},
    onDisband: onDisbandProp = () => {},
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
  let addAgentModel = $state('gpt-5.3')
  let addAgentProjectId = $state('')
  let addAgentDescription = $state('')
  let rosterRefreshNonce = $state(0)
  let disbanding = $state(false)
  let runtimeMessageTimer = null
  let errorMessageTimer = null

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3', 'gpt-5-mini'],
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
    runtimeMessage = 'Team initialized successfully.'
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

  async function handleRuntimeDisband() {
    if (!teamName || disbanding) return
    const confirmed = window.confirm(
      `Disband team "${teamName}"? This will remove mesh state and stop active agent sessions (panes, daemons, and mesh membership).`
    )
    if (!confirmed) return
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
      initializeRequest = null
      addAgentProgress = null
      onDisbandProp(result)
    } catch (err) {
      errorMessage = err?.message || 'Failed to disband team.'
    } finally {
      disbanding = false
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
    teamName = ''
    mode = 'setup'

    coordinationListTeams()
      .then((response) => {
        if (cancelled) return
        const teams = coerceTeams(response)
        const matchingTeam = teams.find((team) => normalizeLeadPath(team) === currentProjectPath)
        if (matchingTeam) {
          teamName = normalizeTeamName(matchingTeam)
          mode = 'runtime'
        }
      })
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
                        class="{formFieldBase} {fieldTone} {selectScheme}"
                        value={addAgentTool}
                        onchange={(event) => updateAddAgentTool(event.currentTarget.value)}
                        data-testid="mesh-add-agent-tool-select"
                      >
                        <option value="claude">Claude</option>
                        <option value="codex">Codex</option>
                        <option value="gemini">Gemini</option>
                      </select>
                      <select
                        class="{formFieldBase} {fieldTone} {selectScheme}"
                        bind:value={addAgentModel}
                        data-testid="mesh-add-agent-model-select"
                      >
                        {#each modelsForTool(addAgentTool) as model}
                          <option value={model}>{model}</option>
                        {/each}
                      </select>
                      <select
                        class="{formFieldBase} {fieldTone} {selectScheme}"
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
                  onAddAgent={openAddAgentForm}
                  onDisband={handleRuntimeDisband}
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
                  <MeshSetupForm
                    {dark}
                    {projectPath}
                    {availableProjects}
                    preflightWarnings={agentWarnings}
                    oninitialize={handleInitialize}
                  />
                {/if}
              {/if}
            </div>
          {/key}
        {/snippet}
      </MeshAvailabilityGate>
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
