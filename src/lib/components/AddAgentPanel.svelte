<script>
  import { getRoleTemplate, listRoleTemplates } from '../ipc.js'
  import { normalizeProjectOption } from '../projectOptions.js'
  import { themeTokens } from '../themeTokens.js'
  import SlideOver from './SlideOver.svelte'

  let {
    open = false,
    dark = false,
    availableProjects = [],
    onClose = () => {},
    onAddAgent = () => {},
  } = $props()

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3-codex', 'gpt-5-mini'],
    gemini: ['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  const t = $derived(themeTokens(dark))
  const tabBase = 'px-2 py-1 text-xs border-b-2 transition-colors'
  const tabActive = $derived(`font-medium ${t.textPrimary} border-brand-500`)
  const tabInactive = $derived(`${t.textMuted} border-transparent hover:text-zinc-500`)
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
      : 'border-zinc-300 bg-white text-zinc-900 placeholder:text-zinc-400'
  )
  const cardTone = $derived(
    dark
      ? 'border-zinc-700/70 bg-zinc-900/70 hover:border-brand-700/60'
      : 'border-zinc-200 bg-white hover:border-brand-300'
  )
  const ghostTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )

  let mode = $state('template')
  let templatesLoading = $state(false)
  let templatesError = $state('')
  let roleTemplates = $state([])
  let selectedRoleId = $state('')
  let submitting = $state(false)
  let successMessage = $state('')

  let name = $state('')
  let tool = $state('codex')
  let model = $state(modelOptionsByTool.codex[0])
  let projectId = $state('')
  let description = $state('')

  const projectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
  )

  const canSubmit = $derived(
    !submitting &&
      name.trim().length > 0 &&
      tool.trim().length > 0 &&
      model.trim().length > 0 &&
      projectId.trim().length > 0
  )

  function resetForm() {
    name = ''
    tool = 'codex'
    model = modelOptionsByTool.codex[0]
    projectId = projectOptions[0]?.id ?? ''
    description = ''
    selectedRoleId = ''
    successMessage = ''
  }

  function modelsForTool(selectedTool) {
    return modelOptionsByTool[selectedTool] ?? ['default']
  }

  function setTool(nextTool) {
    tool = nextTool
    model = modelsForTool(nextTool)[0] ?? ''
  }

  function normalizeRoleTemplate(value) {
    return {
      roleId: value?.roleId ?? value?.role_id ?? '',
      name: value?.name ?? '',
      cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
      model: value?.model ?? '',
      capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
    }
  }

  async function loadRoleTemplates() {
    templatesLoading = true
    templatesError = ''
    try {
      const roles = await listRoleTemplates()
      roleTemplates = (roles ?? []).map(normalizeRoleTemplate)
    } catch (error) {
      roleTemplates = []
      templatesError = error?.message || 'Failed to load role templates.'
    } finally {
      templatesLoading = false
    }
  }

  async function applyRoleTemplate(role) {
    selectedRoleId = role.roleId
    name = role.name || role.roleId
    setTool(role.cliTool)
    model = role.model || modelsForTool(role.cliTool)[0] || ''

    try {
      const detail = await getRoleTemplate(role.roleId)
      description = detail?.instructions ? String(detail.instructions) : description
    } catch {
      description = description || ''
    }
  }

  async function submit() {
    if (!canSubmit) return

    submitting = true
    const payload = {
      name: name.trim(),
      tool,
      model: model.trim(),
      projectId: projectId.trim(),
      description: description.trim(),
    }

    try {
      await onAddAgent(payload)
      successMessage = `Added ${payload.name}`
      resetForm()
      onClose()
    } finally {
      submitting = false
    }
  }

  $effect(() => {
    if (!open) return
    mode = 'template'
    successMessage = ''
    if (!projectId) {
      projectId = projectOptions[0]?.id ?? ''
    }
    void loadRoleTemplates()
  })
</script>

