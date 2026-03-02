<script>
  import { rowTintClass } from '../sessionIndicator.js'

  let {
    teamName = '',
    dark = false,
    onAddAgent = () => {},
    onDisband = () => {},
    onFocusPane = () => {},
    refreshNonce = 0,
  } = $props()

  const keyline = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const subtleButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:border-zinc-600 hover:text-zinc-200'
      : 'border-zinc-300 text-zinc-700 hover:border-zinc-400 hover:text-zinc-900'
  )

  let members = $state([])
  let loading = $state(false)
  let errorMessage = $state('')
  let reonboarding = $state(new Set())

  function statusToState(status) {
    const normalized = String(status || '').toLowerCase()
    if (normalized === 'active') return 'active'
    if (normalized === 'idle') return 'idle'
    return 'offline'
  }

  function normalizeMember(member) {
    return {
      name: member?.name ?? '',
      role: member?.role ?? 'member',
      cliTool: member?.cliTool ?? member?.cli_tool ?? '',
      model: member?.model ?? '',
      projectId: member?.projectId ?? member?.project_id ?? '',
      description: member?.description ?? null,
      sessionStatus: String(member?.sessionStatus ?? member?.session_status ?? 'offline').toLowerCase(),
      paneId: member?.paneId ?? member?.pane_id ?? null,
    }
  }

  function statusGlyph(status) {
    return statusToState(status) === 'offline' ? '○' : '●'
  }

  function statusLabel(status) {
    const state = statusToState(status)
    if (state === 'active') return 'Active'
    if (state === 'idle') return 'Idle'
    return 'Offline'
  }

  function statusClass(status) {
    const state = statusToState(status)
    if (state === 'active') return 'text-success-500'
    if (state === 'idle') return 'text-warning-500'
    return dark ? 'text-zinc-500' : 'text-zinc-400'
  }

  function roleIndicator(role) {
    return role === 'lead' ? '★' : '◦'
  }

  function roleLabel(role) {
    return role === 'lead' ? 'Lead' : 'Member'
  }

  async function refreshRoster() {
    if (!teamName) return
    loading = true
    errorMessage = ''
    try {
      const ipc = await import('../ipc.js')
      const getLiveStatus = ipc?.coordinationGetLiveTeamStatus
      if (typeof getLiveStatus !== 'function') {
        members = []
        return
      }
      const status = await getLiveStatus(teamName)
      members = (status?.members ?? []).map(normalizeMember)
    } catch (err) {
      errorMessage = err?.message || 'Failed to load team roster'
      members = []
    } finally {
      loading = false
    }
  }

  async function handleReonboard(memberName) {
    const ipc = await import('../ipc.js')
    const reonboard = ipc?.coordinationReonboard
    if (typeof reonboard !== 'function') return
    reonboarding = new Set([...reonboarding, memberName])
    try {
      await reonboard(teamName, memberName)
    } finally {
      const next = new Set(reonboarding)
      next.delete(memberName)
      reonboarding = next
    }
  }

  $effect(() => {
    if (!teamName) return
    const nonce = refreshNonce
    let cancelled = false
    let intervalId = null

    const tick = async () => {
      if (cancelled) return
      await refreshRoster()
    }

    void tick()
    intervalId = setInterval(() => {
      void tick()
    }, 5000)

    return () => {
      cancelled = true
      if (intervalId) clearInterval(intervalId)
      void nonce
    }
  })
</script>

<section class="space-y-3" data-testid="mesh-team-roster">
  <header class="flex items-start justify-between gap-3 pb-2 border-b {keyline}">
    <div>
      <h2 class="text-base font-semibold {textPrimary}" data-testid="mesh-runtime-title">Team: {teamName}</h2>
      <p class="text-xs {textMuted}" data-testid="mesh-runtime-placeholder">Live roster refreshes every 5 seconds.</p>
    </div>
    <div class="flex items-center gap-2">
      <button
        class="rounded-md border px-2.5 py-1 text-xs {subtleButton}"
        onclick={onAddAgent}
        data-testid="mesh-add-agent-button"
      >
        + Add Agent
      </button>
      <button
        class="rounded-md border border-danger-400/50 px-2.5 py-1 text-xs text-danger-500 hover:border-danger-500 hover:text-danger-600"
        onclick={onDisband}
        data-testid="mesh-disband-button"
      >
        Disband
      </button>
    </div>
  </header>

  {#if errorMessage}
    <div class="border-l-2 border-danger-400 pl-3 py-1 text-xs text-danger-600" data-testid="mesh-roster-error">
      {errorMessage}
    </div>
  {/if}

  {#if loading && members.length === 0}
    <p class="text-xs {textMuted}" data-testid="mesh-roster-loading">Loading roster...</p>
  {:else if members.length === 0}
    <p class="text-xs {textMuted}" data-testid="mesh-roster-empty">No members found.</p>
  {:else}
    <div class="divide-y {keyline} border-y {keyline}">
      {#each members as member}
        <article
          class={`py-2 px-1 ${rowTintClass({ state: statusToState(member.sessionStatus) })}`}
          data-testid={`mesh-roster-card-${member.name}`}
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 space-y-1">
              <p class="text-sm font-medium truncate {textPrimary}">
                <span data-testid={`mesh-role-indicator-${member.name}`}>{roleIndicator(member.role)}</span>
                {' '}
                <span>{member.name}</span>
                <span class="ml-2 text-[11px] font-normal {textMuted}">{roleLabel(member.role)}</span>
              </p>

              <p class="text-xs {textMuted}" data-testid={`mesh-member-meta-${member.name}`}>
                {member.cliTool || 'Unknown'} · {member.model || 'default'} · {member.projectId || 'project'}
              </p>

              {#if member.description}
                <p class="text-xs {textMuted} truncate">{member.description}</p>
              {/if}
            </div>

            <div class="shrink-0 text-right space-y-1">
              <p class={`text-xs font-semibold ${statusClass(member.sessionStatus)}`} data-testid={`mesh-status-dot-${member.name}`}>
                {statusGlyph(member.sessionStatus)} {statusLabel(member.sessionStatus)}
              </p>

              <div class="flex items-center justify-end gap-1.5">
                <button
                  class="rounded-md border border-brand-500/50 px-2 py-1 text-[11px] text-brand-500 hover:border-brand-500 hover:text-brand-400 disabled:cursor-not-allowed disabled:opacity-50"
                  onclick={() => onFocusPane(member.paneId)}
                  disabled={!member.paneId}
                  data-testid={`mesh-focus-pane-${member.name}`}
                >
                  Focus Pane
                </button>
                {#if member.role !== 'lead'}
                  <button
                    class="rounded-md border px-2 py-1 text-[11px] {subtleButton} disabled:cursor-not-allowed disabled:opacity-50"
                    onclick={() => handleReonboard(member.name)}
                    disabled={reonboarding.has(member.name)}
                    data-testid={`mesh-reonboard-${member.name}`}
                  >
                    Re-onboard
                  </button>
                {/if}
              </div>
            </div>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>
