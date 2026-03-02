<script>
  import { themeTokens } from '../themeTokens.js'

  let {
    teamName = '',
    dark = false,
    onAddAgent = () => {},
    onDisband = () => {},
    onFocusPane = () => {},
    disbanding = false,
    refreshNonce = 0,
  } = $props()

  const t = $derived(themeTokens(dark))
  const actionBase = 'rounded-md px-2 py-1 text-[11px] transition-colors'
  const actionBrand = `${actionBase} text-brand-500 hover:text-brand-400 hover:bg-brand-500/10`
  const actionDanger = `${actionBase} text-danger-500/70 hover:text-danger-500 hover:bg-danger-500/10 disabled:opacity-60 disabled:cursor-not-allowed`
  const rowActionTone = $derived(
    dark
      ? 'rounded px-1.5 py-0.5 text-[10px] border border-zinc-700 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/70 hover:border-zinc-500 disabled:opacity-50'
      : 'rounded px-1.5 py-0.5 text-[10px] border border-zinc-300 text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100 hover:border-zinc-400 disabled:opacity-50'
  )

  let members = $state([])
  let loading = $state(false)
  let errorMessage = $state('')
  let reonboarding = $state(new Set())
  const activeCount = $derived.by(
    () => members.filter((member) => statusToState(member.sessionStatus) === 'active').length
  )
  const idleCount = $derived.by(
    () => members.filter((member) => statusToState(member.sessionStatus) === 'idle').length
  )

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

  function statusLabel(status) {
    const state = statusToState(status)
    if (state === 'active') return 'Active'
    if (state === 'idle') return 'Idle'
    return 'Offline'
  }

  function statusBadgeClass(status) {
    const state = statusToState(status)
    if (state === 'active') {
      return dark
        ? 'border border-success-500/40 bg-success-500/10 text-success-300'
        : 'border border-success-300 bg-success-100 text-success-700'
    }
    if (state === 'idle') {
      return dark
        ? 'border border-warning-500/40 bg-warning-500/10 text-warning-300'
        : 'border border-warning-300 bg-warning-100 text-warning-700'
    }
    return dark
      ? 'border border-zinc-600 bg-zinc-800 text-zinc-300'
      : 'border border-zinc-300 bg-zinc-100 text-zinc-600'
  }

  function toolLabel(tool) {
    const normalized = String(tool || '').toLowerCase()
    if (normalized === 'claude') return 'Claude'
    if (normalized === 'codex') return 'Codex'
    if (normalized === 'gemini') return 'Gemini'
    return tool || 'Unknown'
  }

  function memberMetaLine(member) {
    return `${toolLabel(member.cliTool)} · ${member.model || 'n/a'} · ${member.projectId || 'n/a'}`
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
  <header class="flex items-center justify-between gap-3 pb-3 border-b {t.keyline}">
    <div>
      <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="mesh-runtime-title">{teamName}</h2>
      <p class="text-[11px] {t.textMuted}" data-testid="mesh-runtime-placeholder">
        {members.length} member{members.length !== 1 ? 's' : ''} · {activeCount} active · {idleCount} idle · refresh 5s
      </p>
    </div>
    <div class="flex items-center gap-1.5">
      <button
        class={actionBrand}
        onclick={onAddAgent}
        data-testid="mesh-add-agent-button"
      >
        + Agent
      </button>
      <button
        class={actionDanger}
        onclick={onDisband}
        disabled={disbanding}
        data-testid="mesh-disband-button"
      >
        {disbanding ? 'Disbanding...' : 'Disband'}
      </button>
    </div>
  </header>

  {#if errorMessage}
    <div class="border-l-2 border-danger-400 pl-3 py-1 text-xs text-danger-600/95" data-testid="mesh-roster-error">
      {errorMessage}
    </div>
  {/if}

  {#if loading && members.length === 0}
    <p class="text-xs {t.textMuted}" data-testid="mesh-roster-loading">Loading roster...</p>
  {:else if members.length === 0}
    <p class="text-xs {t.textMuted}" data-testid="mesh-roster-empty">No members found.</p>
  {:else}
    <div>
      {#each members as member, index}
        <article
          class={`flex items-start justify-between gap-3 py-2 ${index < members.length - 1 ? `border-b ${t.keyline}` : ''}`}
          data-testid={`mesh-roster-card-${member.name}`}
        >
          <div class="min-w-0 space-y-0.5">
            <div class="flex items-center gap-2 min-w-0">
              <span
                class={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium ${statusBadgeClass(member.sessionStatus)}`}
                data-testid={`mesh-status-badge-${member.name}`}
              >
                {statusLabel(member.sessionStatus)}
              </span>
              <span class="text-[13px] truncate min-w-0 {t.textPrimary}" data-testid={`mesh-role-indicator-${member.name}`}>
                {member.role === 'lead' ? `★ ${member.name}` : member.name}
              </span>
            </div>
            <p class="text-[11px] truncate {t.textMuted}" data-testid={`mesh-member-meta-${member.name}`}>
              {memberMetaLine(member)}
            </p>
            {#if member.description}
              <p class="text-[10px] truncate {t.textMuted}" data-testid={`mesh-member-description-${member.name}`}>
                {member.description}
              </p>
            {/if}
          </div>

          <div class="flex shrink-0 justify-end items-center gap-1">
            {#if member.paneId}
              <button
                class={rowActionTone}
                onclick={() => onFocusPane(member.paneId)}
                data-testid={`mesh-focus-pane-${member.name}`}
              >
                Focus
              </button>
            {/if}
            {#if member.role !== 'lead'}
              <button
                class={rowActionTone}
                onclick={() => handleReonboard(member.name)}
                disabled={reonboarding.has(member.name)}
                data-testid={`mesh-reonboard-${member.name}`}
              >
                Re-onboard
              </button>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>
