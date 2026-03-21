<script>
  import { focusFirstInteractiveElement, handleModalKeydown, registerModalLayer } from '../a11y.js'
  import MarkdownRenderer from '../MarkdownRenderer.svelte'
  import { themeTokens } from '../themeTokens.js'

  let {
    node = {},
    mode = 'runtime',
    dark = false,
    actions = {},
    anchor = null,
    onVisible = () => {},
  } = $props()

  let dialogEl = $state(null)
  let modalRootEl = $state(null)
  let closeButtonEl = $state(null)
  let restoreFocusElement = null

  const t = $derived(themeTokens(dark))
  const normalizedContext = $derived(mode === 'runtime' ? 'runtime' : 'roster')
  const normalizedRole = $derived(String(node?.role ?? '').trim().toLowerCase() === 'lead' ? 'lead' : 'agent')
  const name = $derived.by(() => String(node?.name ?? '').trim())
  const roleName = $derived.by(() => String(node?.roleName ?? node?.role_name ?? '').trim())
  const titleLabel = $derived(roleName || name || 'Role detail')
  const subjectLabel = $derived.by(() => {
    if (!roleName || !name || roleName === name) return ''
    return name
  })
  const tool = $derived.by(() => String(node?.tool ?? node?.cliTool ?? node?.cli_tool ?? 'claude').trim().toLowerCase())
  const toolLabel = $derived.by(() => {
    if (tool === 'codex') return 'Codex'
    if (tool === 'gemini') return 'Gemini'
    if (tool === 'claude') return 'Claude'
    return String(node?.tool ?? 'Unknown')
  })
  const model = $derived.by(() => String(node?.model ?? node?.modelName ?? node?.model_name ?? '').trim())
  const projectId = $derived.by(() => String(node?.projectId ?? node?.project_id ?? '').trim())
  const projectLabel = $derived.by(() => String(node?.projectLabel ?? node?.project_label ?? '').trim())
  const projectDisplay = $derived(projectLabel || projectId || 'No project')
  const status = $derived.by(() => String(node?.status ?? node?.sessionStatus ?? node?.session_status ?? '').trim().toLowerCase())
  const statusLabel = $derived.by(() => {
    if (normalizedContext !== 'runtime') return 'Template'
    if (status === 'active') return 'Active'
    if (status === 'idle') return 'Idle'
    return 'Offline'
  })
  const statusDotTone = $derived.by(() => {
    if (normalizedContext !== 'runtime') {
      return dark ? 'bg-white/45' : 'bg-zinc-400'
    }
    if (status === 'active') return dark ? 'bg-emerald-300' : 'bg-emerald-500'
    if (status === 'idle') return dark ? 'bg-amber-300' : 'bg-amber-500'
    return dark ? 'bg-zinc-500' : 'bg-zinc-400'
  })
  const focusArea = $derived.by(() => String(node?.focusArea ?? node?.focus_area ?? '').trim())
  const contextSummary = $derived.by(() => String(node?.contextSummary ?? node?.context_summary ?? '').trim())
  const behaviorSummary = $derived.by(() => String(node?.behaviorSummary ?? node?.behavior_summary ?? '').trim())
  const instructions = $derived.by(() => String(node?.instructions ?? node?.description ?? '').trim())
  const paneId = $derived.by(() => String(node?.paneId ?? node?.pane_id ?? '').trim())
  const sessionId = $derived.by(() => String(node?.sessionId ?? node?.session_id ?? '').trim())
  const sessionState = $derived.by(() => String(node?.sessionState ?? node?.session_state ?? '').trim())
  const roleId = $derived.by(() => String(node?.roleId ?? node?.role_id ?? '').trim())
  const capabilities = $derived.by(() =>
    Array.isArray(node?.capabilities)
      ? node.capabilities.map((entry) => String(entry ?? '').trim()).filter(Boolean)
      : []
  )
  const behavioralContract = $derived.by(() => normalizeBehavioralContract(node?.behavioralContract ?? node?.behavioral_contract))

  const overlayTone = $derived(
    dark
      ? 'border-white/[0.1] bg-brand-950/98 text-zinc-100 shadow-[0_38px_130px_rgba(0,0,0,0.62),0_10px_28px_rgba(2,10,12,0.4)]'
      : 'border-brand-200/80 bg-white/98 text-zinc-900 shadow-[0_32px_120px_rgba(15,23,42,0.18),0_10px_28px_rgba(15,23,42,0.08)]'
  )
  const toolbarTone = $derived(
    dark
      ? 'border-white/[0.08] bg-brand-950/92 shadow-[0_12px_36px_rgba(0,0,0,0.2)]'
      : 'border-brand-200/70 bg-white/95 shadow-[0_10px_26px_rgba(15,23,42,0.08)]'
  )
  const focusCardTone = $derived(
    dark
      ? 'border-brand-300/25 bg-brand-500/[0.14] shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_16px_34px_rgba(0,0,0,0.18)]'
      : 'border-brand-200/80 bg-brand-50/95 shadow-[0_14px_32px_rgba(15,23,42,0.06)]'
  )
  const configTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.04] shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]'
      : 'border-zinc-200 bg-zinc-50/85 shadow-[0_10px_26px_rgba(15,23,42,0.05)]'
  )
  const sectionTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.025] shadow-[inset_0_1px_0_rgba(255,255,255,0.03),0_16px_36px_rgba(0,0,0,0.16)]'
      : 'border-zinc-200/90 bg-white shadow-[0_14px_34px_rgba(15,23,42,0.06)]'
  )
  const secondaryActionTone = $derived(
    dark
      ? 'border-white/[0.1] bg-white/[0.03] text-zinc-100 hover:bg-white/[0.08]'
      : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
  )
  const primaryActionTone = $derived(
    dark
      ? 'border-brand-400/70 bg-brand-500 text-white hover:bg-brand-400'
      : 'border-brand-600 bg-brand-600 text-white hover:bg-brand-500'
  )
  const dangerActionTone = $derived(
    dark
      ? 'border-danger-400/35 bg-danger-500/12 text-danger-100 hover:bg-danger-500/18'
      : 'border-danger-300 bg-danger-50 text-danger-700 hover:bg-danger-100'
  )
  const badgeTone = $derived(
    dark
      ? 'border-white/[0.1] bg-white/[0.04] text-zinc-200'
      : 'border-zinc-200 bg-white text-zinc-700'
  )
  const leadBadgeTone = $derived(
    dark
      ? 'border-amber-400/35 bg-amber-500/14 text-amber-100'
      : 'border-amber-300/70 bg-amber-50 text-amber-800'
  )
  const closeTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.03] text-zinc-400 hover:bg-white/[0.08] hover:text-zinc-100'
      : 'border-zinc-200 bg-white/90 text-zinc-500 hover:bg-black/5 hover:text-zinc-900'
  )
  const codeTheme = $derived(dark ? 'github-dark' : 'github-light')
  const breadcrumbLabel = $derived(normalizedContext === 'runtime' ? 'Team Roster' : 'Role Catalog')
  const canFocusPane = $derived(typeof actions?.onFocusPane === 'function')

  const contextMarkdown = $derived(contextSummary)
  const instructionsMarkdown = $derived(instructions)
  const behaviorMarkdown = $derived.by(() => buildBehaviorMarkdown(behavioralContract, behaviorSummary))
  const configurationEntries = $derived.by(() => {
    const entries = [
      { label: 'Tool', value: toolLabel, testId: null },
      { label: 'Model', value: model || 'Not specified', testId: null },
    ]

    if (roleId) {
      entries.push({ label: 'Role ID', value: roleId, testId: null })
    }

    if (projectDisplay && projectDisplay !== 'No project') {
      entries.push({ label: 'Project', value: projectDisplay, testId: 'mesh-node-detail-project' })
    }

    if (normalizedContext === 'runtime') {
      entries.push({ label: 'Status', value: statusLabel, testId: 'mesh-node-detail-status' })
      if (paneId) entries.push({ label: 'Pane', value: paneId, testId: 'mesh-node-detail-pane' })
      if (sessionId) entries.push({ label: 'Session', value: sessionId, testId: 'mesh-node-detail-session' })
      if (sessionState) {
        entries.push({ label: 'Session State', value: sessionState, testId: 'mesh-node-detail-session-state' })
      }
    }

    if (capabilities.length > 0) {
      entries.push({ label: 'Capabilities', value: capabilities.join(', '), testId: 'mesh-node-detail-capabilities' })
    }

    return entries
  })

  function normalizeBehavioralContract(value) {
    const base = {
      communication: [],
      execution: [],
      escalation: [],
    }

    if (value && typeof value === 'object' && !Array.isArray(value)) {
      for (const key of Object.keys(base)) {
        if (Array.isArray(value[key])) {
          base[key] = value[key].map((entry) => String(entry ?? '').trim()).filter(Boolean)
        }
      }
      return base
    }

    if (Array.isArray(value)) {
      base.communication = value
        .map((entry) => {
          if (typeof entry === 'string') return entry.trim()
          if (!entry || typeof entry !== 'object') return ''
          const rule = String(entry.rule ?? entry.text ?? '').trim()
          const enabled = entry.enabled === undefined ? true : Boolean(entry.enabled)
          return enabled ? rule : ''
        })
        .filter(Boolean)
    }

    return base
  }

  function fallbackBulletList(text) {
    return String(text ?? '')
      .split(/\n+/)
      .map((entry) => entry.trim().replace(/^[*-]\s*/, ''))
      .filter(Boolean)
  }

  function buildBehaviorMarkdown(contract, fallbackText) {
    const sections = [
      ['Communication', contract.communication],
      ['Execution', contract.execution],
      ['Escalation', contract.escalation],
    ].filter(([, entries]) => entries.length > 0)

    if (sections.length > 0) {
      return sections
        .map(([label, entries]) => `### ${label}\n${entries.map((entry) => `- ${entry}`).join('\n')}`)
        .join('\n\n')
    }

    const fallbackEntries = fallbackBulletList(fallbackText)
    if (fallbackEntries.length > 0) {
      return fallbackEntries.map((entry) => `- ${entry}`).join('\n')
    }

    return ''
  }

  function invoke(handler) {
    if (typeof handler === 'function') handler()
  }

  function close() {
    invoke(actions?.onClose)
  }

  function handleBackdropClick(event) {
    if (event.target !== event.currentTarget) return
    close()
  }

  function handleBackdropKeydown(event) {
    if (event.key === 'Escape') {
      event.preventDefault()
      close()
    }
  }

  function handleKeydown(event) {
    handleModalKeydown(event, dialogEl, close)
  }

  $effect(() => {
    if (!dialogEl || !modalRootEl) return
    if (
      !restoreFocusElement
      && document.activeElement instanceof HTMLElement
      && !modalRootEl.contains(document.activeElement)
    ) {
      restoreFocusElement = document.activeElement
    }

    const unregisterModal = registerModalLayer(modalRootEl)
    const rafId = requestAnimationFrame(() => {
      focusFirstInteractiveElement(dialogEl, () => closeButtonEl)
      onVisible?.()
    })

    window.addEventListener('keydown', handleKeydown)
    return () => {
      cancelAnimationFrame(rafId)
      unregisterModal()
      window.removeEventListener('keydown', handleKeydown)
      if (restoreFocusElement?.isConnected) {
        restoreFocusElement.focus()
      }
      restoreFocusElement = null
    }
  })
