<script>
  import SlideOver from './SlideOver.svelte'
  import { themeTokens } from '../themeTokens.js'

  let {
    open = false,
    dark = false,
    role = null,
    onSave = () => {},
    onCancel = () => {},
    onDelete = () => {}
  } = $props()

  const t = $derived(themeTokens(dark))

  const modelOptionsByTool = {
    claude: ['claude-opus-4-6', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001'],
    codex: ['gpt-5.3-codex', 'gpt-5.2', 'gpt-4o'],
    gemini: ['gemini-3.1-pro', 'gemini-2.5-pro', 'gemini-2.0-flash'],
  }

  const toolOptions = ['claude', 'codex', 'gemini']

  let name = $state('')
  let roleId = $state('')
  let tool = $state('claude')
  let model = $state('claude-opus-4-6')
  let instructions = $state('')
  let behavioralContract = $state([])
  let capabilities = $state([])

  let newRule = $state('')
  let newCapability = $state('')
  let manualId = $state(false)

  const isBuiltIn = $derived(role?.builtIn || false)
  const isExisting = $derived(role !== null)

  const canSave = $derived(
    name.trim().length > 0 &&
    roleId.trim().length > 0 &&
    tool.length > 0 &&
    model.length > 0
  )

  const inputTone = $derived(
    dark
      ? 'bg-zinc-900 border-zinc-700 text-zinc-100 placeholder-zinc-600 focus:border-brand-500'
      : 'bg-white border-zinc-300 text-zinc-900 placeholder-zinc-400 focus:border-brand-500'
  )

  const labelTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const addButtonTone = $derived(
    dark
      ? 'bg-zinc-800 text-zinc-100 hover:bg-zinc-700'
      : 'bg-zinc-100 text-zinc-700 hover:bg-zinc-200'
  )
  function slugify(text) {
    return text
      .toString()
      .toLowerCase()
      .trim()
      .replace(/\s+/g, '-')
      .replace(/[^\w-]+/g, '')
      .replace(/--+/g, '-')
  }

  function handleNameInput(e) {
    name = e.target.value
    if (!manualId && !isExisting) {
      roleId = slugify(name)
    }
  }

  function handleIdInput(e) {
    roleId = e.target.value
    manualId = true
  }

  function handleToolChange(e) {
    tool = e.target.value
    model = modelOptionsByTool[tool][0]
  }

  function addRule() {
    if (newRule.trim()) {
      behavioralContract = [...behavioralContract, { rule: newRule.trim(), enabled: true }]
      newRule = ''
    }
  }

  function removeRule(index) {
    behavioralContract = behavioralContract.filter((_, i) => i !== index)
  }

  function toggleRule(index) {
    behavioralContract = behavioralContract.map((r, i) =>
      i === index ? { ...r, enabled: !r.enabled } : r
    )
  }

  function addCapability() {
    if (newCapability.trim()) {
      capabilities = [...capabilities, newCapability.trim()]
      newCapability = ''
    }
  }

  function removeCapability(index) {
    capabilities = capabilities.filter((_, i) => i !== index)
  }

  function handleSave() {
    if (!canSave) return
    onSave({
      roleId,
      name,
      tool,
      model,
      instructions,
      behavioralContract: JSON.parse(JSON.stringify(behavioralContract)),
      capabilities: [...capabilities]
    })
  }

  $effect(() => {
    if (open) {
      if (role) {
        name = role.name || ''
        roleId = role.roleId || ''
        tool = role.tool || 'claude'
        model = role.model || modelOptionsByTool[tool][0]
        instructions = role.instructions || ''
        behavioralContract = role.behavioralContract ? JSON.parse(JSON.stringify(role.behavioralContract)) : []
        capabilities = role.capabilities ? [...role.capabilities] : []
        manualId = true
      } else {
        name = ''
        roleId = ''
        tool = 'claude'
        model = 'claude-opus-4-6'
        instructions = ''
        behavioralContract = []
        capabilities = []
        manualId = false
      }
    }
  })
</script>

<SlideOver {open} title={isExisting ? 'Edit Role' : 'Create Role'} {dark} onClose={onCancel}>
  <div class="space-y-6" data-testid="role-editor-container">
    <!-- Basic Info -->
    <div class="space-y-4">
      <div class="grid grid-cols-2 gap-4">
        <div class="space-y-1">
          <label class="text-[10px] font-medium uppercase tracking-wider {labelTone}" for="role-name">
            Role Name
          </label>
          <input
            id="role-name"
            type="text"
            class="w-full h-9 px-3 rounded-md border text-sm transition-colors focus:outline-none {inputTone}"
            placeholder="e.g. Frontend Developer"
            value={name}
            oninput={handleNameInput}
            data-testid="role-editor-name-input"
          />
        </div>
        <div class="space-y-1">
          <label class="text-[10px] font-medium uppercase tracking-wider {labelTone}" for="role-id">
            Role ID
          </label>
          <input
            id="role-id"
            type="text"
            class="w-full h-9 px-3 rounded-md border text-sm transition-colors focus:outline-none {inputTone} {isExisting ? 'opacity-60 cursor-not-allowed' : ''}"
            placeholder="e.g. frontend-dev"
            value={roleId}
            oninput={handleIdInput}
            disabled={isExisting}
            data-testid="role-editor-id-input"
          />
        </div>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div class="space-y-1">
          <label class="text-[10px] font-medium uppercase tracking-wider {labelTone}" for="role-tool">
            Tool
          </label>
          <select
            id="role-tool"
            class="w-full h-9 px-2 rounded-md border text-sm transition-colors focus:outline-none {inputTone}"
            value={tool}
            onchange={handleToolChange}
            data-testid="role-editor-tool-select"
          >
            {#each toolOptions as opt}
              <option value={opt}>{opt.charAt(0).toUpperCase() + opt.slice(1)}</option>
            {/each}
          </select>
        </div>
        <div class="space-y-1">
          <label class="text-[10px] font-medium uppercase tracking-wider {labelTone}" for="role-model">
            Model
          </label>
          <select
            id="role-model"
            class="w-full h-9 px-2 rounded-md border text-sm transition-colors focus:outline-none {inputTone}"
            bind:value={model}
            data-testid="role-editor-model-select"
          >
            {#each modelOptionsByTool[tool] as opt}
              <option value={opt}>{opt}</option>
            {/each}
          </select>
        </div>
      </div>
    </div>

    <!-- Instructions -->
    <div class="space-y-1">
      <label class="text-[10px] font-medium uppercase tracking-wider {labelTone}" for="role-instructions">
        Instructions
      </label>
      <textarea
        id="role-instructions"
        class="w-full p-3 rounded-md border text-sm font-mono transition-colors focus:outline-none {inputTone}"
        rows="6"
        placeholder="Role instructions (Markdown supported)..."
        bind:value={instructions}
        data-testid="role-editor-instructions-input"
      ></textarea>
    </div>

    <!-- Behavioral Contract -->
    <div class="space-y-2">
      <div class="text-[10px] font-medium uppercase tracking-wider {labelTone}">
        Behavioral Contract
      </div>
      <div class="space-y-2">
        {#each behavioralContract as item, i}
          <div class="flex items-center gap-2 p-2 rounded border {dark ? 'border-zinc-800 bg-zinc-900/50' : 'border-zinc-100 bg-zinc-50/50'}">
            <input
              type="checkbox"
              checked={item.enabled}
              onchange={() => toggleRule(i)}
              class="w-4 h-4 rounded border-zinc-300 text-brand-600 focus:ring-brand-500"
              data-testid="role-rule-{i}-checkbox"
            />
            <span class="flex-1 text-xs {t.textBody}">{item.rule}</span>
            <button
              class="text-zinc-500 hover:text-red-500 transition-colors"
              onclick={() => removeRule(i)}
              data-testid="role-rule-{i}-remove"
              aria-label="Remove rule"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
            </button>
          </div>
        {/each}
        <div class="flex gap-2">
          <input
            type="text"
            class="flex-1 h-8 px-3 rounded-md border text-xs transition-colors focus:outline-none {inputTone}"
            placeholder="Add a rule..."
            bind:value={newRule}
            onkeydown={(e) => e.key === 'Enter' && addRule()}
            data-testid="role-editor-add-rule-input"
          />
          <button
            class="h-8 px-3 rounded-md text-xs transition-colors {addButtonTone}"
            onclick={addRule}
            data-testid="role-editor-add-rule-button"
          >
            Add
          </button>
        </div>
      </div>
    </div>

    <!-- Capabilities -->
    <div class="space-y-2">
      <div class="text-[10px] font-medium uppercase tracking-wider {labelTone}">
        Capabilities
      </div>
      <div class="space-y-2">
        <div class="flex flex-wrap gap-2">
          {#each capabilities as cap, i}
            <span class="inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium {dark ? 'bg-zinc-800 text-zinc-300' : 'bg-zinc-100 text-zinc-700'}">
              {cap}
              <button
                class="hover:text-red-500 transition-colors"
                onclick={() => removeCapability(i)}
                data-testid="role-capability-{i}-remove"
                aria-label="Remove capability"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
              </button>
            </span>
          {/each}
        </div>
        <div class="flex gap-2">
          <input
            type="text"
            class="flex-1 h-8 px-3 rounded-md border text-xs transition-colors focus:outline-none {inputTone}"
            placeholder="Add a capability tag..."
            bind:value={newCapability}
            onkeydown={(e) => e.key === 'Enter' && addCapability()}
            data-testid="role-editor-add-capability-input"
          />
          <button
            class="h-8 px-3 rounded-md text-xs transition-colors {addButtonTone}"
            onclick={addCapability}
            data-testid="role-editor-add-capability-button"
          >
            Add
          </button>
        </div>
      </div>
    </div>

    <!-- Actions -->
    <div class="flex items-center justify-between pt-4 border-t {dark ? 'border-zinc-800' : 'border-zinc-100'}">
      <div>
        {#if isExisting && !isBuiltIn}
          <button
            class="text-xs text-red-500 hover:text-red-400 font-medium transition-colors"
            onclick={() => onDelete(role.roleId)}
            data-testid="role-editor-delete"
          >
            Delete Role
          </button>
        {/if}
      </div>
      <div class="flex gap-3">
        <button
          class="px-4 py-2 rounded-md text-xs font-medium transition-colors {dark ? 'text-zinc-400 hover:text-zinc-200' : 'text-zinc-600 hover:text-zinc-900'}"
          onclick={onCancel}
          data-testid="role-editor-cancel"
        >
          Cancel
        </button>
        <button
          class="px-4 py-2 rounded-md bg-brand-600 text-white text-xs font-semibold hover:bg-brand-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          onclick={handleSave}
          disabled={!canSave}
          data-testid="role-editor-save"
        >
          Save Role
        </button>
      </div>
    </div>
  </div>
</SlideOver>
