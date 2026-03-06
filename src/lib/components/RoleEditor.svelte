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
    codex: ['gpt-5.4-high', 'gpt-5.3-codex', 'gpt-5.2', 'gpt-4o'],
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

  const cardTone = $derived(
    dark
      ? 'bg-white/[0.03] border-white/[0.06]'
      : 'bg-brand-50/50 border-brand-200/40'
  )

  const inputTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )

  const labelTone = 'text-zinc-500'
  const sectionHeaderTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

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
  <div class="space-y-4 pb-20" data-testid="role-editor-container">
    
    <!-- Basic Info Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
      <header class="mb-3">
        <h3 class="text-[10px] font-bold uppercase tracking-wider {sectionHeaderTone}">Basic Info</h3>
      </header>
      
      <div class="space-y-3">
        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-name">
              Role Name
            </label>
            <input
              id="role-name"
              type="text"
              class="w-full h-9 px-3 rounded-lg border text-sm transition-all outline-none {inputTone}"
              placeholder="e.g. Frontend Developer"
              value={name}
              oninput={handleNameInput}
              data-testid="role-editor-name-input"
            />
          </div>
          <div class="space-y-1.5">
            <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-id">
              Role ID
            </label>
            <input
              id="role-id"
              type="text"
              class="w-full h-9 px-3 rounded-lg border text-sm transition-all outline-none {inputTone} {isExisting ? 'opacity-50 cursor-not-allowed' : ''}"
              placeholder="e.g. frontend-dev"
              value={roleId}
              oninput={handleIdInput}
              disabled={isExisting}
              data-testid="role-editor-id-input"
            />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-tool">
              Tool
            </label>
            <div class="relative">
              <select
                id="role-tool"
                class="w-full h-9 px-2 rounded-lg border text-sm appearance-none transition-all outline-none {inputTone}"
                value={tool}
                onchange={handleToolChange}
                data-testid="role-editor-tool-select"
              >
                {#each toolOptions as opt}
                  <option value={opt}>{opt.charAt(0).toUpperCase() + opt.slice(1)}</option>
                {/each}
              </select>
              <div class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
              </div>
            </div>
          </div>
          <div class="space-y-1.5">
            <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-model">
              Model
            </label>
            <div class="relative">
              <select
                id="role-model"
                class="w-full h-9 px-2 rounded-lg border text-sm appearance-none transition-all outline-none {inputTone}"
                bind:value={model}
                data-testid="role-editor-model-select"
              >
                {#each modelOptionsByTool[tool] as opt}
                  <option value={opt}>{opt}</option>
                {/each}
              </select>
              <div class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Instructions Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 delay-75 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
      <header class="mb-3">
        <h3 class="text-[10px] font-bold uppercase tracking-wider {sectionHeaderTone}">Instructions</h3>
      </header>
      <div class="space-y-1.5">
        <label class="sr-only" for="role-instructions">Instructions</label>
        <textarea
          id="role-instructions"
          class="w-full p-3 rounded-lg border text-xs font-mono transition-all outline-none resize-none {inputTone}"
          rows="8"
          placeholder="Role instructions (Markdown supported)..."
          bind:value={instructions}
          data-testid="role-editor-instructions-input"
        ></textarea>
      </div>
    </section>

    <!-- Behavioral Contract Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 delay-100 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
      <header class="mb-3">
        <h3 class="text-[10px] font-bold uppercase tracking-wider {sectionHeaderTone}">Behavioral Contract</h3>
      </header>
      
      <div class="space-y-2">
        {#each behavioralContract as item, i}
          <div class="group flex items-start gap-2.5 p-2 rounded-lg border transition-all {dark ? 'border-white/[0.04] bg-white/[0.02] hover:bg-white/[0.04]' : 'border-brand-200/20 bg-white hover:bg-brand-50/30'}">
            <label for="role-rule-{i}-checkbox-input" class="flex flex-1 items-start gap-2.5 cursor-pointer">
              <div class="pt-0.5 relative flex items-center justify-center">
                <input
                  id="role-rule-{i}-checkbox-input"
                  type="checkbox"
                  checked={item.enabled}
                  onchange={() => toggleRule(i)}
                  class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
                  data-testid="role-rule-{i}-checkbox"
                />
                <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
              </div>
              
              <span class="flex-1 text-xs leading-5 {item.enabled ? t.textBody : t.textMuted + ' line-through opacity-50'}">{item.rule}</span>
            </label>

            <button
              class="h-8 w-8 flex items-center justify-center rounded-md {dark ? 'text-zinc-500' : 'text-zinc-600'} hover:text-danger-500 hover:bg-danger-500/10 transition-all opacity-0 group-hover:opacity-100"
              onclick={() => removeRule(i)}
              data-testid="role-rule-{i}-remove"
              aria-label="Remove rule"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
            </button>
          </div>
        {/each}
        
        <div class="flex gap-2 pt-1">
          <input
            type="text"
            class="flex-1 h-9 px-3 rounded-lg border text-xs transition-all outline-none {inputTone}"
            placeholder="Add a hard constraint..."
            bind:value={newRule}
            onkeydown={(e) => e.key === 'Enter' && addRule()}
            data-testid="role-editor-add-rule-input"
          />
          <button
            class="h-9 px-4 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 transition-all shadow-lg shadow-brand-500/10 disabled:opacity-50 disabled:pointer-events-none"
            onclick={addRule}
            disabled={!newRule.trim()}
            data-testid="role-editor-add-rule-button"
          >
            Add
          </button>
        </div>
      </div>
    </section>

    <!-- Capabilities Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 delay-150 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
      <header class="mb-3">
        <h3 class="text-[10px] font-bold uppercase tracking-wider {sectionHeaderTone}">Capabilities</h3>
      </header>
      <div class="space-y-3">
        <div class="flex flex-wrap gap-1.5">
          {#each capabilities as cap, i}
            <span class="inline-flex items-center gap-1.5 pl-2.5 pr-1 py-1 rounded-full text-[10px] font-bold border transition-all {dark ? 'bg-white/[0.04] border-white/[0.08] text-brand-400' : 'bg-brand-50 border-brand-200/50 text-brand-700'}">
              {cap}
              <button
                class="h-6 w-6 flex items-center justify-center rounded-full hover:bg-black/10 dark:hover:bg-white/10 transition-colors"
                onclick={() => removeCapability(i)}
                data-testid="role-capability-{i}-remove"
                aria-label="Remove capability"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
              </button>
            </span>
          {/each}
          {#if capabilities.length === 0}
            <p class="text-[10px] italic {t.textMuted} px-1">No custom capabilities defined.</p>
          {/if}
        </div>
        
        <div class="flex gap-2">
          <input
            type="text"
            class="flex-1 h-9 px-3 rounded-lg border text-xs transition-all outline-none {inputTone}"
            placeholder="Add capability tag..."
            bind:value={newCapability}
            onkeydown={(e) => e.key === 'Enter' && addCapability()}
            data-testid="role-editor-add-capability-input"
          />
          <button
            class="h-9 px-4 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 transition-all shadow-lg shadow-brand-500/10 disabled:opacity-50 disabled:pointer-events-none"
            onclick={addCapability}
            disabled={!newCapability.trim()}
            data-testid="role-editor-add-capability-button"
          >
            Add
          </button>
        </div>
      </div>
    </section>

    <!-- Floating Footer Actions -->
    <div class="fixed bottom-0 right-0 left-0 p-4 border-t backdrop-blur-md transition-all z-10 {dark ? 'bg-brand-950/80 border-white/[0.06]' : 'bg-white/80 border-brand-200/60'}" style="width: inherit; border-bottom-right-radius: inherit;">
      <div class="flex items-center justify-between max-w-full">
        <div>
          {#if isExisting && !isBuiltIn}
            <button
              class="h-9 px-3 rounded-lg text-xs text-danger-500 hover:bg-danger-500/10 font-bold transition-all active:scale-95"
              onclick={() => onDelete(role.roleId)}
              data-testid="role-editor-delete"
            >
              Delete Role
            </button>
          {/if}
        </div>
        <div class="flex gap-2">
          <button
            class="h-9 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
            onclick={onCancel}
            data-testid="role-editor-cancel"
          >
            Cancel
          </button>
          <button
            class="h-9 px-5 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
            onclick={handleSave}
            disabled={!canSave}
            data-testid="role-editor-save"
          >
            Save Role
          </button>
        </div>
      </div>
    </div>
  </div>
</SlideOver>

<style>
  /* Custom select arrow removal for Safari/Chrome */
  select {
    -webkit-appearance: none;
    -moz-appearance: none;
    appearance: none;
  }
</style>
