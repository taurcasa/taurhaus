<script>
  import {
    coordinationAddMember,
    coordinationCreateTeam,
    coordinationDisbandTeam,
    coordinationGetTeamStatus,
    coordinationListTeams,
    coordinationRemoveMember,
  } from '../ipc.js'

  let { dark = false } = $props()

  const panelBg = $derived(dark ? 'bg-zinc-900 border-zinc-800' : 'bg-zinc-50 border-zinc-200')
  const cardBg = $derived(dark ? 'bg-zinc-950 border-zinc-800' : 'bg-white border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textMuted = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const inputBg = $derived(
    dark ? 'bg-zinc-900 border-zinc-700 text-zinc-100' : 'bg-white border-zinc-300 text-zinc-900'
  )

  let teams = $state([])
  let selectedTeam = $state('')
  let selectedStatus = $state(null)
  let createTeamName = $state('')
  let memberName = $state('')
  let backendKind = $state('mesh_bridged')
  let loadingTeams = $state(false)
  let loadingStatus = $state(false)
  let submitting = $state(false)
  let errorMessage = $state('')
  let didInit = $state(false)

  const selectedMembers = $derived(selectedStatus?.members ?? [])
  const canCreate = $derived(!submitting && createTeamName.trim().length > 0)
  const canAddMember = $derived(!submitting && selectedTeam.length > 0 && memberName.trim().length > 0)
  const createDisabledClass = $derived(canCreate ? '' : 'opacity-50 cursor-not-allowed')
  const addDisabledClass = $derived(canAddMember ? '' : 'opacity-50 cursor-not-allowed')

  $effect(() => {
    if (didInit) return
    didInit = true
    void refreshTeams()
  })

  function normalizeTeamName(team) {
    return team?.teamName ?? team?.team_name ?? ''
  }

  function formatError(err, fallback) {
    const message = err?.message || `${err || fallback}`
    return message.trim().length > 0 ? message : fallback
  }

  async function refreshTeams() {
    loadingTeams = true
    errorMessage = ''
    try {
      teams = await coordinationListTeams()
      if (selectedTeam && !teams.some((team) => normalizeTeamName(team) === selectedTeam)) {
        selectedTeam = ''
        selectedStatus = null
      }
      if (selectedTeam) {
        await loadTeamStatus(selectedTeam)
      }
    } catch (err) {
      errorMessage = `Failed to list teams: ${formatError(err, 'unknown error')}`
    } finally {
      loadingTeams = false
    }
  }

  async function loadTeamStatus(teamName) {
    loadingStatus = true
    errorMessage = ''
    try {
      selectedStatus = await coordinationGetTeamStatus(teamName)
    } catch (err) {
      selectedStatus = null
      errorMessage = `Failed to load team status: ${formatError(err, 'unknown error')}`
    } finally {
      loadingStatus = false
    }
  }

  async function handleCreateTeam() {
    if (!canCreate) return
    submitting = true
    errorMessage = ''
    const teamName = createTeamName.trim()
    try {
      await coordinationCreateTeam(teamName)
      createTeamName = ''
      await refreshTeams()
      selectedTeam = teamName
      await loadTeamStatus(teamName)
    } catch (err) {
      errorMessage = `Failed to create team: ${formatError(err, 'unknown error')}`
    } finally {
      submitting = false
    }
  }

  async function handleDisbandTeam() {
    if (!selectedTeam || submitting) return
    if (!confirm(`Disband team "${selectedTeam}"? This cannot be undone.`)) return
    submitting = true
    errorMessage = ''
    try {
      await coordinationDisbandTeam(selectedTeam)
      selectedTeam = ''
      selectedStatus = null
      await refreshTeams()
    } catch (err) {
      errorMessage = `Failed to disband team: ${formatError(err, 'unknown error')}`
    } finally {
      submitting = false
    }
  }

  async function handleAddMember() {
    if (!canAddMember) return
    submitting = true
    errorMessage = ''
    try {
      await coordinationAddMember(selectedTeam, memberName.trim(), backendKind)
      memberName = ''
      await loadTeamStatus(selectedTeam)
    } catch (err) {
      errorMessage = `Failed to add member: ${formatError(err, 'unknown error')}`
    } finally {
      submitting = false
    }
  }

  async function handleRemoveMember(name) {
    if (!selectedTeam || submitting) return
    if (!confirm(`Remove member "${name}" from "${selectedTeam}"?`)) return
    submitting = true
    errorMessage = ''
    try {
      await coordinationRemoveMember(selectedTeam, name)
      await loadTeamStatus(selectedTeam)
    } catch (err) {
      errorMessage = `Failed to remove member: ${formatError(err, 'unknown error')}`
    } finally {
      submitting = false
    }
  }

  async function handleSelectTeam(teamName) {
    if (!teamName) return
    selectedTeam = teamName
    await loadTeamStatus(teamName)
  }
</script>

