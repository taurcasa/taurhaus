<script>
  import { getToolIcon, getToolName } from '../toolLogos.js'

  let {
    name = '',
    tool = 'claude',
    model = '',
    projectId = '',
    role = 'agent',
    description = '',
    editing = false,
    onSave = () => {},
    onRemove = () => {},
    dark = false,
    testId = 'agent-card',
  } = $props()

  const modelOptionsByTool = {
    claude: ['opus', 'sonnet', 'haiku'],
    codex: ['gpt-5.3-codex', 'gpt-5-mini'],
    gemini: ['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  const toolOptions = ['claude', 'codex', 'gemini']

  function draftFromValues(values = {}) {
    return {
      name: String(values.name ?? ''),
      tool: String(values.tool ?? 'claude').toLowerCase(),
      model: String(values.model ?? ''),
      projectId: String(values.projectId ?? ''),
      description: String(values.description ?? ''),
    }
  }

  let isEditing = $state(false)
  let draft = $state(draftFromValues())
  let initialized = false

  const isLead = $derived(String(role ?? '').toLowerCase() === 'lead')
  const icon = $derived(getToolIcon(draft.tool))
  const cardTone = $derived(
    dark
      ? 'border-[var(--agent-card-border-dark)] text-[var(--agent-card-text-dark)]'
      : 'border-[var(--agent-card-border-light)] text-[var(--agent-card-text-light)] shadow-[var(--agent-card-shadow-light)]'
  )
  const mutedTone = $derived(dark ? 'text-zinc-400' : 'text-brand-700')
  const nodeTone = $derived(
    dark
      ? 'bg-linear-to-b from-[var(--mesh-node-gradient-from)] to-[var(--mesh-node-gradient-to)]'
      : 'bg-linear-to-b from-[var(--agent-card-bg-light-from)] to-[var(--agent-card-bg-light-to)]'
  )
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500'
      : 'border-brand-200 bg-white text-brand-900 placeholder:text-brand-700/60'
  )
  const ghostTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800'
      : 'border-brand-200 text-brand-700 hover:bg-brand-50'
  )
  const labelTone = $derived(dark ? 'text-zinc-500' : 'text-brand-700')
  const normalizedTool = $derived(toolOptions.includes(draft.tool) ? draft.tool : 'claude')
  const modelOptions = $derived(modelOptionsByTool[normalizedTool] ?? modelOptionsByTool.claude)

  function resetDraft() {
    draft = draftFromValues({ name, tool, model, projectId, description })
  }

  function startEditing() {
    resetDraft()
    isEditing = true
  }

  function cancelEditing() {
    resetDraft()
    isEditing = false
  }

  function handleToolChange(value) {
    const nextTool = String(value || 'claude').toLowerCase()
    const nextModels = modelOptionsByTool[nextTool] ?? modelOptionsByTool.claude
    draft = {
      ...draft,
      tool: nextTool,
      model: nextModels.includes(draft.model) ? draft.model : nextModels[0],
    }
  }

  function handleSave() {
    const payload = {
      name: draft.name.trim(),
      tool: normalizedTool,
      model: draft.model.trim(),
      projectId: draft.projectId.trim(),
      description: draft.description.trim(),
    }
    onSave(payload)
    isEditing = false
  }

  $effect(() => {
    if (initialized) return
    initialized = true
    isEditing = Boolean(editing)
    resetDraft()
  })
</script>

<article
  class="rounded-lg border-b p-3 transition-all {cardTone} {nodeTone}"
  data-testid={testId}
