<script>
  import ModelSelect from './ModelSelect.svelte'
  import SlideOver from './SlideOver.svelte'
  import { getModelCatalogContext } from '../context/ModelCatalogContext.js'
  import { toolOptions } from '../meshDefaults.js'
  import { EMPTY_MODEL_CATALOG, resolveMemberModel } from '../modelCatalog.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    open = false,
    dark = false,
    role = null,
    modelCatalog = null,
    onSave = () => {},
    onCancel = () => {},
    onDelete = () => {}
  } = $props()

  const t = $derived(themeTokens(dark))
  const modelCatalogContext = getModelCatalogContext()
  const catalog = $derived(modelCatalog ?? modelCatalogContext?.catalog ?? EMPTY_MODEL_CATALOG)

  const availableTools = $derived(toolOptions())

  let name = $state('')
  let roleId = $state('')
  let tool = $state('claude')
  let model = $state('')
  let reasoningEffort = $state(null)
  let focusArea = $state('')
  let contextSummary = $state('')
  let behaviorSummary = $state('')
  let instructions = $state('')
  let behavioralContract = $state([])

  let newRule = $state('')
  let manualId = $state(false)

  const isBuiltIn = $derived(role?.builtIn || false)
  const isExisting = $derived(role !== null)

  const resolvedModel = $derived(resolveMemberModel({ tool, model, reasoningEffort }, null, catalog))

  const canSave = $derived(
    name.trim().length > 0 &&
    roleId.trim().length > 0 &&
    tool.length > 0 &&
    resolvedModel.model.length > 0
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
  const guidanceCardTone = $derived(
    dark
      ? 'border-white/[0.05] bg-black/20'
      : 'border-brand-200/50 bg-white/80'
  )
  const guidanceTextTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-600')

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
    model = ''
    reasoningEffort = null
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

  function optionalValue(value) {
    const normalized = String(value ?? '').trim()
    return normalized.length > 0 ? normalized : null
  }

  function handleSave() {
    if (!canSave) return
    const capabilityPolicy = role?.capabilityPolicy ?? role?.capability_policy ?? null
    onSave({
      roleId,
      name,
      tool,
      model: resolvedModel.model,
      reasoningEffort: resolvedModel.reasoningEffort,
      focusArea: optionalValue(focusArea),
      contextSummary: optionalValue(contextSummary),
      behaviorSummary: optionalValue(behaviorSummary),
      instructions,
      behavioralContract: JSON.parse(JSON.stringify(behavioralContract)),
      ...(capabilityPolicy ? { capabilityPolicy } : {}),
    })
  }

  $effect(() => {
    if (open) {
      if (role) {
        name = role.name || ''
        roleId = role.roleId || ''
        tool = role.tool || role.cliTool || 'claude'
        model = role.model || ''
        reasoningEffort = role.reasoningEffort ?? role.reasoning_effort ?? null
        focusArea = role.focusArea ?? role.focus_area ?? ''
        contextSummary = role.contextSummary ?? role.context_summary ?? ''
        behaviorSummary = role.behaviorSummary ?? role.behavior_summary ?? ''
        instructions = role.instructions || ''
        behavioralContract = role.behavioralContract ? JSON.parse(JSON.stringify(role.behavioralContract)) : []
        manualId = true
      } else {
        name = ''
        roleId = ''
        tool = 'claude'
        model = ''
        reasoningEffort = null
        focusArea = ''
        contextSummary = ''
        behaviorSummary = ''
        instructions = ''
        behavioralContract = []
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
                {#each availableTools as opt}
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
            <ModelSelect
              id="role-model"
              {tool}
              {model}
              {reasoningEffort}
              {catalog}
              {dark}
              compact
              inputClass={inputTone}
              testId="role-editor-model-select"
              onchange={(next) => {
                model = next.model
                reasoningEffort = next.reasoningEffort
              }}
            />
          </div>
        </div>
      </div>
    </section>

    <!-- Context Steering Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 delay-75 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
      <header class="mb-3 space-y-1">
        <h3 class="text-[10px] font-bold uppercase tracking-wider {sectionHeaderTone}">Context Steering</h3>
        <p class="text-[11px] leading-relaxed {guidanceTextTone}">
          Define the lane this role should keep warm over time. Think in accumulated context, not generic capabilities.
        </p>
      </header>

      <div class="space-y-3">
        <div class="rounded-xl border p-3 space-y-2 {guidanceCardTone}">
          <div class="space-y-1">
            <div class="flex items-center justify-between gap-3">
              <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-focus-area">
                Focus Area
              </label>
              <span class="text-[9px] font-bold uppercase tracking-[0.18em] {sectionHeaderTone}">Lane</span>
            </div>
            <p class="text-[11px] leading-relaxed {guidanceTextTone}">
              What domain does this agent specialize in?
            </p>
          </div>
          <input
            id="role-focus-area"
            type="text"
            class="w-full h-10 px-3 rounded-lg border text-sm transition-all outline-none {inputTone}"
            placeholder="Frontend UI and component architecture"
            bind:value={focusArea}
            data-testid="role-editor-focus-area-input"
          />
        </div>

        <div class="rounded-xl border p-3 space-y-2 {guidanceCardTone}">
          <div class="space-y-1">
            <div class="flex items-center justify-between gap-3">
              <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-context-summary">
                Context Summary
              </label>
              <span class="text-[9px] font-bold uppercase tracking-[0.18em] {sectionHeaderTone}">Memory</span>
            </div>
            <p class="text-[11px] leading-relaxed {guidanceTextTone}">
              What context should this agent accumulate over time?
            </p>
          </div>
          <textarea
            id="role-context-summary"
            class="w-full p-3 rounded-lg border text-xs leading-5 transition-all outline-none resize-none {inputTone}"
            rows="3"
            placeholder="Component patterns, design tokens, accessibility rules, and visual test coverage."
            bind:value={contextSummary}
            data-testid="role-editor-context-summary-input"
          ></textarea>
        </div>

        <div class="rounded-xl border p-3 space-y-2 {guidanceCardTone}">
          <div class="space-y-1">
            <div class="flex items-center justify-between gap-3">
              <label class="text-[10px] font-medium uppercase tracking-wide {labelTone}" for="role-behavior-summary">
                Behavioral Boundary
              </label>
              <span class="text-[9px] font-bold uppercase tracking-[0.18em] {sectionHeaderTone}">Escalation</span>
            </div>
            <p class="text-[11px] leading-relaxed {guidanceTextTone}">
              What should this agent handle directly versus escalate?
            </p>
          </div>
          <textarea
            id="role-behavior-summary"
            class="w-full p-3 rounded-lg border text-xs leading-5 transition-all outline-none resize-none {inputTone}"
            rows="3"
            placeholder="Handles UI implementation independently; escalates architecture changes and product-direction calls."
            bind:value={behaviorSummary}
            data-testid="role-editor-behavior-summary-input"
          ></textarea>
        </div>
      </div>
    </section>

    <!-- Instructions Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 delay-100 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
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

    <!-- Operational Boundaries Section -->
    <section class="p-3 rounded-xl border transition-all duration-200 delay-150 animate-in fade-in slide-in-from-bottom-1 {cardTone}">
      <header class="mb-3 space-y-1">
        <h3 class="text-[10px] font-bold uppercase tracking-wider {sectionHeaderTone}">Operational Boundaries</h3>
        <p class="text-[11px] leading-relaxed {guidanceTextTone}">
          Capture the non-negotiable rules the lead can depend on when routing work to this role.
        </p>
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
            placeholder="Add a rule the lead can rely on..."
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
