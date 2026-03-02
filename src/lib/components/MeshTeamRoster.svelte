<script>
  import { themeTokens } from '../themeTokens.js'

  let {
    teamName = '',
    dark = false,
    onAddAgent = () => {},
    onDisband = () => {},
    onFocusPane = () => {},
    refreshNonce = 0,
  } = $props()

  const t = $derived(themeTokens(dark))
  const actionBase = 'rounded-md px-2 py-1 text-[11px] transition-colors'
  const actionBrand = `${actionBase} text-brand-500 hover:text-brand-400 hover:bg-brand-500/10`
  const actionDanger = `${actionBase} text-danger-500/70 hover:text-danger-500 hover:bg-danger-500/10`
  const rowButtonTone = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/70'
      : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-200'
  )
  const leadBadgeClass = $derived(
    dark
      ? 'border border-zinc-600 text-zinc-400 bg-transparent font-mono'
      : 'border border-zinc-300 text-zinc-500 bg-transparent font-mono'
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

  function statusDotColor(status) {
    const state = statusToState(status)
    if (state === 'active') return 'bg-success-400'
    if (state === 'idle') return 'bg-warning-400'
    return dark ? 'bg-zinc-600' : 'bg-zinc-300'
  }

  function toolLabel(tool) {
    if (tool === 'claude') return 'Claude'
    if (tool === 'codex') return 'Codex'
    if (tool === 'gemini') return 'Gemini'
    return tool || 'Unknown'
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
        data-testid="mesh-disband-button"
      >
        Disband
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
    <div class="space-y-0.5">
      <div class="grid grid-cols-[10px_minmax(0,1fr)_minmax(0,180px)_120px] items-center h-5 -mx-2 px-2 text-[10px] uppercase tracking-[0.06em] {t.textMuted}">
        <span>Status</span>
        <span>Name</span>
        <span>Tool · Model</span>
        <span class="text-right">Actions</span>
      </div>
      {#each members as member}
        <article
          class="grid grid-cols-[10px_minmax(0,1fr)_minmax(0,180px)_120px] items-center gap-2 h-[30px] -mx-2 px-2 rounded {dark ? 'hover:bg-zinc-900' : 'hover:bg-zinc-50'} group"
          data-testid={`mesh-roster-card-${member.name}`}
        >
          <span
            class={`w-1.5 h-1.5 shrink-0 rounded-full ${statusDotColor(member.sessionStatus)}`}
            data-testid={`mesh-status-dot-${member.name}`}
          ></span>

          <div class="flex items-center gap-1.5 min-w-0">
            <span class="text-[13px] truncate min-w-0 {t.textPrimary}" data-testid={`mesh-role-indicator-${member.name}`}>
              {member.name}
            </span>
            {#if member.role === 'lead'}
              <span class="text-[9px] uppercase tracking-[0.06em] px-1 py-0.5 rounded {leadBadgeClass}">lead</span>
            {/if}
          </div>

          <span class="text-[11px] truncate {t.textMuted}" data-testid={`mesh-member-meta-${member.name}`}>
            {toolLabel(member.cliTool)}{member.model ? ` · ${member.model}` : ''}
          </span>

          <div class="flex justify-end items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
            {#if member.paneId}
              <button
                class="rounded px-1.5 py-0.5 text-[10px] text-brand-500 hover:text-brand-400 hover:bg-brand-500/10"
                onclick={() => onFocusPane(member.paneId)}
                data-testid={`mesh-focus-pane-${member.name}`}
              >
                Focus
              </button>
            {/if}
            {#if member.role !== 'lead'}
              <button
                class="rounded px-1.5 py-0.5 text-[10px] {rowButtonTone} disabled:opacity-50"
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
