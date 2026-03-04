<script>
  import PresetCard from './PresetCard.svelte'

  let {
    presets = [],
    dark = false,
    onSelectPreset = () => {},
    onBrowseTemplates = () => {},
    onStartCustom = () => {},
  } = $props()

  const titleTone = $derived(dark ? 'text-zinc-100' : 'text-brand-900')
  const subtitleTone = $derived(dark ? 'text-zinc-400' : 'text-brand-700')
  const browseTone = $derived(
    dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700'
  )
  const scratchTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800'
      : 'border-brand-200 text-brand-700 hover:bg-brand-50'
  )

  const normalizedPresets = $derived.by(() => {
    if (!Array.isArray(presets)) return []
    return presets.filter((preset) => preset && (preset.presetId || preset.name))
  })
</script>

<section class="flex flex-col items-center justify-center gap-4 py-4" data-testid="mesh-empty-state">
  <header class="space-y-1 text-center">
    <h3 class="text-base font-semibold {titleTone}">Compose your team</h3>
    <p class="text-[13px] {subtitleTone}">
      Choose a preset to start, or build from scratch
    </p>
  </header>

  {#if normalizedPresets.length > 0}
    <div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
      {#each normalizedPresets as preset}
        <PresetCard
          name={preset.name}
          description={preset.description}
          leadCount={preset.leadCount ?? 1}
          agentCount={preset.agentCount ?? 0}
          tools={preset.tools ?? []}
          builtIn={preset.builtIn ?? false}
          dark={dark}
          testId={`mesh-template-preset-${preset.presetId ?? preset.name}`}
          onSelect={() => onSelectPreset(preset)}
          onInspect={() => onBrowseTemplates()}
        />
      {/each}
    </div>
  {/if}

  <div class="flex items-center gap-2">
    <button
      class="rounded-md px-2 py-1 text-xs font-medium transition-colors {browseTone}"
      type="button"
      onclick={() => onBrowseTemplates()}
      data-testid="mesh-template-browse-catalog"
    >
      Browse all templates
    </button>
    <button
      class="rounded-md border px-2 py-1 text-xs transition-colors {scratchTone}"
      type="button"
      onclick={() => onStartCustom()}
      data-testid="mesh-template-build-custom"
    >
      Start from scratch
    </button>
  </div>
</section>