<section class="rounded-xl border p-4 space-y-4 {panelBg}" data-testid="coordination-panel">
  <div class="flex items-center justify-between">
    <h2 class="text-sm font-semibold tracking-wide uppercase {textPrimary}">Coordination</h2>
    <button
      class="text-xs px-2 py-1 rounded border {cardBg} {textMuted} hover:text-brand-600"
      onclick={refreshTeams}
      disabled={loadingTeams || submitting}
      data-testid="coordination-refresh"
    >
      Refresh
    </button>
  </div>

  {#if errorMessage}
    <div
      class="rounded-md border px-3 py-2 text-xs bg-danger-50 text-danger-600 border-danger-400/40"
      data-testid="coordination-error"
    >
      {errorMessage}
    </div>
  {/if}

  <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
    <div class="rounded-lg border p-3 space-y-2 {cardBg}">
      <h3 class="text-xs font-semibold uppercase tracking-wide {textMuted}">Teams</h3>
      {#if loadingTeams}
        <p class="text-xs {textMuted}" data-testid="coordination-teams-loading">Loading teams...</p>
      {:else if teams.length === 0}
        <p class="text-xs {textMuted}" data-testid="coordination-teams-empty">No teams found.</p>
      {:else}
        <ul class="space-y-1" data-testid="coordination-team-list">
          {#each teams as team}
            {@const teamName = normalizeTeamName(team)}
            <li>
              <button
                class="w-full text-left text-xs px-2.5 py-2 rounded-md border transition-colors
                  {selectedTeam === teamName ? 'border-brand-500 bg-brand-50 text-brand-700' : `${cardBg} ${textPrimary}`}"
                onclick={() => handleSelectTeam(teamName)}
                data-testid={`coordination-team-${teamName}`}
              >
                {teamName}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="rounded-lg border p-3 space-y-2 {cardBg}">
      <h3 class="text-xs font-semibold uppercase tracking-wide {textMuted}">Create Team</h3>
      <div class="flex gap-2">
        <input
          class="flex-1 rounded-md border px-2.5 py-2 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
          placeholder="architecture-final"
          bind:value={createTeamName}
          data-testid="coordination-create-team-input"
        />
        <button
          class="rounded-md bg-brand-600 px-3 py-2 text-xs font-medium text-white hover:bg-brand-700 {createDisabledClass}"
          onclick={handleCreateTeam}
          disabled={!canCreate}
          data-testid="coordination-create-team-button"
        >
          Create
        </button>
      </div>
    </div>
  </div>

  <div class="rounded-lg border p-3 space-y-3 {cardBg}">
    <div class="flex items-center justify-between">
      <h3 class="text-xs font-semibold uppercase tracking-wide {textMuted}">
        {selectedTeam ? `Team: ${selectedTeam}` : 'Team Status'}
      </h3>
      <button
        class="text-xs px-2 py-1 rounded bg-danger-500 text-white hover:bg-danger-600 disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={handleDisbandTeam}
        disabled={!selectedTeam || submitting}
        data-testid="coordination-disband-team-button"
      >
        Disband
      </button>
    </div>

    {#if !selectedTeam}
      <p class="text-xs {textMuted}" data-testid="coordination-no-team-selected">Select a team to manage members.</p>
    {:else}
      <div class="space-y-2">
        {#if loadingStatus}
          <p class="text-xs {textMuted}" data-testid="coordination-status-loading">Loading status...</p>
        {:else if selectedMembers.length === 0}
          <p class="text-xs {textMuted}" data-testid="coordination-members-empty">No members yet.</p>
        {:else}
          <ul class="space-y-1.5" data-testid="coordination-member-list">
            {#each selectedMembers as name}
              <li class="flex items-center justify-between rounded-md border px-2.5 py-2 {panelBg}">
                <span class="text-xs {textPrimary}">{name}</span>
                <button
                  class="text-xs rounded bg-danger-500 text-white px-2 py-1 hover:bg-danger-600"
                  onclick={() => handleRemoveMember(name)}
                  data-testid={`coordination-remove-member-${name}`}
                >
                  Remove
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="grid grid-cols-1 md:grid-cols-[1fr_auto_auto] gap-2">
        <input
          class="rounded-md border px-2.5 py-2 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
          placeholder="codex-reviewer"
          bind:value={memberName}
          data-testid="coordination-add-member-input"
        />
        <select
          class="rounded-md border px-2.5 py-2 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500 {inputBg}"
          bind:value={backendKind}
          data-testid="coordination-backend-kind-select"
        >
          <option value="mesh_bridged">mesh_bridged</option>
          <option value="claude_native">claude_native</option>
        </select>
        <button
          class="rounded-md bg-brand-600 px-3 py-2 text-xs font-medium text-white hover:bg-brand-700 {addDisabledClass}"
          onclick={handleAddMember}
          disabled={!canAddMember}
          data-testid="coordination-add-member-button"
        >
          Add Member
        </button>
      </div>
    {/if}
  </div>
</section>
