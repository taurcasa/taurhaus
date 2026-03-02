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

  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    onAddAgent: onAddAgentProp = () => {},
    onDisband: onDisbandProp = () => {},
    onFocusPane: onFocusPaneProp = () => {},
  } = $props()

  const panelBg = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const keyline = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const inputBg = $derived(
    dark
      ? 'bg-zinc-900/70 border-zinc-800 text-zinc-100 placeholder:text-zinc-600'
      : 'bg-white border-zinc-300 text-zinc-900 placeholder:text-zinc-400'
  )
  const subtleButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:border-zinc-600 hover:text-zinc-200'
      : 'border-zinc-300 text-zinc-700 hover:border-zinc-400 hover:text-zinc-900'
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
  let addAgentModel = $state('gpt-5.3')
  let addAgentProjectId = $state('')
  let addAgentDescription = $state('')
  let rosterRefreshNonce = $state(0)

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
      const id = project.id || project.path || project.name || ''
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
    if (!teamName) return
    const confirmed = window.confirm(
      `Disband team "${teamName}"? This only removes mesh state and preserves active sessions.`
    )
    if (!confirmed) return
    try {
      const result = await coordinationDisbandTeam(teamName)
      mode = 'setup'
      runtimeMessage = result?.alreadyDisbanded
        ? 'Team was already disbanded.'
        : 'Team disbanded. Existing sessions remain running.'
      teamName = ''
      showInitProgress = false
      showAddAgentForm = false
      initializeRequest = null
      addAgentProgress = null
      onDisbandProp(result)
    } catch (err) {
      errorMessage = err?.message || 'Failed to disband team.'
    }
  }

  $effect(() => {
    const currentProjectPath = projectPath
    let cancelled = false

    loading = true
    errorMessage = ''
    runtimeMessage = ''
    showAddAgentForm = false
    addAgentProgress = null
    addAgentError = ''
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

<section class="flex-1 min-h-0 overflow-y-auto {panelBg}" data-testid="mesh-tab">
  <div class="max-w-4xl px-5 pt-4 pb-6 space-y-4">
    {#if loading}
      <p class="text-sm {textMuted}" data-testid="mesh-loading">Checking Mesh team state...</p>
    {:else}
      {#if errorMessage}
        <div class="border-l-2 border-danger-400 pl-3 py-1 text-xs text-danger-600" data-testid="mesh-error">
          {errorMessage}
        </div>
      {/if}

      {#if runtimeMessage}
        <div class="border-l-2 border-success-400 pl-3 py-1 text-xs text-success-600" data-testid="mesh-runtime-message">
          {runtimeMessage}
        </div>
      {/if}

      <MeshAvailabilityGate {dark} {projectPath}>
        {#snippet children(agentWarnings)}
          {#if mode === 'runtime'}
            {#if showAddAgentForm}
              <section class="pt-2 border-t {keyline} space-y-3" data-testid="mesh-add-agent-form">
                <p class="text-[11px] font-semibold uppercase tracking-[0.06em] {textMuted}">Add Agent</p>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
                  <input
                    class="rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                    placeholder="Agent name"
                    bind:value={addAgentName}
                    data-testid="mesh-add-agent-name-input"
                  />
                  <select
                    class="rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                    value={addAgentTool}
                    onchange={(event) => updateAddAgentTool(event.currentTarget.value)}
                    data-testid="mesh-add-agent-tool-select"
                  >
                    <option value="claude">Claude</option>
                    <option value="codex">Codex</option>
                    <option value="gemini">Gemini</option>
                  </select>
                  <select
                    class="rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                    bind:value={addAgentModel}
                    data-testid="mesh-add-agent-model-select"
                  >
                    {#each modelsForTool(addAgentTool) as model}
                      <option value={model}>{model}</option>
                    {/each}
                  </select>
                  <select
                    class="rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
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
                  class="w-full rounded-md border px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
                  placeholder="Description (optional)"
                  bind:value={addAgentDescription}
                  data-testid="mesh-add-agent-description-input"
                />

                {#if addAgentError}
                  <p class="text-xs text-danger-500" data-testid="mesh-add-agent-error">{addAgentError}</p>
                {/if}

                <div class="flex justify-end gap-2">
                  <button
                    class="rounded-md border px-3 py-1.5 text-xs {subtleButton}"
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
                    class="rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
                    onclick={submitAddAgent}
                    disabled={!canSubmitAddAgent}
                    data-testid="mesh-add-agent-submit"
                  >
                    Add Agent
                  </button>
                </div>
              </section>
            {/if}

            {#if addAgentProgress}
              <section class="pt-2 border-t {keyline} space-y-1.5" data-testid="mesh-add-agent-progress">
                <p class="text-[11px] font-semibold uppercase tracking-[0.06em] {textMuted}">
                  Add Agent Progress: {addAgentProgress.status}
                </p>
                {#if addAgentProgress.report?.steps?.length}
                  <ul class="space-y-1">
                    {#each addAgentProgress.report.steps as progress}
                      <li class="text-xs {textMuted}" data-testid={`mesh-add-agent-step-${progress.step}`}>
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
        {/snippet}
      </MeshAvailabilityGate>
    {/if}
  </div>
</section>