<SlideOver {open} title="Add Agent" {dark} onClose={onClose}>
  {#snippet children()}
    <section class="space-y-3" data-testid="add-agent-panel">
      <div class="flex items-center gap-1.5 border-b pb-1 {t.keyline}">
        <button
          class="{tabBase} {mode === 'template' ? tabActive : tabInactive}"
          onclick={() => (mode = 'template')}
          data-testid="add-agent-tab-template"
        >
          From Template
        </button>
        <button
          class="{tabBase} {mode === 'manual' ? tabActive : tabInactive}"
          onclick={() => (mode = 'manual')}
          data-testid="add-agent-tab-manual"
        >
          Manual
        </button>
      </div>

      {#if mode === 'template'}
        <section class="space-y-2" data-testid="add-agent-template-section">
          {#if templatesLoading}
            <p class="text-xs {t.textMuted}" data-testid="add-agent-templates-loading">Loading templates...</p>
          {:else if templatesError}
            <p class="text-xs text-danger-500">{templatesError}</p>
          {:else if roleTemplates.length === 0}
            <div class="rounded-md border p-2 {cardTone}" data-testid="add-agent-no-templates">
              <p class="text-xs {t.textMuted}">No role templates found.</p>
              <button
                class="mt-2 rounded border px-2 py-1 text-[11px] {ghostTone}"
                onclick={() => (mode = 'manual')}
                data-testid="add-agent-switch-manual"
              >
                Switch to manual
              </button>
            </div>
          {:else}
            <div class="space-y-1.5 max-h-[170px] overflow-y-auto" data-testid="add-agent-template-list">
              {#each roleTemplates as role}
                <button
                  class="w-full rounded-md border px-2 py-1.5 text-left transition-colors {cardTone}
                    {selectedRoleId === role.roleId ? 'ring-1 ring-brand-500/60' : ''}"
                  onclick={() => {
                    void applyRoleTemplate(role)
                  }}
                  data-testid={`add-agent-template-${role.roleId}`}
                >
                  <p class="text-[12px] font-medium {t.textPrimary} truncate">{role.name}</p>
                  <p class="text-[10px] {t.textMuted}">{role.roleId} · {role.cliTool} · {role.model}</p>
                </button>
              {/each}
            </div>
          {/if}
        </section>
      {/if}

      <section class="space-y-2" data-testid="add-agent-form">
        <label class="block space-y-1">
          <span class="text-[11px] uppercase {t.textMuted}">Name</span>
          <input
            class="h-8 w-full rounded-md border px-2 text-xs {inputTone}"
            bind:value={name}
            placeholder="frontend-dev"
            data-testid="add-agent-name-input"
          />
        </label>

        <div class="grid grid-cols-2 gap-2">
          <label class="block space-y-1">
            <span class="text-[11px] uppercase {t.textMuted}">Tool</span>
            <select
              class="h-8 w-full rounded-md border px-2 text-xs {inputTone}"
              value={tool}
              onchange={(event) => {
                setTool(event.currentTarget.value)
              }}
              data-testid="add-agent-tool-select"
            >
              <option value="claude">claude</option>
              <option value="codex">codex</option>
              <option value="gemini">gemini</option>
            </select>
          </label>

          <label class="block space-y-1">
            <span class="text-[11px] uppercase {t.textMuted}">Model</span>
            <select
              class="h-8 w-full rounded-md border px-2 text-xs {inputTone}"
              bind:value={model}
              data-testid="add-agent-model-select"
            >
              {#each modelsForTool(tool) as modelOption}
                <option value={modelOption}>{modelOption}</option>
              {/each}
            </select>
          </label>
        </div>

        <label class="block space-y-1">
          <span class="text-[11px] uppercase {t.textMuted}">Project</span>
          <select
            class="h-8 w-full rounded-md border px-2 text-xs {inputTone}"
            bind:value={projectId}
            data-testid="add-agent-project-select"
          >
            <option value="">Select project</option>
            {#each projectOptions as project}
              <option value={project.id}>{project.label}</option>
            {/each}
          </select>
        </label>

        <label class="block space-y-1">
          <span class="text-[11px] uppercase {t.textMuted}">Description</span>
          <textarea
            class="w-full rounded-md border px-2 py-1.5 text-xs {inputTone}"
            rows="3"
            bind:value={description}
            placeholder="Optional responsibility summary"
            data-testid="add-agent-description-input"
          ></textarea>
        </label>

        {#if mode === 'template' && selectedRoleId}
          <section class="rounded-md border px-2 py-1.5 {cardTone}" data-testid="add-agent-template-preview">
            <p class="text-[11px] uppercase {t.textMuted}">Preview</p>
            <p class="text-[12px] {t.textPrimary}">{name || 'Unnamed agent'} · {tool} · {model}</p>
            <p class="text-[11px] {t.textMuted} truncate">{projectId || 'No project selected'}</p>
          </section>
        {/if}
      </section>

      <footer class="flex items-center justify-between gap-2 border-t pt-2 {t.keyline}">
        <p class="text-xs text-success-500 min-h-4" data-testid="add-agent-success">{successMessage}</p>
        <div class="flex items-center gap-2">
          <button
            class="rounded-md border px-2 py-1 text-[11px] {ghostTone}"
            onclick={onClose}
            disabled={submitting}
            data-testid="add-agent-cancel"
          >
            Cancel
          </button>
          <button
            class="rounded-md bg-brand-600 px-2 py-1 text-[11px] font-medium text-white hover:bg-brand-700 disabled:opacity-50 disabled:cursor-not-allowed"
            onclick={submit}
            disabled={!canSubmit}
            data-testid="add-agent-submit"
          >
            {submitting ? 'Adding...' : 'Add Agent'}
          </button>
        </div>
      </footer>
    </section>
  {/snippet}
</SlideOver>
