<script>
  import { themeTokens } from '../themeTokens.js'
  import StatusBadge from './StatusBadge.svelte'

  let {
    node = {},
    mode = 'setup',
    dark = false,
    actions = {},
    anchor = null,
    onVisible = () => {},
  } = $props()

  const DETAIL_ENTER_DURATION_MS = 80
  const DETAIL_ENTER_EASING = 'cubic-bezier(0.22,1,0.36,1)'
  let visibleTimer = null
  let visibleNotified = false

  const name = $derived.by(() => String(node?.name ?? '').trim() || 'unnamed-agent')
  const role = $derived.by(() => String(node?.role ?? 'agent'))
  const tool = $derived.by(() => String(node?.tool ?? 'claude'))
  const model = $derived.by(() => String(node?.model ?? ''))
  const status = $derived.by(() => String(node?.status ?? 'offline'))
  const projectId = $derived.by(() => String(node?.projectId ?? ''))
  const isCrossProject = $derived.by(() => Boolean(node?.isCrossProject ?? node?.is_cross_project))
  const projectLabel = $derived.by(() => String(node?.projectLabel ?? node?.project_label ?? '').trim())
  const description = $derived.by(() => String(node?.description ?? '').trim())
  const roleName = $derived.by(() => String(node?.roleName ?? node?.role_name ?? '').trim())
  const focusArea = $derived.by(() => String(node?.focusArea ?? node?.focus_area ?? '').trim())
  const contextSummary = $derived.by(() => String(node?.contextSummary ?? node?.context_summary ?? '').trim())
  const behaviorSummary = $derived.by(() => String(node?.behaviorSummary ?? node?.behavior_summary ?? '').trim())
  const paneId = $derived.by(() => String(node?.paneId ?? '').trim())
  const sessionId = $derived.by(() => String(node?.sessionId ?? '').trim())
  const sessionState = $derived.by(() => String(node?.sessionState ?? '').trim())
  const displayProject = $derived.by(() => projectLabel || projectId || 'No project')
  const displayProjectContext = $derived.by(() => {
    if (!isCrossProject || !projectId) return ''
    if (!projectLabel) return projectId
    return projectId === projectLabel ? '' : projectId
  })

  const t = $derived(themeTokens(dark))
  const normalizedRole = $derived(role === 'lead' ? 'lead' : 'agent')
  const normalizedMode = $derived(mode === 'runtime' ? 'runtime' : 'setup')

  const normalizedStatus = $derived.by(() => {
    const value = String(status || '').toLowerCase()
    if (value === 'active') return 'active'
    if (value === 'idle') return 'idle'
    return 'offline'
  })

  const roleLabel = $derived(normalizedRole === 'lead' ? 'Lead' : 'Agent')

  const toolLabel = $derived.by(() => {
    const value = String(tool || '').toLowerCase()
    if (value === 'claude') return 'Claude'
    if (value === 'codex') return 'Codex'
    if (value === 'gemini') return 'Gemini'
    return tool || 'Unknown'
  })

  const statusLabel = $derived.by(() => {
    if (normalizedStatus === 'active') return 'Active'
    if (normalizedStatus === 'idle') return 'Idle'
    return 'Offline'
  })

  const runtimeHasDiagnostics = $derived(
    normalizedMode === 'runtime' && (paneId.length > 0 || sessionId.length > 0 || sessionState.length > 0)
  )
  const hasRoleSection = $derived(
    roleName.length > 0 || focusArea.length > 0 || contextSummary.length > 0 || behaviorSummary.length > 0
  )

  const surfaceClass = $derived(
    dark
      ? 'text-zinc-100 border-white/12 bg-zinc-950/95 shadow-[0_24px_48px_rgba(0,0,0,0.55)]'
      : 'text-zinc-900 border-brand-200/90 bg-white/97 shadow-[0_22px_42px_rgba(15,23,42,0.16)]'
  )
  const closeTone = $derived(
    dark
      ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/10'
      : 'text-zinc-500 hover:text-zinc-900 hover:bg-black/5'
  )
  const cardTone = $derived(
    dark
      ? 'bg-white/[0.04] border-white/10 text-zinc-200'
      : 'bg-brand-50/70 border-brand-200/80 text-zinc-700'
  )
  const metaPillTone = $derived(
    dark
      ? 'bg-white/[0.05] border-white/10 text-zinc-200'
      : 'bg-white border-brand-200 text-zinc-700'
  )
  const crossProjectCardTone = $derived(
    dark
      ? 'bg-brand-500/[0.08] border-brand-400/20 text-zinc-100'
      : 'bg-brand-50/85 border-brand-200/90 text-zinc-900'
  )
  const crossProjectProjectTone = $derived(
    dark
      ? 'bg-brand-500/16 border-brand-400/35 text-brand-100'
      : 'bg-white border-brand-300 text-brand-700'
  )
  const crossProjectLocationTone = $derived(
    dark
      ? 'bg-white/[0.05] border-white/12 text-zinc-200'
      : 'bg-white/90 border-zinc-300 text-zinc-700'
  )
  const crossProjectLabelTone = $derived(dark ? 'text-brand-100/90' : 'text-brand-700')
  const crossProjectPathTone = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const roleChipTone = $derived(
    normalizedRole === 'lead'
      ? (dark
        ? 'bg-brand-500/20 text-brand-200 border-brand-400/35'
        : 'bg-brand-50 text-brand-700 border-brand-300')
      : (dark
        ? 'bg-zinc-800 text-zinc-200 border-zinc-600'
        : 'bg-zinc-100 text-zinc-700 border-zinc-300')
  )
  const secondaryActionTone = $derived(
    dark
      ? 'bg-white/[0.04] border-white/12 text-zinc-100 hover:bg-white/[0.1]'
      : 'bg-white border-brand-300 text-brand-800 hover:bg-brand-50'
  )
  const primaryActionTone = $derived(
    dark
      ? 'bg-brand-500 text-white border-brand-400 hover:bg-brand-400'
      : 'bg-brand-600 text-white border-brand-600 hover:bg-brand-500'
  )
  const dangerActionTone = $derived(
    dark
      ? 'bg-danger-500/20 border-danger-400/40 text-danger-200 hover:bg-danger-500/30'
      : 'bg-danger-50 border-danger-300 text-danger-700 hover:bg-danger-100'
  )
  const normalizedAnchor = $derived.by(() => {
    if (!anchor || typeof anchor !== 'object') return null
    const left = Number(anchor.left)
    const top = Number(anchor.top)
    const cardWidth = Number(anchor.cardWidth)
    if (!Number.isFinite(left) || !Number.isFinite(top)) return null
    return {
      left,
      top,
      cardWidth: Number.isFinite(cardWidth) ? Math.max(176, cardWidth) : 240,
      placement: anchor.placement === 'top' ? 'top' : 'bottom',
    }
  })
  const anchoredStyle = $derived.by(() => {
    const animationStyle = `animation: mesh-detail-enter ${DETAIL_ENTER_DURATION_MS}ms ${DETAIL_ENTER_EASING}`
    if (!normalizedAnchor) {
      return [
        'left: 50%',
        'top: 100%',
        'transform: translateX(-50%)',
        'margin-top: 12px',
        'width: min(21rem, calc(100% - 16px))',
        animationStyle,
      ].join('; ')
    }

    return [
      `left: ${normalizedAnchor.left}px`,
      `top: ${normalizedAnchor.top}px`,
      `width: ${normalizedAnchor.cardWidth}px`,
      'max-width: calc(100% - 16px)',
      animationStyle,
    ].join('; ')
  })
  const placement = $derived.by(() => normalizedAnchor?.placement ?? 'bottom')

  function invoke(handler) {
    if (typeof handler === 'function') handler()
  }

  function notifyVisible() {
    if (visibleNotified) return
    visibleNotified = true
    if (typeof onVisible === 'function') onVisible()
  }

  function handleAnimationEnd(event) {
    if (event?.animationName !== 'mesh-detail-enter') return
    notifyVisible()
  }

  $effect(() => {
    visibleNotified = false
    if (visibleTimer) {
      clearTimeout(visibleTimer)
      visibleTimer = null
    }

    visibleTimer = setTimeout(() => {
      visibleTimer = null
      notifyVisible()
    }, DETAIL_ENTER_DURATION_MS)

    return () => {
      if (visibleTimer) {
        clearTimeout(visibleTimer)
        visibleTimer = null
      }
    }
  })
