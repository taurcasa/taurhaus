<script>
  import {
    focusFirstInteractiveElement,
    handleModalKeydown,
    registerModalLayer,
  } from '../a11y.js'
  import { defaultModelForTool, MODEL_OPTIONS_BY_TOOL, normalizeTool } from '../meshDefaults.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    open = false,
    dark = false,
    role = null,
    saving = false,
    errorMessage = '',
    onSave = () => {},
    onCancel = () => {},
  } = $props()

  let dialogEl = $state(null)
  let modalRootEl = $state(null)
  let closeButtonEl = $state(null)
  let restoreFocusElement = null

  let name = $state('')
  let roleId = $state('')
  let kind = $state('agent')
  let tool = $state('codex')
  let model = $state(defaultModelForTool('codex'))
  let focusArea = $state('')
  let contextSummary = $state('')
  let behaviorSummary = $state('')
  let instructions = $state('')
  let manualId = $state(false)

  const t = $derived(themeTokens(dark))
  const isExisting = $derived(Boolean(role?.roleId))
  const dialogTitle = $derived(isExisting ? 'Edit Role' : 'Create Role')
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
  const sectionTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.025] shadow-[inset_0_1px_0_rgba(255,255,255,0.03),0_16px_36px_rgba(0,0,0,0.16)]'
      : 'border-zinc-200/90 bg-white shadow-[0_14px_34px_rgba(15,23,42,0.06)]'
  )
  const inputTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/60 text-zinc-100 placeholder-zinc-500'
      : 'border-brand-200/60 bg-white text-zinc-900 placeholder-zinc-400'
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
  const closeTone = $derived(
    dark
      ? 'border-white/[0.08] bg-white/[0.03] text-zinc-400 hover:bg-white/[0.08] hover:text-zinc-100'
      : 'border-zinc-200 bg-white/90 text-zinc-500 hover:bg-black/5 hover:text-zinc-900'
  )
  const canSave = $derived(
    !saving
      && String(name ?? '').trim().length > 0
      && String(roleId ?? '').trim().length > 0
      && String(tool ?? '').trim().length > 0
      && String(model ?? '').trim().length > 0
  )
  const modelOptions = $derived.by(() => {
    const options = MODEL_OPTIONS_BY_TOOL[normalizeTool(tool)] ?? []
    if (model && !options.includes(model)) {
      return [model, ...options]
    }
    return options
  })

  function slugify(value) {
    return String(value ?? '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '')
  }

  function optionalValue(value) {
    const normalized = String(value ?? '').trim()
    return normalized.length > 0 ? normalized : null
  }

  function invoke(handler) {
    if (typeof handler === 'function') handler()
  }

  function close() {
    invoke(onCancel)
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

  function handleNameInput(event) {
    name = event.currentTarget.value
    if (!manualId && !isExisting) {
      roleId = slugify(name)
    }
  }

  function handleRoleIdInput(event) {
    roleId = event.currentTarget.value
    manualId = true
  }

  function handleToolChange(event) {
    tool = normalizeTool(event.currentTarget.value || 'codex')
    model = defaultModelForTool(tool)
  }

  function handleKindChange(event) {
    kind = event.currentTarget.value === 'lead' ? 'lead' : 'agent'
  }

  function submit() {
    if (!canSave) return
    onSave?.({
      roleId: String(roleId ?? '').trim(),
      name: String(name ?? '').trim(),
      kind: kind === 'lead' ? 'lead' : 'agent',
      tool: normalizeTool(tool || 'codex'),
      model: String(model ?? '').trim(),
      focusArea: optionalValue(focusArea),
      contextSummary: optionalValue(contextSummary),
      behaviorSummary: optionalValue(behaviorSummary),
      instructions: String(instructions ?? '').trim(),
    })
  }

  $effect(() => {
    if (!open) return

    if (role && typeof role === 'object') {
      name = String(role.name ?? '').trim()
      roleId = String(role.roleId ?? '').trim()
      kind = String(role.kind ?? 'agent').trim().toLowerCase() === 'lead' ? 'lead' : 'agent'
      tool = normalizeTool(role.tool ?? role.cliTool ?? role.defaults?.cliTool ?? 'codex')
      model = String(
        role.model ?? role.defaults?.model ?? defaultModelForTool(tool)
      ).trim()
      focusArea = String(role.focusArea ?? role.focus_area ?? '').trim()
      contextSummary = String(role.contextSummary ?? role.context_summary ?? '').trim()
      behaviorSummary = String(role.behaviorSummary ?? role.behavior_summary ?? '').trim()
      instructions = String(role.instructions ?? '').trim()
      manualId = true
      return
    }

    name = ''
    roleId = ''
    kind = 'agent'
    tool = 'codex'
    model = defaultModelForTool('codex')
    focusArea = ''
    contextSummary = ''
    behaviorSummary = ''
    instructions = ''
    manualId = false
  })

  $effect(() => {
    if (!open || !dialogEl || !modalRootEl) return
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

{#if open}
  <div
    bind:this={modalRootEl}
    class="fixed inset-0 z-40 flex items-center justify-center bg-black/62 p-4 backdrop-blur-[6px]"
    data-testid="mesh-role-editor-host"
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
      aria-label={dialogTitle}
      tabindex="-1"
      data-testid="mesh-role-editor"
    >
      <button
        bind:this={closeButtonEl}
        class="absolute right-6 top-6 z-20 inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-full border transition {closeTone}"
        type="button"
        aria-label="Close role editor"
        onclick={close}
        data-testid="mesh-role-editor-close"
      >
        <span aria-hidden="true" class="text-lg leading-none">×</span>
      </button>

      <div class="sticky top-0 z-10 border-b px-6 pb-5 pt-6 backdrop-blur {toolbarTone}">
        <div class="mx-auto w-full max-w-[640px] pr-12">
          <p class="text-[11px] font-medium uppercase tracking-[0.18em] {t.textMuted}">
            Role Catalog
          </p>
          <div class="mt-2 flex items-start justify-between gap-4">
            <div class="min-w-0 flex-1">
              <h2 class="truncate text-[36px] font-semibold leading-none" data-testid="mesh-role-editor-title">
                {dialogTitle}
              </h2>
              <p class="mt-3 text-[13px] leading-relaxed {t.textSecondary}">
                Shape the role prompt, defaults, and context without leaving the builder.
              </p>
            </div>
          </div>

          <div class="mt-5 flex flex-wrap items-center gap-2" data-testid="mesh-role-editor-toolbar">
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {secondaryActionTone}"
              type="button"
              onclick={close}
              data-testid="mesh-role-editor-cancel"
            >
              Cancel
            </button>
            <button
              class="inline-flex h-10 items-center gap-2 rounded-xl border px-4 text-[13px] font-medium transition {primaryActionTone}"
              type="button"
              onclick={submit}
              disabled={!canSave}
              data-testid="mesh-role-editor-save"
            >
              {saving ? 'Saving...' : 'Save Role'}
            </button>
          </div>
        </div>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-10 pt-7">
        <div class="mx-auto flex w-full max-w-[640px] flex-col gap-6">
          {#if errorMessage}
            <section class="rounded-[20px] border px-5 py-4 {sectionTone}" data-testid="mesh-role-editor-error">
              <p class="text-[13px] font-medium text-danger-500">{errorMessage}</p>
            </section>
          {/if}

          <section class="space-y-4 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-role-editor-basics">
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">Basics</h3>

            <div class="grid gap-4 sm:grid-cols-2">
              <label class="space-y-1.5">
                <span class="text-[11px] font-medium {t.textMuted}">Name</span>
                <input
                  class="h-11 w-full rounded-[16px] border px-3 text-[14px] outline-none {inputTone}"
                  value={name}
                  oninput={handleNameInput}
                  data-testid="mesh-role-editor-name-input"
                />
              </label>

              <label class="space-y-1.5">
                <span class="text-[11px] font-medium {t.textMuted}">Role ID</span>
                <input
                  class="h-11 w-full rounded-[16px] border px-3 text-[14px] outline-none {inputTone} {isExisting ? 'cursor-not-allowed opacity-60' : ''}"
                  value={roleId}
                  oninput={handleRoleIdInput}
                  disabled={isExisting}
                  data-testid="mesh-role-editor-id-input"
                />
              </label>
            </div>

            <div class="grid gap-4 sm:grid-cols-2">
              <label class="space-y-1.5">
                <span class="text-[11px] font-medium {t.textMuted}">Kind</span>
                <select
                  class="h-11 w-full rounded-[16px] border px-3 text-[14px] outline-none {inputTone}"
                  value={kind}
                  onchange={handleKindChange}
                  data-testid="mesh-role-editor-kind-input"
                >
                  <option value="agent">Agent</option>
                  <option value="lead">Lead</option>
                </select>
              </label>

              <label class="space-y-1.5">
                <span class="text-[11px] font-medium {t.textMuted}">Tool</span>
                <select
                  class="h-11 w-full rounded-[16px] border px-3 text-[14px] outline-none {inputTone}"
                  value={tool}
                  onchange={handleToolChange}
                  data-testid="mesh-role-editor-tool-input"
                >
                  <option value="claude">Claude</option>
                  <option value="codex">Codex</option>
                  <option value="gemini">Gemini</option>
                </select>
              </label>
            </div>

            <label class="space-y-1.5">
              <span class="text-[11px] font-medium {t.textMuted}">Model</span>
              <select
                class="h-11 w-full rounded-[16px] border px-3 text-[14px] outline-none {inputTone}"
                bind:value={model}
                data-testid="mesh-role-editor-model-input"
              >
                {#each modelOptions as option}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            </label>
          </section>

          <section class="space-y-4 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-role-editor-context">
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">Context</h3>

            <label class="space-y-1.5">
              <span class="text-[11px] font-medium {t.textMuted}">Focus Area</span>
              <textarea
                class="min-h-24 w-full rounded-[16px] border px-3 py-3 text-[14px] outline-none {inputTone}"
                value={focusArea}
                oninput={(event) => {
                  focusArea = event.currentTarget.value
                }}
                data-testid="mesh-role-editor-focus-area-input"
              ></textarea>
            </label>

            <label class="space-y-1.5">
              <span class="text-[11px] font-medium {t.textMuted}">Context Summary</span>
              <textarea
                class="min-h-28 w-full rounded-[16px] border px-3 py-3 text-[14px] outline-none {inputTone}"
                value={contextSummary}
                oninput={(event) => {
                  contextSummary = event.currentTarget.value
                }}
                data-testid="mesh-role-editor-context-summary-input"
              ></textarea>
            </label>

            <label class="space-y-1.5">
              <span class="text-[11px] font-medium {t.textMuted}">Behavior Summary</span>
              <textarea
                class="min-h-28 w-full rounded-[16px] border px-3 py-3 text-[14px] outline-none {inputTone}"
                value={behaviorSummary}
                oninput={(event) => {
                  behaviorSummary = event.currentTarget.value
                }}
                data-testid="mesh-role-editor-behavior-summary-input"
              ></textarea>
            </label>
          </section>

          <section class="space-y-4 rounded-[24px] border px-5 py-5 {sectionTone}" data-testid="mesh-role-editor-instructions">
            <h3 class="text-[12px] font-semibold uppercase tracking-[0.16em] {t.textMuted}">Instructions</h3>

            <label class="space-y-1.5">
              <span class="text-[11px] font-medium {t.textMuted}">Prompt Body</span>
              <textarea
                class="min-h-52 w-full rounded-[16px] border px-3 py-3 font-mono text-[13px] outline-none {inputTone}"
                value={instructions}
                oninput={(event) => {
                  instructions = event.currentTarget.value
                }}
                data-testid="mesh-role-editor-instructions-input"
              ></textarea>
            </label>
          </section>
        </div>
      </div>
    </div>
  </div>
{/if}