</script>

<div
  bind:this={modalRootEl}
  class="fixed inset-0 z-40 flex items-center justify-center bg-black/62 p-4 backdrop-blur-[6px]"
  data-testid="mesh-node-detail-host"
  role="presentation"
  tabindex="-1"
  onclick={handleBackdropClick}
  onkeydown={handleBackdropKeydown}
>
  <div
    bind:this={dialogEl}
    class="relative flex h-[min(100%,calc(100vh-2rem))] w-full max-w-[980px] flex-col overflow-hidden rounded-[32px] border {overlayTone}"
    role="dialog"
    aria-modal="true"
    aria-label={titleLabel}
    tabindex="-1"
    data-testid="mesh-node-detail"
  >
    <button
      bind:this={closeButtonEl}
      class="absolute right-6 top-6 z-20 inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-full border transition {closeTone}"
      type="button"
      aria-label="Close role detail"
      onclick={close}
      data-testid="mesh-node-detail-close"
    >
      <span aria-hidden="true" class="text-lg leading-none">×</span>
    </button>

    <div class="sticky top-0 z-10 border-b px-6 pb-5 pt-6 backdrop-blur {toolbarTone}">
      <div class="mx-auto w-full max-w-[640px] pr-12">
        <div class="flex items-start gap-4">
          <div class="min-w-0 flex-1 space-y-2">
            <p class="text-[11px] font-medium uppercase tracking-[0.18em] {t.textMuted}">
              {breadcrumbLabel}
            </p>
            <div class="flex min-w-0 flex-wrap items-center gap-3">
              <h2 class="truncate text-[36px] font-semibold leading-none" data-testid="mesh-node-detail-name">
                {titleLabel}
              </h2>
              {#if normalizedRole === 'lead'}
                <span class="inline-flex items-center rounded-full border px-2.5 py-1 text-[12px] font-semibold {leadBadgeTone}">
                  Lead
                </span>
              {/if}
            </div>
            <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] {t.textSecondary}">
              <span class="inline-flex items-center gap-2">
                <span class="inline-block h-2 w-2 rounded-full {statusDotTone}" aria-hidden="true"></span>
                {toolLabel}{model ? ` · ${model}` : ''}
              </span>
              <span class="inline-flex items-center gap-2">
                  <span class="inline-block h-2 w-2 rounded-full {statusDotTone}" aria-hidden="true"></span>
                <span>{statusLabel}</span>
              </span>
              {#if subjectLabel}
                <span class="{t.textMuted}" data-testid="mesh-node-detail-subject">{subjectLabel}</span>
              {/if}
            </div>
          </div>
        </div>

        <div class="mt-5 flex flex-wrap items-center gap-2" data-testid="mesh-node-detail-toolbar">
          {#if normalizedContext === 'runtime'}
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {primaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onResume)}
              disabled={Boolean(actions?.resumeDisabled) || typeof actions?.onResume !== 'function'}
              data-testid="mesh-node-detail-resume"
            >
              Resume
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {dangerActionTone}"
              type="button"
              onclick={() => invoke(actions?.onStop)}
              disabled={Boolean(actions?.stopDisabled) || typeof actions?.onStop !== 'function'}
              data-testid="mesh-node-detail-stop"
            >
              Stop
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onFocusPane)}
              disabled={!canFocusPane}
              data-testid="mesh-node-detail-focus"
            >
              Focus Pane
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onCapture)}
              disabled={Boolean(actions?.captureDisabled) || typeof actions?.onCapture !== 'function'}
              data-testid="mesh-node-detail-capture"
            >
              Capture
            </button>
          {:else}
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {primaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onAdd)}
              disabled={Boolean(actions?.addDisabled) || typeof actions?.onAdd !== 'function'}
              data-testid="mesh-node-detail-add"
            >
              Add to Team
            </button>
          {/if}
        </div>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-10 pt-7">
      <div class="mx-auto flex w-full max-w-[640px] flex-col gap-6">
        {#if focusArea}
          <section class="rounded-[24px] border px-5 py-4 {focusCardTone}" data-testid="mesh-node-detail-focus-card">
            <div data-testid="mesh-node-detail-focus-area">
              <MarkdownRenderer source={focusArea} {dark} codeTheme={codeTheme} />
            </div>
          </section>
        {/if}

        {#if contextMarkdown}
          <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-context-summary">
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">Context Summary</h3>
            <div class="{t.textPrimary}">
              <MarkdownRenderer source={contextMarkdown} {dark} codeTheme={codeTheme} />
            </div>
          </section>
        {/if}

        {#if behaviorMarkdown}
          <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-role-section">
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">Behavior Boundaries</h3>
            <div class="rounded-[20px] border px-5 py-4 {configTone}" data-testid="mesh-node-detail-behavior-summary">
              <MarkdownRenderer source={behaviorMarkdown} {dark} codeTheme={codeTheme} />
            </div>
          </section>
        {/if}

        {#if instructionsMarkdown}
          <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-description">
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">
              {normalizedContext === 'runtime' ? 'Operational Notes' : 'Instructions'}
            </h3>
            <div class="{t.textPrimary}">
              <MarkdownRenderer source={instructionsMarkdown} {dark} codeTheme={codeTheme} />
            </div>
          </section>
        {/if}

        {#if configurationEntries.length > 0}
          <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid={normalizedContext === 'runtime' ? 'mesh-node-detail-runtime' : 'mesh-node-detail-configuration'}>
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">Configuration</h3>
            <dl class="rounded-[20px] border px-5 py-4 {configTone}">
              {#each configurationEntries as entry}
                <div class="grid grid-cols-[116px_minmax(0,1fr)] gap-3 py-1.5 text-[14px]">
                  <dt class="{t.textMuted}">{entry.label}</dt>
                  <dd
                    class="min-w-0 break-words font-medium {t.textPrimary}"
                    data-testid={entry.testId}
                    title={entry.value}
                  >
                    {entry.value}
                  </dd>
                </div>
              {/each}
            </dl>
          </section>
        {/if}
      </div>
    </div>
  </div>
</div>