>
  {#if !isEditing}
    <div class="flex items-center gap-2">
      <svg class="h-3 w-3 shrink-0 text-zinc-500" viewBox={icon.viewBox} fill="currentColor" aria-hidden="true">
        <path d={icon.path}></path>
      </svg>
      <div class="min-w-0">
        <p class="truncate text-[13px] font-semibold" data-testid={`${testId}-name`}>
          {name || (isLead ? 'team-lead' : 'Unnamed agent')}
        </p>
        <p class="truncate text-[11px] {mutedTone}" data-testid={`${testId}-tool-model`}>
          {getToolName(normalizedTool)} · {draft.model || model || 'model'}
        </p>
        <p class="truncate text-[11px] {mutedTone}" data-testid={`${testId}-project`}>
          {projectId || 'No project'}
        </p>
      </div>
      <div class="ml-auto flex items-center gap-1">
        <button
          class="rounded-md border px-2 py-1 text-xs transition-colors {ghostTone}"
          type="button"
          onclick={startEditing}
          data-testid={`${testId}-edit`}
        >
          Edit
        </button>
        {#if !isLead}
          <button
            class="rounded-md border px-2 py-1 text-xs transition-colors {ghostTone}"
            type="button"
            onclick={() => onRemove()}
            data-testid={`${testId}-remove`}
          >
            Remove
          </button>
        {/if}
      </div>
    </div>
  {:else}
    <div class="grid grid-cols-1 gap-2" data-testid={`${testId}-edit-form`}>
      <label class="space-y-1">
        <span class="text-[10px] font-medium uppercase tracking-wide {labelTone}">Name</span>
        <input
          class="w-full rounded-md border px-2 py-1.5 text-xs {inputTone}"
          value={draft.name}
          oninput={(event) => {
            draft = { ...draft, name: event.currentTarget.value }
          }}
          data-testid={`${testId}-name-input`}
        />
      </label>

      <div class="grid grid-cols-2 gap-2">
        <label class="space-y-1">
          <span class="text-[10px] font-medium uppercase tracking-wide {labelTone}">Tool</span>
          <select
            class="w-full rounded-md border px-2 py-1.5 text-xs {inputTone}"
            value={normalizedTool}
            onchange={(event) => {
              handleToolChange(event.currentTarget.value)
            }}
            data-testid={`${testId}-tool-select`}
          >
            {#each toolOptions as option}
              <option value={option}>{option}</option>
            {/each}
          </select>
        </label>

        <label class="space-y-1">
          <span class="text-[10px] font-medium uppercase tracking-wide {labelTone}">Model</span>
          <select
            class="w-full rounded-md border px-2 py-1.5 text-xs {inputTone}"
            value={draft.model || modelOptions[0]}
            onchange={(event) => {
              draft = { ...draft, model: event.currentTarget.value }
            }}
            data-testid={`${testId}-model-select`}
          >
            {#each modelOptions as option}
              <option value={option}>{option}</option>
            {/each}
          </select>
        </label>
      </div>

      <label class="space-y-1">
        <span class="text-[10px] font-medium uppercase tracking-wide {labelTone}">Project ID</span>
        <input
          class="w-full rounded-md border px-2 py-1.5 text-xs {inputTone}"
          value={draft.projectId}
          oninput={(event) => {
            draft = { ...draft, projectId: event.currentTarget.value }
          }}
          data-testid={`${testId}-project-input`}
        />
      </label>

      <label class="space-y-1">
        <span class="text-[10px] font-medium uppercase tracking-wide {labelTone}">Description</span>
        <textarea
          class="h-16 w-full rounded-md border px-2 py-1.5 text-xs {inputTone}"
          value={draft.description}
          oninput={(event) => {
            draft = { ...draft, description: event.currentTarget.value }
          }}
          data-testid={`${testId}-description-input`}
        ></textarea>
      </label>

      <div class="flex items-center justify-end gap-1.5">
        <button
          class="rounded-md border px-2 py-1 text-xs transition-colors {ghostTone}"
          type="button"
          onclick={cancelEditing}
          data-testid={`${testId}-cancel`}
        >
          Cancel
        </button>
        <button
          class="rounded-md bg-brand-600 px-2.5 py-1 text-xs font-medium text-white transition-colors hover:bg-brand-700"
          type="button"
          onclick={handleSave}
          data-testid={`${testId}-save`}
        >
          Save
        </button>
      </div>
    </div>
  {/if}
</article>
