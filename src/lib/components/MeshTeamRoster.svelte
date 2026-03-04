<script>
  import { toolIcon as sessionToolIcon } from '../sessionIndicator.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    teamName = '',
    dark = false,
    onAddAgent = () => {},
    onDisband = () => {},
    onRemoveAgent = () => {},
    onResumeAgent = () => {},
    onFocusPane = () => {},
    disbanding = false,
    removingMembers = [],
    resumingMembers = [],
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
  const rowSelectTone = $derived(
    dark
      ? 'rounded px-1.5 py-0.5 text-[10px] border border-zinc-700 bg-zinc-900 text-zinc-300 disabled:opacity-50'
      : 'rounded px-1.5 py-0.5 text-[10px] border border-zinc-300 bg-white text-zinc-700 disabled:opacity-50'
  )

  let members = $state([])
  let loading = $state(false)
  let errorMessage = $state('')
  let reonboarding = $state(new Set())
  let reonboardSent = $state(new Set())
  let resumeModes = $state({})
  let showOverflowMenu = $state(false)
  const activeCount = $derived.by(
    () => members.filter((member) => statusToState(member.sessionStatus) === 'active').length
  )
  const idleCount = $derived.by(
    () => members.filter((member) => statusToState(member.sessionStatus) === 'idle').length
  )
  const removingMemberSet = $derived.by(() => {
    if (removingMembers instanceof Set) return removingMembers
    if (Array.isArray(removingMembers)) return new Set(removingMembers)
    return new Set()
  })
  const resumingMemberSet = $derived.by(() => {
    if (resumingMembers instanceof Set) return resumingMembers
    if (Array.isArray(resumingMembers)) return new Set(resumingMembers)
    return new Set()
  })

  function statusToState(status) {
    const normalized = String(status || '').toLowerCase()
    if (normalized === 'active') return 'active'
    if (normalized === 'idle') return 'idle'
    return 'offline'
  }

  function normalizeMember(member) {
    const cliTool = member?.cliTool ?? member?.cli_tool ?? ''
    return {
      name: member?.name ?? '',
      role: member?.role ?? 'member',
      cliTool,
      model: member?.model ?? '',
      projectId: member?.projectId ?? member?.project_id ?? '',
      description: member?.description ?? null,
      sessionStatus: String(member?.sessionStatus ?? member?.session_status ?? 'offline').toLowerCase(),
      paneId: member?.paneId ?? member?.pane_id ?? null,
      toolLabel: toolLabel(cliTool),
      toolIcon: sessionToolIcon({ cli_tool: cliTool }),
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
        ? 'border border-success-500/40 bg-success-500/10 text-success-300 animate-[activepulse_2s_ease-in-out_infinite]'
        : 'border border-success-300 bg-success-100 text-success-700 animate-[activepulse_2s_ease-in-out_infinite]'
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

  function handleWindowClick(event) {
    const target = event.target
    if (
      showOverflowMenu &&
      target instanceof Element &&
      !target.closest('[data-testid="mesh-overflow-menu-button"]') &&
      !target.closest('[data-testid="mesh-overflow-menu"]')
    ) {
      showOverflowMenu = false
    }
  }

  function isRemovingMember(memberName) {
    return removingMemberSet.has(memberName)
  }

  function isResumingMember(memberName) {
    return resumingMemberSet.has(memberName)
  }

  function isRowBusy(memberName) {
    return isRemovingMember(memberName) || isResumingMember(memberName)
  }

  function getResumeMode(memberName) {
    return resumeModes[memberName] ?? 'continue'
  }

  function setResumeMode(memberName, mode) {
    resumeModes = { ...resumeModes, [memberName]: mode === 'fresh' ? 'fresh' : 'continue' }
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
      reonboardSent = new Set([...reonboardSent, memberName])
      setTimeout(() => {
        const next = new Set(reonboardSent)
        next.delete(memberName)
        reonboardSent = next
      }, 2000)
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

<svelte:window onclick={handleWindowClick} />

<section class="space-y-3" data-testid="mesh-team-roster">
  <header class="flex items-center justify-between gap-3 pb-3 border-b {t.keyline}">
    <div>
      <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="mesh-runtime-title">{teamName}</h2>
      <div class="flex items-center gap-1.5">
        <p class="text-[11px] {t.textMuted}" data-testid="mesh-runtime-placeholder">
          {members.length} member{members.length !== 1 ? 's' : ''} · {activeCount} active · {idleCount} idle · refresh 5s
        </p>
        <button
          class="text-[10px] {t.textMuted} hover:text-brand-500"
          onclick={() => {
            void refreshRoster()
          }}
          data-testid="mesh-roster-refresh"
          title="Refresh now"
        >
          ↻
        </button>
      </div>
    </div>
    <div class="flex items-center gap-1.5">
      <button
        class={actionBrand}
        onclick={() => {
          showOverflowMenu = false
          onAddAgent()
        }}
        data-testid="mesh-add-agent-button"
      >
        + Agent
      </button>
      <div class="relative">
        <button
          class="rounded-md px-1.5 py-1 text-[11px] {t.textMuted} {dark
            ? 'hover:text-zinc-200 hover:bg-zinc-800/70'
            : 'hover:text-zinc-900 hover:bg-zinc-100'}"
          onclick={() => {
            showOverflowMenu = !showOverflowMenu
          }}
          data-testid="mesh-overflow-menu-button"
        >
          ⋯
        </button>
        {#if showOverflowMenu}
          <div
            class="absolute right-0 top-full mt-1 rounded-md shadow-lg border {dark ? 'bg-zinc-900 border-zinc-700' : 'bg-white border-zinc-200'} py-1 z-10 min-w-[120px]"
            data-testid="mesh-overflow-menu"
          >
            <button
              class={actionDanger}
              onclick={() => {
                onDisband()
              }}
              disabled={disbanding}
              data-testid="mesh-disband-button"
            >
              {disbanding ? 'Disbanding...' : 'Disband team'}
            </button>
          </div>
        {/if}
      </div>
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
              <span class="text-[13px] font-semibold truncate min-w-0 {t.textPrimary}" data-testid={`mesh-role-indicator-${member.name}`}>
                {member.role === 'lead' ? `★ ${member.name}` : member.name}
              </span>
            </div>
            <p class="text-[11px] truncate {t.textMuted}" data-testid={`mesh-member-meta-${member.name}`}>
              <span class="inline-flex items-center gap-1">
                <svg
                  class="h-3 w-3 shrink-0"
                  viewBox={member.toolIcon.viewBox}
                  fill="currentColor"
                  aria-hidden="true"
                  data-testid={`mesh-member-tool-icon-${member.name}`}
                >
                  <path d={member.toolIcon.path}></path>
                </svg>
                <span>{member.toolLabel}</span>
              </span>
              {' · '}
              {member.model || 'n/a'}
              {' · '}
              {member.projectId || 'n/a'}
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
                title="Jump to this agent's terminal pane"
                disabled={isRowBusy(member.name)}
                data-testid={`mesh-focus-pane-${member.name}`}
              >
                Focus
              </button>
            {/if}
            {#if statusToState(member.sessionStatus) === 'offline'}
              <select
                class={rowSelectTone}
                value={getResumeMode(member.name)}
                onchange={(event) => setResumeMode(member.name, event.currentTarget.value)}
                disabled={isRowBusy(member.name)}
                title="Resume mode"
                data-testid={`mesh-resume-mode-${member.name}`}
              >
                <option value="continue">Continue</option>
                <option value="fresh">Fresh</option>
              </select>
              <button
                class={rowActionTone}
                onclick={() => onResumeAgent(member.name, getResumeMode(member.name))}
                disabled={isRowBusy(member.name)}
                title="Resume this agent session"
                data-testid={`mesh-resume-member-${member.name}`}
              >
                {isResumingMember(member.name) ? 'Resuming...' : 'Resume'}
              </button>
            {/if}
            {#if member.role !== 'lead'}
              <button
                class={rowActionTone}
                onclick={() => handleReonboard(member.name)}
                disabled={reonboarding.has(member.name) || isRowBusy(member.name)}
                title="Re-send setup instructions to this agent"
                data-testid={`mesh-reonboard-${member.name}`}
              >
                Re-onboard
              </button>
              <button
                class={rowActionTone}
                onclick={() => onRemoveAgent(member.name)}
                disabled={isRowBusy(member.name)}
                title="Remove this agent and clean up managed resources"
                data-testid={`mesh-remove-member-${member.name}`}
              >
                {isRemovingMember(member.name) ? 'Removing...' : 'Remove'}
              </button>
              {#if reonboardSent.has(member.name)}
                <span class="text-[10px] text-success-400 animate-[meshfade_2s_ease-out_forwards]" data-testid={`mesh-reonboard-sent-${member.name}`}>
                  Sent!
                </span>
              {/if}
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {/if}
</section>

<style>
  @keyframes activepulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.7;
    }
  }

  @keyframes meshfade {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }
</style>
