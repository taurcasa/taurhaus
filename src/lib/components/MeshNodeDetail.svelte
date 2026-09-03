<script>
  import { focusFirstInteractiveElement, handleModalKeydown, registerModalLayer } from '../a11y.js'
  import { activitySignal } from '../activitySignal.js'
  import { accountState } from '../accounts.svelte.js'
  import MarkdownRenderer from '../MarkdownRenderer.svelte'
  import AccountPicker from './AccountPicker.svelte'
  import ModelSelect from './ModelSelect.svelte'
  import { getModelCatalogContext } from '../context/ModelCatalogContext.js'
  import { normalizeTool } from '../meshDefaults.js'
  import { EMPTY_MODEL_CATALOG, defaultEffortFor, defaultModelFor } from '../modelCatalog.js'
  import { themeTokens } from '../themeTokens.js'
  import { toolLabel as registeredToolLabel, tools } from '../toolRegistry.js'
  import { exhaustedUsage } from '../usageWindows.js'

  let {
    node = {},
    mode = 'runtime',
    dark = false,
    editing = false,
    editDraft = null,
    saving = false,
    errorMessage = '',
    dirty = false,
    actions = {},
    anchor = null,
    modelCatalog = null,
    onVisible = () => {},
  } = $props()

  const modelCatalogContext = getModelCatalogContext()
  const catalog = $derived(modelCatalog ?? modelCatalogContext?.catalog ?? EMPTY_MODEL_CATALOG)

  let dialogEl = $state(null)
  let modalRootEl = $state(null)
  let closeButtonEl = $state(null)
  let titleInputEl = $state(null)
  let restoreFocusElement = null
  const t = $derived(themeTokens(dark))

  const normalizedContext = $derived(mode === 'runtime' ? 'runtime' : 'roster')
  const isEditing = $derived(normalizedContext === 'roster' && Boolean(editing))
  const modeOptions = ['implementation', 'review', 'research', 'coordination']
  const normalizedRole = $derived(String(node?.role ?? '').trim().toLowerCase() === 'lead' ? 'lead' : 'agent')
  const name = $derived.by(() => String(node?.name ?? '').trim())
  const roleName = $derived.by(() => String(node?.roleName ?? node?.role_name ?? '').trim())
  const titleLabel = $derived.by(() => {
    if (isEditing) {
      return String(editDraft?.name ?? '').trim() || 'Untitled role'
    }
    return roleName || name || 'Role detail'
  })
  const subjectLabel = $derived.by(() => {
    if (isEditing) return ''
    if (!roleName || !name || roleName === name) return ''
    return name
  })
  const tool = $derived.by(() => String(node?.tool ?? node?.cliTool ?? node?.cli_tool ?? 'claude').trim().toLowerCase())
  const editTool = $derived.by(() => normalizeTool(editDraft?.tool ?? node?.tool ?? node?.cliTool ?? 'claude'))
  const toolOptions = $derived(tools())
  const toolLabel = $derived.by(() => {
    const currentTool = isEditing ? editTool : tool
    const fallback = isEditing
      ? String(editDraft?.tool ?? 'Unknown')
      : String(node?.tool ?? 'Unknown')
    return registeredToolLabel(currentTool, fallback)
  })
  const model = $derived.by(() =>
    isEditing
      ? String(editDraft?.model ?? '').trim()
      : String(node?.model ?? node?.modelName ?? node?.model_name ?? '').trim()
  )
  const reasoningEffort = $derived.by(() =>
    String(
      (isEditing
        ? editDraft?.reasoningEffort ?? editDraft?.reasoning_effort
        : node?.reasoningEffort ?? node?.reasoning_effort) ?? ''
    ).trim()
  )
  const modelDisplay = $derived(
    model ? `${model}${reasoningEffort ? ` · ${reasoningEffort}` : ''}` : ''
  )
  const accountId = $derived.by(() => String(node?.accountId ?? node?.account_id ?? '').trim())
  const accountLabel = $derived.by(() =>
    String(node?.accountLabel ?? node?.account_label ?? accountId ?? '').trim()
  )
  const accountFallbackFrom = $derived.by(() =>
    String(node?.accountFallbackFrom ?? node?.account_fallback_from ?? '').trim()
  )
  const accountApplied = $derived(node?.accountApplied ?? node?.account_applied ?? null)
  const accountDisplay = $derived.by(() => {
    const actual = accountLabel || 'Account'
    if (accountFallbackFrom) return `was ${accountFallbackFrom} → now ${actual}`
    if (accountApplied === false) return `${actual} · not guaranteed`
    if (accountApplied === true) return `${actual} · applied`
    return accountLabel ? `${actual} · configured` : ''
  })
  const detectedAccount = $derived(
    accountState(tool).accounts.find((account) => account.id === accountId) ?? null
  )
  const detectedAccounts = $derived(accountState(tool).accounts ?? [])
  const exhaustedAccountReason = $derived(exhaustedUsage(detectedAccount?.usage))
  const toolDescriptor = $derived(toolOptions.find((descriptor) => descriptor.id === tool) ?? null)
  const canSwitchExhaustedAccount = $derived(
    normalizedContext === 'runtime' &&
      Boolean(exhaustedAccountReason) &&
      Boolean(toolDescriptor?.capabilities?.accountSelection) &&
      !toolDescriptor?.capabilities?.teamConfigNamespace &&
      detectedAccounts.some((account) => account.logged_in && account.id !== accountId) &&
      typeof actions?.onSwitchAccount === 'function'
  )
  let accountPickerOpen = $state(false)
  const accountHeadroom = $derived.by(() => {
    const readings = (detectedAccount?.usage?.windows ?? [])
      .map((window) => Number(window?.used_percentage ?? window?.usedPercentage))
      .filter(Number.isFinite)
    if (readings.length === 0) return null
    return Math.max(0, Math.min(100, 100 - Math.max(...readings)))
  })
  // The effort the lead attached to the current assignment. Distinct from the
  // launch effort in `modelDisplay`, and only ever set for a runtime node.
  const taskEffort = $derived.by(() =>
    String(node?.taskEffort ?? node?.task_effort ?? '').trim()
  )
  const taskEffortWhy = $derived.by(() =>
    String(node?.taskEffortWhy ?? node?.task_effort_why ?? '').trim()
  )
  const editKind = $derived.by(() =>
    String(editDraft?.kind ?? node?.role ?? 'agent').trim().toLowerCase() === 'lead' ? 'lead' : 'agent'
  )
  const projectId = $derived.by(() => String(node?.projectId ?? node?.project_id ?? '').trim())
  const projectLabel = $derived.by(() => String(node?.projectLabel ?? node?.project_label ?? '').trim())
  const projectDisplay = $derived(projectLabel || projectId || 'No project')
  const signal = $derived(activitySignal(node))
  const statusLabel = $derived.by(() => {
    if (isEditing) return editKind === 'lead' ? 'Lead' : 'Agent'
    if (normalizedContext !== 'runtime') return 'Template'
    return signal.label
  })
  const statusDotTone = $derived.by(() => {
    if (normalizedContext !== 'runtime') {
      return dark ? 'bg-zinc-400' : 'bg-zinc-400'
    }
    if (signal.level === 'working' || signal.level === 'active') return 'bg-emerald-300'
    if (signal.level === 'idle') return 'bg-amber-300'
    if (signal.level === 'uncertain') return 'bg-sky-300'
    return dark ? 'bg-zinc-500' : 'bg-zinc-300'
  })
  const focusArea = $derived.by(() =>
    isEditing
      ? String(editDraft?.focusArea ?? '').trim()
      : String(node?.focusArea ?? node?.focus_area ?? '').trim()
  )
  const contextSummary = $derived.by(() =>
    isEditing
      ? String(editDraft?.contextSummary ?? '').trim()
      : String(node?.contextSummary ?? node?.context_summary ?? '').trim()
  )
  const behaviorSummary = $derived.by(() =>
    isEditing
      ? String(editDraft?.behaviorSummary ?? '').trim()
      : String(node?.behaviorSummary ?? node?.behavior_summary ?? '').trim()
  )
  const instructions = $derived.by(() =>
    isEditing
      ? String(editDraft?.instructions ?? '').trim()
      : String(node?.instructions ?? node?.description ?? '').trim()
  )
  const communicationStyle = $derived.by(() =>
    isEditing
      ? String(editDraft?.communicationStyle ?? editDraft?.communication_style ?? '').trim()
      : String(node?.communicationStyle ?? node?.communication_style ?? '').trim()
  )
  const qualityGates = $derived.by(() =>
    isEditing
      ? editableStringList(editDraft?.qualityGates ?? editDraft?.quality_gates)
      : displayStringList(node?.qualityGates ?? node?.quality_gates)
  )
  const definitionOfDone = $derived.by(() =>
    isEditing
      ? editableStringList(editDraft?.definitionOfDone ?? editDraft?.definition_of_done)
      : displayStringList(node?.definitionOfDone ?? node?.definition_of_done)
  )
  const phaseScope = $derived.by(() =>
    isEditing
      ? editableStringList(editDraft?.phaseScope ?? editDraft?.phase_scope)
      : displayStringList(node?.phaseScope ?? node?.phase_scope)
  )
  const modeValue = $derived.by(() =>
    isEditing
      ? String(editDraft?.mode ?? '').trim()
      : String(node?.mode ?? '').trim()
  )
  const inheritsFrom = $derived.by(() =>
    isEditing
      ? String(editDraft?.inheritsFrom ?? editDraft?.inherits_from ?? '').trim()
      : String(node?.inheritsFrom ?? node?.inherits_from ?? '').trim()
  )
  const requiredArtifacts = $derived.by(() =>
    isEditing
      ? editableStringList(editDraft?.requiredArtifacts ?? editDraft?.required_artifacts)
      : displayStringList(node?.requiredArtifacts ?? node?.required_artifacts)
  )
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
  const instructionsVisible = $derived(
    !isEditing || Boolean(editDraft?.showInstructions) || String(editDraft?.instructions ?? '').trim().length > 0
  )

  const overlayTone = $derived(
    dark
      ? 'border-white/[0.09] bg-zinc-950/98 text-zinc-100 shadow-[0_42px_140px_rgba(0,0,0,0.6),0_18px_42px_rgba(2,10,12,0.36)]'
      : 'border-zinc-200 bg-white/98 text-zinc-900 shadow-[0_36px_110px_rgba(15,23,42,0.18),0_16px_36px_rgba(15,23,42,0.1)]'
  )
  const toolbarTone = $derived(
    dark
      ? 'border-white/[0.08] bg-[linear-gradient(180deg,rgba(24,24,27,0.97),rgba(24,24,27,0.92))] shadow-[0_16px_44px_rgba(0,0,0,0.24)]'
      : 'border-zinc-200/90 bg-[linear-gradient(180deg,rgba(255,255,255,0.98),rgba(244,244,245,0.96))] shadow-[0_12px_30px_rgba(15,23,42,0.08)]'
  )
  const editorSectionCardTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.04] shadow-[inset_0_1px_0_rgba(255,255,255,0.03),0_12px_28px_rgba(0,0,0,0.14)]'
      : 'border-zinc-200 bg-zinc-50/95 shadow-[inset_0_1px_0_rgba(255,255,255,0.96),0_12px_24px_rgba(15,23,42,0.05)]'
  )
  const focusCardTone = $derived.by(() =>
    isEditing
      ? dark
        ? 'border-white/[0.12] bg-brand-400/[0.13] shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_20px_46px_rgba(0,0,0,0.2)]'
        : 'border-brand-200 bg-brand-50/90 shadow-[inset_0_1px_0_rgba(255,255,255,0.95),0_18px_38px_rgba(15,23,42,0.08)]'
      : dark
        ? 'border-white/[0.1] bg-brand-400/[0.1] shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_18px_38px_rgba(0,0,0,0.18)]'
        : 'border-brand-200/80 bg-brand-50/75 shadow-[inset_0_1px_0_rgba(255,255,255,0.92),0_14px_30px_rgba(15,23,42,0.07)]'
  )
  const configTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.035] shadow-[inset_0_1px_0_rgba(255,255,255,0.04),0_12px_28px_rgba(0,0,0,0.12)]'
      : 'border-zinc-200 bg-zinc-50/90 shadow-[inset_0_1px_0_rgba(255,255,255,0.95),0_12px_24px_rgba(15,23,42,0.06)]'
  )
  const sectionTone = $derived.by(() =>
    isEditing
      ? dark
        ? 'border-white/[0.08] bg-white/[0.03] shadow-[inset_0_1px_0_rgba(255,255,255,0.04),0_20px_44px_rgba(0,0,0,0.18)]'
        : 'border-zinc-200 bg-zinc-50/82 shadow-[inset_0_1px_0_rgba(255,255,255,0.95),0_16px_34px_rgba(15,23,42,0.06)]'
      : dark
        ? 'border-white/[0.08] bg-white/[0.022] shadow-[inset_0_1px_0_rgba(255,255,255,0.03),0_18px_40px_rgba(0,0,0,0.16)]'
        : 'border-zinc-200 bg-zinc-50/70 shadow-[inset_0_1px_0_rgba(255,255,255,0.92),0_14px_30px_rgba(15,23,42,0.05)]'
  )
  const editableFieldTone = $derived(
    dark
      ? 'bg-transparent text-zinc-100 placeholder-zinc-500 caret-brand-300'
      : 'bg-transparent text-zinc-900 placeholder-zinc-400 caret-brand-500'
  )
  const selectPillTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.045] text-zinc-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]'
      : 'border-zinc-300 bg-white text-zinc-900 shadow-[inset_0_1px_0_rgba(255,255,255,0.95)]'
  )
  const subtleHintTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const secondaryActionTone = $derived(
    dark
      ? 'border-white/[0.1] bg-white/[0.03] text-zinc-100 hover:bg-white/[0.08]'
      : 'border-zinc-300 bg-white text-zinc-700 hover:bg-zinc-50'
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
      : 'border-zinc-200 bg-white text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900'
  )
  const shellMutedTone = $derived(t.textMuted)
  const shellSecondaryTone = $derived(t.textSecondary)
  const focusContentTone = $derived(t.textPrimary)
  const bodyTone = $derived(t.textBody)
  const sectionLabelTone = $derived(dark ? 'text-brand-300/85' : 'text-brand-700/90')
  const errorTextTone = $derived(dark ? 'text-danger-100' : 'text-danger-700')
  const codeTheme = $derived(dark ? 'github-dark' : 'github-light')
  const breadcrumbLabel = $derived.by(() => {
    if (normalizedContext === 'runtime') return 'Team Roster'
    if (isEditing) return 'Role Catalog › Editing'
    return 'Role Catalog'
  })
  const canFocusPane = $derived(typeof actions?.onFocusPane === 'function')

  const contextMarkdown = $derived(contextSummary)
  const instructionsMarkdown = $derived(instructions)
  const behaviorMarkdown = $derived.by(() => buildBehaviorMarkdown(behavioralContract, behaviorSummary))
  const configurationEntries = $derived.by(() => {
    const entries = [
      { label: 'Tool', value: toolLabel, testId: null },
      { label: 'Model', value: modelDisplay || 'Not specified', testId: null },
    ]

    if (accountDisplay) {
      entries.push({
        label: 'Account',
        value: accountDisplay,
        testId: 'mesh-node-detail-account',
        account: true,
      })
    }

    if (taskEffort) {
      entries.push({
        label: 'Task effort',
        value: taskEffort,
        testId: 'mesh-node-detail-task-effort',
        title: taskEffortWhy || taskEffort,
      })
    }

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

  function editableStringList(value) {
    if (Array.isArray(value)) {
      return value.map((entry) => String(entry ?? ''))
    }
    if (typeof value === 'string' && value.trim()) {
      return [value]
    }
    return []
  }

  function displayStringList(value) {
    return editableStringList(value)
      .map((entry) => entry.trim())
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

  function autoGrow(textarea) {
    if (!(textarea instanceof HTMLTextAreaElement)) return
    textarea.style.height = 'auto'
    textarea.style.height = `${Math.max(textarea.scrollHeight, 80)}px`
  }

  function autoGrowTextarea(node, _value) {
    const resize = () => autoGrow(node)
    resize()
    node.addEventListener('input', resize)

    return {
      update(_nextValue) {
        resize()
      },
      destroy() {
        node.removeEventListener('input', resize)
      },
    }
  }

  function invoke(handler) {
    if (typeof handler === 'function') handler()
  }

  function switchAccount(nextAccountId) {
    accountPickerOpen = false
    actions?.onSwitchAccount?.(nextAccountId)
  }

  function updateDraft(patch) {
    if (!isEditing || !editDraft || typeof actions?.onEditChange !== 'function') return
    actions.onEditChange({
      ...editDraft,
      ...patch,
    })
  }

  function handleToolChange(value) {
    const nextTool = normalizeTool(value || 'codex')
    const nextModel = defaultModelFor(catalog, nextTool)
    updateDraft({
      tool: nextTool,
      model: nextModel,
      reasoningEffort: defaultEffortFor(catalog, nextTool, nextModel),
    })
  }

  function handleSaveEdit() {
    invoke(actions?.onSaveEdit)
  }

  function updateDraftList(field, index, value) {
    if (!isEditing || !editDraft) return
    const nextEntries = editableStringList(editDraft?.[field])
    nextEntries[index] = value
    updateDraft({ [field]: nextEntries })
  }

  function addDraftListItem(field) {
    if (!isEditing || !editDraft) return
    updateDraft({
      [field]: [...editableStringList(editDraft?.[field]), ''],
    })
  }

  function removeDraftListItem(field, index) {
    if (!isEditing || !editDraft) return
    updateDraft({
      [field]: editableStringList(editDraft?.[field]).filter((_, entryIndex) => entryIndex !== index),
    })
  }

  function updateDraftCommaList(field, value) {
    updateDraft({
      [field]: String(value ?? '')
        .split(',')
        .map((entry) => entry.trim())
        .filter(Boolean),
    })
  }

  function close() {
    if (isEditing && typeof actions?.onCancelEdit === 'function') {
      invoke(actions?.onCancelEdit)
      return
    }
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
    if (isEditing && event.key === 'Escape') {
      event.preventDefault()
      invoke(actions?.onCancelEdit)
      return
    }
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
      focusFirstInteractiveElement(dialogEl, () => (isEditing ? titleInputEl : closeButtonEl))
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
    {#if !isEditing}
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
    {/if}

    <div
      class="sticky top-0 z-10 border-b px-6 pb-5 pt-6 backdrop-blur {toolbarTone}"
      data-testid="mesh-node-detail-header"
    >
      <div class="mx-auto w-full max-w-[640px] pr-12">
        <div class="flex items-start gap-4">
          <div class="min-w-0 flex-1 space-y-2">
            <p class="text-[11px] font-medium uppercase tracking-[0.18em] {shellMutedTone}">
              {breadcrumbLabel}
            </p>
            <div class="flex min-w-0 flex-wrap items-center gap-3">
              {#if isEditing}
                <input
                  bind:this={titleInputEl}
                  class="min-w-0 flex-1 bg-transparent text-[26px] font-bold leading-[1.15] tracking-[-0.02em] outline-none {editableFieldTone}"
                  value={titleLabel}
                  oninput={(event) => updateDraft({ name: event.currentTarget.value })}
                  data-testid="mesh-node-detail-name-input"
                />
              {:else}
                <h2 class="truncate text-[36px] font-semibold leading-none" data-testid="mesh-node-detail-name">
                  {titleLabel}
                </h2>
              {/if}
              {#if !isEditing && normalizedRole === 'lead'}
                <span class="inline-flex items-center rounded-full border px-2.5 py-1 text-[12px] font-semibold {leadBadgeTone}">
                  Lead
                </span>
              {/if}
              {#if !isEditing && modeValue}
                <span
                  class="inline-flex items-center rounded-full border px-2.5 py-1 text-[12px] font-medium {badgeTone}"
                  data-testid="mesh-node-detail-mode-badge"
                >
                  {modeValue.replace(/[_-]+/g, ' ')}
                </span>
              {/if}
            </div>
            {#if isEditing}
              <div class="mt-4 flex flex-wrap items-center justify-between gap-3" data-testid="mesh-node-detail-toolbar">
                <div class="flex flex-wrap items-center gap-2 text-[13px]">
                  <select
                    class="h-9 rounded-xl border px-3 outline-none {selectPillTone}"
                    value={editTool}
                    onchange={(event) => handleToolChange(event.currentTarget.value)}
                    data-testid="mesh-node-detail-tool-input"
                  >
                    {#each toolOptions as descriptor (descriptor.id)}
                      <option value={descriptor.id}>{descriptor.label}</option>
                    {/each}
                  </select>
                  <div class="w-[18rem] max-w-full">
                    <ModelSelect
                      tool={editTool}
                      {model}
                      reasoningEffort={reasoningEffort || null}
                      {catalog}
                      {dark}
                      compact
                      inputClass={selectPillTone}
                      testId="mesh-node-detail-model-input"
                      onchange={(next) =>
                        updateDraft({ model: next.model, reasoningEffort: next.reasoningEffort })}
                    />
                  </div>
                  <select
                    class="h-9 rounded-xl border px-3 outline-none {selectPillTone}"
                    value={editKind}
                    onchange={(event) => updateDraft({ kind: event.currentTarget.value })}
                    data-testid="mesh-node-detail-kind-input"
                  >
                    <option value="agent">Agent</option>
                    <option value="lead">Lead</option>
                  </select>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                  <button
                    class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
                    type="button"
                    onclick={() => invoke(actions?.onCancelEdit)}
                    data-testid="mesh-node-detail-cancel"
                  >
                    Cancel
                  </button>
                  <button
                    class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {primaryActionTone}"
                    type="button"
                    onclick={handleSaveEdit}
                    disabled={saving}
                    data-testid="mesh-node-detail-save"
                  >
                    {#if dirty}
                      <span class="inline-block h-2 w-2 rounded-full bg-emerald-300" data-testid="mesh-node-detail-unsaved-dot"></span>
                    {/if}
                    {saving ? 'Saving...' : 'Save Changes'}
                  </button>
                </div>
              </div>
            {:else}
              <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] {shellSecondaryTone}">
                <span class="inline-flex items-center gap-2" data-testid="mesh-node-detail-tool-model">
                  <span class="inline-block h-2 w-2 rounded-full {statusDotTone}" aria-hidden="true"></span>
                  {toolLabel}{modelDisplay ? ` · ${modelDisplay}` : ''}
                </span>
                <span class="inline-flex items-center gap-2">
                    <span class="inline-block h-2 w-2 rounded-full {statusDotTone}" aria-hidden="true"></span>
                  <span>{statusLabel}</span>
                </span>
                {#if subjectLabel}
                  <span class="{shellMutedTone}" data-testid="mesh-node-detail-subject">{subjectLabel}</span>
                {/if}
              </div>
              {/if}
          </div>
        </div>

        {#if !isEditing}
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
            {#if canSwitchExhaustedAccount}
              <button
                class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
                type="button"
                aria-label="Switch exhausted account"
                onclick={() => { accountPickerOpen = !accountPickerOpen }}
                data-testid="mesh-node-detail-switch-account"
              >
                Switch account…
              </button>
            {/if}
          {:else if isEditing}
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onCancelEdit)}
              data-testid="mesh-node-detail-cancel"
            >
              Cancel
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {primaryActionTone}"
              type="button"
              onclick={handleSaveEdit}
              disabled={saving}
              data-testid="mesh-node-detail-save"
            >
              {#if dirty}
                <span class="inline-block h-2 w-2 rounded-full bg-emerald-300" data-testid="mesh-node-detail-unsaved-dot"></span>
              {/if}
              {saving ? 'Saving...' : 'Save Changes'}
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
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onEdit)}
              disabled={typeof actions?.onEdit !== 'function'}
              data-testid="mesh-node-detail-edit"
            >
              Edit
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
              type="button"
              onclick={() => invoke(actions?.onExport)}
              disabled={Boolean(actions?.exportDisabled) || typeof actions?.onExport !== 'function'}
              data-testid="mesh-node-detail-export"
            >
              Export YAML
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {dangerActionTone}"
              type="button"
              onclick={() => invoke(actions?.onDelete)}
              disabled={Boolean(actions?.deleteDisabled) || typeof actions?.onDelete !== 'function'}
              data-testid="mesh-node-detail-delete"
            >
              Delete
            </button>
          {/if}
        </div>

        {#if accountPickerOpen}
          <div class="mt-3">
            <AccountPicker
              {tool}
              accounts={detectedAccounts}
              defaultAccountId={accountId}
              preselectedAccountId={accountId}
              {dark}
              skin="popover"
              showRemember={false}
              reason={{
                kind: exhaustedAccountReason?.kind,
                accountLabel: accountLabel,
                windowTitle: exhaustedAccountReason?.window?.title,
                resetsAt: exhaustedAccountReason?.window?.resets_at ?? exhaustedAccountReason?.window?.resetsAt,
              }}
              onConfirm={switchAccount}
              onCancel={() => { accountPickerOpen = false }}
            />
          </div>
        {/if}
        {/if}
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-10 pt-7">
      <div class="mx-auto flex w-full max-w-[640px] flex-col gap-6">
        {#if isEditing && errorMessage}
          <section class="rounded-[20px] border px-5 py-4 {sectionTone}" data-testid="mesh-node-detail-error">
            <p class="text-[13px] font-medium {errorTextTone}">{errorMessage}</p>
          </section>
        {/if}

        {#if isEditing || focusArea}
          {#if isEditing}
            <label class="block space-y-2.5" data-testid="mesh-node-detail-focus-card">
              <span class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Focus Area</span>
              <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
                <textarea
                  use:autoGrowTextarea={focusArea}
                  rows="2"
                  class="min-h-[80px] w-full resize-none overflow-hidden border-0 bg-transparent p-0 text-[15px] font-normal leading-[1.65] outline-none {editableFieldTone}"
                  value={focusArea}
                  oninput={(event) => updateDraft({ focusArea: event.currentTarget.value })}
                  data-testid="mesh-node-detail-focus-input"
                ></textarea>
              </div>
            </label>
            {:else}
            <section class="rounded-[24px] border px-5 py-4 {focusCardTone}" data-testid="mesh-node-detail-focus-card">
              <div class="{focusContentTone}" data-testid="mesh-node-detail-focus-area">
                <MarkdownRenderer source={focusArea} {dark} codeTheme={codeTheme} />
              </div>
            </section>
            {/if}
        {/if}

        {#if isEditing || contextMarkdown}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-context-summary">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Context Summary</h3>
              <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
                <textarea
                  use:autoGrowTextarea={contextSummary}
                  rows="4"
                  class="min-h-[80px] w-full resize-none overflow-hidden border-0 bg-transparent p-0 text-[15px] font-normal leading-[1.65] outline-none {editableFieldTone}"
                  value={contextSummary}
                  oninput={(event) => updateDraft({ contextSummary: event.currentTarget.value })}
                  data-testid="mesh-node-detail-context-input"
                ></textarea>
              </div>
            </section>
            {:else}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-context-summary">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Context Summary</h3>
              <div class="{bodyTone}">
                <MarkdownRenderer source={contextMarkdown} {dark} codeTheme={codeTheme} />
              </div>
            </section>
            {/if}
        {/if}

        {#if isEditing || behaviorMarkdown}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-role-section">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Behavior Boundaries</h3>
              <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}" data-testid="mesh-node-detail-behavior-summary">
                <textarea
                  use:autoGrowTextarea={behaviorSummary}
                  rows="4"
                  class="min-h-[80px] w-full resize-none overflow-hidden border-0 bg-transparent p-0 text-[15px] font-normal leading-[1.65] outline-none {editableFieldTone}"
                  value={behaviorSummary}
                  oninput={(event) => updateDraft({ behaviorSummary: event.currentTarget.value })}
                  data-testid="mesh-node-detail-behavior-input"
                ></textarea>
              </div>
              <p class="text-[12px] {subtleHintTone}" data-testid="mesh-node-detail-markdown-hint">Supports markdown</p>
            </section>
            {:else}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-role-section">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Behavior Boundaries</h3>
              <div class="rounded-[20px] border px-5 py-4 {configTone}" data-testid="mesh-node-detail-behavior-summary">
                <MarkdownRenderer source={behaviorMarkdown} {dark} codeTheme={codeTheme} />
              </div>
            </section>
          {/if}
        {/if}

        {#if isEditing || communicationStyle}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-communication-style">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Communication Style</h3>
              <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
                <textarea
                  use:autoGrowTextarea={communicationStyle}
                  rows="3"
                  class="min-h-[80px] w-full resize-none overflow-hidden border-0 bg-transparent p-0 text-[15px] font-normal leading-[1.65] outline-none {editableFieldTone}"
                  value={communicationStyle}
                  oninput={(event) => updateDraft({ communicationStyle: event.currentTarget.value })}
                  data-testid="mesh-node-detail-communication-style-input"
                ></textarea>
              </div>
            </section>
          {:else}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-communication-style">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Communication Style</h3>
              <p class="text-[14px] leading-[1.65] {bodyTone}" data-testid="mesh-node-detail-communication-style-body">
                {communicationStyle}
              </p>
            </section>
          {/if}
        {/if}

        {#if isEditing || qualityGates.length > 0}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-quality-gates">
            <div class="flex items-center justify-between gap-3">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Quality Gates</h3>
              <button
                class="inline-flex h-8 items-center rounded-xl border px-3 text-[12px] font-medium transition {secondaryActionTone}"
                type="button"
                onclick={() => addDraftListItem('qualityGates')}
                data-testid="mesh-node-detail-quality-gates-add"
              >
                + Add gate
              </button>
            </div>
            <div class="space-y-2.5 rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
              {#if qualityGates.length > 0}
                {#each qualityGates as gate, index}
                  <div class="flex items-start gap-2.5" data-testid={`mesh-node-detail-quality-gates-row-${index}`}>
                    <span class="mt-3 inline-block h-1.5 w-1.5 rounded-full bg-brand-400/80" aria-hidden="true"></span>
                    <input
                      class="min-w-0 flex-1 rounded-xl border px-3 py-2 text-[14px] outline-none {selectPillTone}"
                      type="text"
                      value={gate}
                      oninput={(event) => updateDraftList('qualityGates', index, event.currentTarget.value)}
                      data-testid={`mesh-node-detail-quality-gates-input-${index}`}
                    />
                    <button
                      class="inline-flex h-9 items-center rounded-xl border px-3 text-[12px] font-medium transition {secondaryActionTone}"
                      type="button"
                      onclick={() => removeDraftListItem('qualityGates', index)}
                      data-testid={`mesh-node-detail-quality-gates-remove-${index}`}
                    >
                      Remove
                    </button>
                  </div>
                {/each}
              {:else}
                <p class="text-[12px] {subtleHintTone}">Add the checks this role should pass before handoff.</p>
              {/if}
            </div>
            </section>
          {:else}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-quality-gates">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Quality Gates</h3>
              <ul class="space-y-2">
                {#each qualityGates as gate}
                  <li class="flex items-start gap-2 text-[14px] leading-[1.55] {bodyTone}">
                    <span class="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border {badgeTone}" aria-hidden="true">✓</span>
                    <span>{gate}</span>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        {/if}

        {#if isEditing || definitionOfDone.length > 0}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-definition-of-done">
            <div class="flex items-center justify-between gap-3">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Definition of Done</h3>
              <button
                class="inline-flex h-8 items-center rounded-xl border px-3 text-[12px] font-medium transition {secondaryActionTone}"
                type="button"
                onclick={() => addDraftListItem('definitionOfDone')}
                data-testid="mesh-node-detail-definition-of-done-add"
              >
                + Add item
              </button>
            </div>
            <div class="space-y-2.5 rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
              {#if definitionOfDone.length > 0}
                {#each definitionOfDone as item, index}
                  <div class="flex items-start gap-2.5" data-testid={`mesh-node-detail-definition-of-done-row-${index}`}>
                    <span class="mt-3 inline-block h-1.5 w-1.5 rounded-full bg-brand-400/80" aria-hidden="true"></span>
                    <input
                      class="min-w-0 flex-1 rounded-xl border px-3 py-2 text-[14px] outline-none {selectPillTone}"
                      type="text"
                      value={item}
                      oninput={(event) => updateDraftList('definitionOfDone', index, event.currentTarget.value)}
                      data-testid={`mesh-node-detail-definition-of-done-input-${index}`}
                    />
                    <button
                      class="inline-flex h-9 items-center rounded-xl border px-3 text-[12px] font-medium transition {secondaryActionTone}"
                      type="button"
                      onclick={() => removeDraftListItem('definitionOfDone', index)}
                      data-testid={`mesh-node-detail-definition-of-done-remove-${index}`}
                    >
                      Remove
                    </button>
                  </div>
                {/each}
              {:else}
                <p class="text-[12px] {subtleHintTone}">Capture the concrete exit conditions for this role.</p>
              {/if}
            </div>
            </section>
          {:else}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-definition-of-done">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Definition of Done</h3>
              <ul class="space-y-2">
                {#each definitionOfDone as item}
                  <li class="flex items-start gap-2 text-[14px] leading-[1.55] {bodyTone}">
                    <span class="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border {badgeTone}" aria-hidden="true">✓</span>
                    <span>{item}</span>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        {/if}

        {#if isEditing || phaseScope.length > 0}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-phase-scope">
            <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Phase Scope</h3>
            <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
              <input
                class="w-full rounded-xl border px-3 py-2 text-[14px] outline-none {selectPillTone}"
                type="text"
                value={phaseScope.join(', ')}
                placeholder="implementation, verification, release"
                oninput={(event) => updateDraftCommaList('phaseScope', event.currentTarget.value)}
                data-testid="mesh-node-detail-phase-scope-input"
              />
            </div>
            </section>
          {:else}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-phase-scope">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Phase Scope</h3>
              <div class="flex flex-wrap gap-2">
                {#each phaseScope as phase}
                  <span class="inline-flex items-center rounded-full border px-2.5 py-1 text-[12px] font-medium {badgeTone}">
                    {phase.replace(/[_-]+/g, ' ')}
                  </span>
                {/each}
              </div>
            </section>
          {/if}
        {/if}

        {#if isEditing}
          <section class="space-y-2.5" data-testid="mesh-node-detail-mode">
            <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Mode</h3>
            <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
              <select
                class="h-10 w-full rounded-xl border px-3 outline-none {selectPillTone}"
                value={modeValue}
                onchange={(event) => updateDraft({ mode: event.currentTarget.value })}
                data-testid="mesh-node-detail-mode-input"
              >
                <option value="">None</option>
                {#each modeOptions as option}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            </div>
          </section>

          <section class="space-y-2.5" data-testid="mesh-node-detail-inherits-from">
            <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Inherits From</h3>
            <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
              <input
                class="w-full rounded-xl border px-3 py-2 text-[14px] outline-none {selectPillTone}"
                type="text"
                value={inheritsFrom}
                placeholder="shared-role-id"
                oninput={(event) => updateDraft({ inheritsFrom: event.currentTarget.value })}
                data-testid="mesh-node-detail-inherits-from-input"
              />
            </div>
          </section>

          <section class="space-y-2.5" data-testid="mesh-node-detail-required-artifacts">
            <div class="flex items-center justify-between gap-3">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">Required Artifacts</h3>
              <button
                class="inline-flex h-8 items-center rounded-xl border px-3 text-[12px] font-medium transition {secondaryActionTone}"
                type="button"
                onclick={() => addDraftListItem('requiredArtifacts')}
                data-testid="mesh-node-detail-required-artifacts-add"
              >
                + Add artifact
              </button>
            </div>
            <div class="space-y-2.5 rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
              {#if requiredArtifacts.length > 0}
                {#each requiredArtifacts as artifact, index}
                  <div class="flex items-start gap-2.5" data-testid={`mesh-node-detail-required-artifacts-row-${index}`}>
                    <span class="mt-3 inline-block h-1.5 w-1.5 rounded-full bg-brand-400/80" aria-hidden="true"></span>
                    <input
                      class="min-w-0 flex-1 rounded-xl border px-3 py-2 text-[14px] outline-none {selectPillTone}"
                      type="text"
                      value={artifact}
                      oninput={(event) => updateDraftList('requiredArtifacts', index, event.currentTarget.value)}
                      data-testid={`mesh-node-detail-required-artifacts-input-${index}`}
                    />
                    <button
                      class="inline-flex h-9 items-center rounded-xl border px-3 text-[12px] font-medium transition {secondaryActionTone}"
                      type="button"
                      onclick={() => removeDraftListItem('requiredArtifacts', index)}
                      data-testid={`mesh-node-detail-required-artifacts-remove-${index}`}
                    >
                      Remove
                    </button>
                  </div>
                {/each}
              {:else}
                <p class="text-[12px] {subtleHintTone}">List the deliverables this role must leave behind.</p>
              {/if}
            </div>
          </section>
        {/if}

        {#if instructionsVisible}
          {#if isEditing}
            <section class="space-y-2.5" data-testid="mesh-node-detail-description">
              <h3 class="text-[11px] font-medium uppercase tracking-[0.16em] {sectionLabelTone}">
                {normalizedContext === 'runtime' ? 'Operational Notes' : 'Instructions'}
              </h3>
              <div class="rounded-[14px] border px-4 py-3 {editorSectionCardTone}">
                <textarea
                  use:autoGrowTextarea={instructions}
                  rows="4"
                  class="min-h-[80px] w-full resize-none overflow-hidden border-0 bg-transparent p-0 text-[15px] font-normal leading-[1.65] outline-none {editableFieldTone}"
                  value={instructions}
                  oninput={(event) => updateDraft({ instructions: event.currentTarget.value, showInstructions: true })}
                  data-testid="mesh-node-detail-instructions-input"
                ></textarea>
              </div>
            </section>
            {:else if instructionsMarkdown}
            <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-node-detail-description">
              <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">
                {normalizedContext === 'runtime' ? 'Operational Notes' : 'Instructions'}
              </h3>
              <div class="{bodyTone}">
                <MarkdownRenderer source={instructionsMarkdown} {dark} codeTheme={codeTheme} />
              </div>
            </section>
            {/if}
        {/if}

        {#if isEditing && !instructionsVisible}
          <button
            class="inline-flex w-full items-center justify-center gap-2 rounded-[14px] border px-4 py-3 text-[13px] font-medium transition {secondaryActionTone}"
            type="button"
            onclick={() => invoke(actions?.onAddSection)}
            data-testid="mesh-node-detail-add-section"
          >
            + Add section
          </button>
        {/if}

        {#if !isEditing && configurationEntries.length > 0}
          <section class="space-y-3 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid={normalizedContext === 'runtime' ? 'mesh-node-detail-runtime' : 'mesh-node-detail-configuration'}>
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {sectionLabelTone}">Configuration</h3>
            <dl class="rounded-[20px] border px-5 py-4 {configTone}">
              {#each configurationEntries as entry}
                <div class="grid grid-cols-[116px_minmax(0,1fr)] gap-3 py-1.5 text-[14px]">
                  <dt class="{shellMutedTone}">{entry.label}</dt>
                  <dd
                    class="min-w-0 break-words font-medium {bodyTone}"
                    data-testid={entry.testId}
                    title={entry.title ?? entry.value}
                  >
                    {#if entry.account}
                      <span class="inline-flex items-center gap-2">
                        <span
                          class="h-1.5 w-8 overflow-hidden rounded-full bg-zinc-300/50"
                          data-testid="mesh-node-detail-account-meter"
                        >
                          <span
                            class="block h-full rounded-full bg-brand-500"
                            style={`width: ${accountHeadroom ?? 100}%`}
                          ></span>
                        </span>
                        <span>{entry.value}</span>
                      </span>
                    {:else}
                      {entry.value}
                    {/if}
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
