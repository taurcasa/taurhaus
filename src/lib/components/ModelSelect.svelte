<script>
  import {
    catalogFor,
    defaultEffortFor,
    defaultModelFor,
    effortsFor,
    entryFor,
    isKnownModel,
    toolEffortsFor,
  } from '../modelCatalog.js'

  let {
    tool = 'codex',
    model = '',
    reasoningEffort = null,
    catalog = null,
    disabled = false,
    dark = false,
    compact = false,
    id = null,
    inputClass = '',
    testId = 'model-select',
    onchange = () => {},
  } = $props()

  const entries = $derived(catalogFor(catalog, tool))
  const selectedModel = $derived(String(model ?? '').trim() || defaultModelFor(catalog, tool))
  const selectedEntry = $derived(entryFor(catalog, tool, selectedModel))

  const modelOptions = $derived.by(() => {
    const known = entries
      .map((entry) => ({
        id: String(entry?.id ?? '').trim(),
        label: String(entry?.label ?? entry?.id ?? '').trim() || String(entry?.id ?? ''),
        deprecated: Boolean(entry?.deprecated),
        replacement: String(entry?.replacement ?? '').trim(),
      }))
      .filter((entry) => entry.id)
    // A value the catalog does not know (a YAML model, a newer release) is shown
    // as-is instead of being silently replaced by the first list entry.
    if (selectedModel && !known.some((entry) => entry.id === selectedModel)) {
      return [{ id: selectedModel, label: selectedModel, deprecated: false, replacement: '' }, ...known]
    }
    return known
  })

  // A model the catalog does not know still gets the tool's effort vocabulary:
  // that is exactly what the backend validates against (`supports_effort`).
  const efforts = $derived(
    isKnownModel(catalog, tool, selectedModel)
      ? effortsFor(catalog, tool, selectedModel)
      : toolEffortsFor(catalog, tool)
  )
  const selectedEffort = $derived(String(reasoningEffort ?? '').trim())
  // The leading empty option is the unset state: no effort is sent and the CLI's
  // own global setting applies. Never pre-select the catalog default here — that
  // would silently pin an effort the user never chose.
  const effortOptions = $derived(
    selectedEffort && !efforts.includes(selectedEffort)
      ? ['', selectedEffort, ...efforts]
      : ['', ...efforts]
  )
  const hasEffortSelect = $derived(efforts.length > 0 || Boolean(selectedEffort))
  const deprecationHint = $derived.by(() => {
    if (!selectedEntry?.deprecated) return ''
    const replacement = String(selectedEntry.replacement ?? '').trim()
    return replacement ? `Deprecated → ${replacement}` : 'Deprecated'
  })

  const controlTone = $derived(
    dark
      ? 'border-white/[0.08] bg-zinc-950/60 text-zinc-100 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/25'
      : 'border-brand-200/60 bg-white text-zinc-900 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/15'
  )
  const hintTone = $derived(dark ? 'text-amber-400/80' : 'text-amber-600')
  const sizeTone = $derived(compact ? 'h-8 px-2 text-xs' : 'h-10 px-3 text-sm')
  const controlClass = $derived(
    `w-full min-w-0 rounded-[14px] border outline-none transition-colors disabled:opacity-60 ${sizeTone} ${inputClass || controlTone}`
  )

  function labelFor(option) {
    if (!option.deprecated) return option.label
    return option.replacement ? `${option.label} → ${option.replacement}` : `${option.label} (deprecated)`
  }

  function handleModelChange(event) {
    const nextModel = event.currentTarget.value
    onchange({
      model: nextModel,
      reasoningEffort: defaultEffortFor(catalog, tool, nextModel),
    })
  }

  function handleEffortChange(event) {
    onchange({
      model: selectedModel,
      reasoningEffort: event.currentTarget.value || null,
    })
  }
</script>

<div class="flex w-full min-w-0 flex-col gap-1" data-testid={`${testId}-root`}>
  <div class="flex w-full min-w-0 items-center gap-1.5">
    <select
      {id}
      class="{controlClass} flex-1"
      value={selectedModel}
      {disabled}
      aria-label="Model"
      onchange={handleModelChange}
      data-testid={testId}
    >
      {#each modelOptions as option (option.id)}
        <option value={option.id}>{labelFor(option)}</option>
      {/each}
    </select>

    {#if hasEffortSelect}
      <select
        class="{controlClass} w-[6.5rem] flex-none"
        value={selectedEffort}
        {disabled}
        aria-label="Reasoning effort"
        onchange={handleEffortChange}
        data-testid={`${testId}-effort`}
      >
        {#each effortOptions as effort (effort)}
          <option value={effort}>{effort || 'default'}</option>
        {/each}
      </select>
    {/if}
  </div>

  {#if deprecationHint}
    <span class="text-[10px] {hintTone}" data-testid={`${testId}-deprecated`}>{deprecationHint}</span>
  {/if}
</div>