</script>

<aside
  class="absolute z-20 rounded-2xl border p-3.5 backdrop-blur-sm pointer-events-auto mesh-node-detail {surfaceClass}"
  style={anchoredStyle}
  data-testid="mesh-node-detail"
  data-placement={placement}
  data-enter-duration-ms={DETAIL_ENTER_DURATION_MS}
  onanimationend={handleAnimationEnd}
>
  <header class="flex items-start gap-2">
    <div class="min-w-0 flex-1 space-y-1.5">
      <div class="flex items-center gap-2 min-w-0">
        <h3
          class="text-[14px] font-semibold truncate"
          data-testid="mesh-node-detail-name"
          title={name}
        >{name}</h3>
        <span class="shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide {roleChipTone}">
          {roleLabel}
        </span>
      </div>
      <div class="flex items-center gap-1.5">
        <StatusBadge status={normalizedStatus} size="md" {dark} />
        <span class="text-[11px] {dark ? t.textMuted : 'text-zinc-600'}" data-testid="mesh-node-detail-status">{statusLabel}</span>
      </div>
    </div>

    <button
      class="h-8 w-8 shrink-0 rounded-lg border border-transparent transition-colors {closeTone}"
      onclick={() => invoke(actions?.onClose)}
      aria-label="Close node detail"
      data-testid="mesh-node-detail-close"
    >
      <span aria-hidden="true" class="text-sm leading-none">x</span>
    </button>
  </header>

  <div class="mt-3 flex flex-wrap items-center gap-2">
    <p
      class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium {metaPillTone}"
      data-testid="mesh-node-detail-tool-model"
      title={model ? `${toolLabel} · ${model}` : toolLabel}
    >
      <span class="inline-block h-1.5 w-1.5 rounded-full bg-brand-500" aria-hidden="true"></span>
      {toolLabel} {model ? `· ${model}` : ''}
    </p>

    {#if !isCrossProject}
      <p
        class="inline-flex max-w-full items-center gap-1.5 truncate rounded-full border px-2.5 py-1 text-[11px] {metaPillTone}"
        data-testid="mesh-node-detail-project"
        title={projectId || 'No project'}
      >
        <span aria-hidden="true">Project:</span> {displayProject}
      </p>
    {/if}
  </div>

  {#if isCrossProject}
    <section
      class="mt-3 rounded-xl border p-2.5 space-y-2 {crossProjectCardTone}"
      data-testid="mesh-node-detail-project-card"
    >
      <div class="flex items-center justify-between gap-3">
        <p class="text-[10px] font-semibold uppercase tracking-[0.16em] {crossProjectLabelTone}">
          Project Context
        </p>
        <span class="text-[10px] {dark ? 'text-zinc-400' : 'text-zinc-500'}">other project</span>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <p
          class="inline-flex max-w-full items-center gap-1.5 truncate rounded-full border px-2.5 py-1 text-[11px] font-medium {crossProjectProjectTone}"
          data-testid="mesh-node-detail-project"
          title={projectId || 'No project'}
        >
          <span aria-hidden="true">Project:</span> {displayProject}
        </p>

        <p
          class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium {crossProjectLocationTone}"
          data-testid="mesh-node-detail-location"
        >
          <span aria-hidden="true">Location:</span> other project
        </p>
      </div>

      {#if displayProjectContext}
        <p
          class="text-[11px] leading-relaxed break-all {crossProjectPathTone}"
          data-testid="mesh-node-detail-project-context"
          title={displayProjectContext}
        >
          Path: {displayProjectContext}
        </p>
      {/if}
    </section>
  {:else if displayProjectContext}
    <p
      class="mt-2 text-[11px] break-all {dark ? t.textMuted : 'text-zinc-600'}"
      data-testid="mesh-node-detail-project-context"
      title={displayProjectContext}
    >
      Path: {displayProjectContext}
    </p>
  {/if}

  {#if hasRoleSection}
    <section class="mt-3 rounded-xl border p-2.5 space-y-2 {cardTone}" data-testid="mesh-node-detail-role-section">
      <p class="text-[10px] font-semibold uppercase tracking-wide {dark ? 'text-zinc-400' : 'text-zinc-500'}">Role</p>
      {#if roleName.length > 0}
        <p class="text-[12px] font-semibold" data-testid="mesh-node-detail-role-name">{roleName}</p>
      {/if}
      {#if focusArea.length > 0}
        <p class="text-[11px] leading-relaxed" data-testid="mesh-node-detail-focus-area">
          <span class="font-medium">Focus:</span> {focusArea}
        </p>
      {/if}
      {#if contextSummary.length > 0}
        <p class="text-[11px] leading-relaxed" data-testid="mesh-node-detail-context-summary">{contextSummary}</p>
      {/if}
      {#if behaviorSummary.length > 0}
        <p class="text-[11px] leading-relaxed" data-testid="mesh-node-detail-behavior-summary">{behaviorSummary}</p>
      {/if}
    </section>
  {/if}

  {#if description.length > 0}
    <section class="mt-3 rounded-xl border p-2.5 {cardTone}">
      <p
        class="text-[12px] leading-relaxed"
        data-testid="mesh-node-detail-description"
      >
        {description}
      </p>
    </section>
  {/if}

  {#if runtimeHasDiagnostics}
    <section class="mt-3 rounded-xl border p-2.5 space-y-1.5 {cardTone}" data-testid="mesh-node-detail-runtime">
      <p class="text-[10px] font-semibold uppercase tracking-wide {dark ? 'text-zinc-400' : 'text-zinc-500'}">Runtime</p>
      {#if paneId.length > 0}
        <div class="flex items-center justify-between gap-3 text-[11px]">
          <span class="{dark ? 'text-zinc-400' : 'text-zinc-500'}">Pane</span>
          <code class="font-mono text-[11px] {dark ? 'text-zinc-200' : 'text-zinc-700'}" data-testid="mesh-node-detail-pane">{paneId}</code>
        </div>
      {/if}
      {#if sessionId.length > 0}
        <div class="flex items-center justify-between gap-3 text-[11px]">
          <span class="{dark ? 'text-zinc-400' : 'text-zinc-500'}">Session</span>
          <code class="font-mono text-[11px] truncate {dark ? 'text-zinc-200' : 'text-zinc-700'}" data-testid="mesh-node-detail-session" title={sessionId}>{sessionId}</code>
        </div>
      {/if}
      {#if sessionState.length > 0}
        <div class="flex items-center justify-between gap-3 text-[11px]">
          <span class="{dark ? 'text-zinc-400' : 'text-zinc-500'}">State</span>
          <span class="{dark ? 'text-zinc-200' : 'text-zinc-700'}" data-testid="mesh-node-detail-session-state">{sessionState}</span>
        </div>
      {/if}
    </section>
  {/if}

  <div class="mt-3 pt-2.5 border-t {dark ? 'border-white/10' : t.keyline}">
    <div class="flex flex-wrap gap-2">
      {#if normalizedMode === 'setup'}
        <button
          class="inline-flex h-9 items-center gap-1.5 rounded-full border px-3 text-[12px] font-semibold transition-colors {secondaryActionTone}"
          onclick={() => invoke(actions?.onEdit)}
          data-testid="mesh-node-detail-edit"
        >
          <span aria-hidden="true">Edit</span>
        </button>
        {#if normalizedRole !== 'lead'}
          <button
            class="inline-flex h-9 items-center gap-1.5 rounded-full border px-3 text-[12px] font-semibold transition-colors {dangerActionTone}"
            onclick={() => invoke(actions?.onRemove)}
            data-testid="mesh-node-detail-remove"
          >
            <span aria-hidden="true">Remove</span>
          </button>
        {/if}
      {:else}
        <button
          class="inline-flex h-9 items-center gap-1.5 rounded-full border px-3 text-[12px] font-semibold transition-colors disabled:opacity-45 disabled:cursor-not-allowed {primaryActionTone}"
          onclick={() => invoke(actions?.onResume)}
          disabled={Boolean(actions?.resumeDisabled) || typeof actions?.onResume !== 'function'}
          data-testid="mesh-node-detail-resume"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
          Resume
        </button>
        <button
          class="inline-flex h-9 items-center gap-1.5 rounded-full border px-3 text-[12px] font-semibold transition-colors disabled:opacity-45 disabled:cursor-not-allowed {dangerActionTone}"
          onclick={() => invoke(actions?.onStop)}
          disabled={Boolean(actions?.stopDisabled) || typeof actions?.onStop !== 'function'}
          data-testid="mesh-node-detail-stop"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
          Stop
        </button>
        <button
          class="inline-flex h-9 items-center gap-1.5 rounded-full border px-3 text-[12px] font-semibold transition-colors disabled:opacity-45 disabled:cursor-not-allowed {secondaryActionTone}"
          onclick={() => invoke(actions?.onCapture)}
          disabled={Boolean(actions?.captureDisabled) || typeof actions?.onCapture !== 'function'}
          data-testid="mesh-node-detail-capture"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M4 7h4l2-2h4l2 2h4v12H4z"/><circle cx="12" cy="13" r="3"/></svg>
          Capture
        </button>
        <button
          class="inline-flex h-9 items-center gap-1.5 rounded-full border px-3 text-[12px] font-semibold transition-colors disabled:opacity-45 disabled:cursor-not-allowed {secondaryActionTone}"
          onclick={() => invoke(actions?.onFocusPane)}
          disabled={typeof actions?.onFocusPane !== 'function'}
          data-testid="mesh-node-detail-focus"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 3h7v2H5v5H3zM21 3v7h-2V5h-5V3zM3 21v-7h2v5h5v2zM21 21h-7v-2h5v-5h2z"/></svg>
          Focus
        </button>
      {/if}
    </div>
  </div>
</aside>
