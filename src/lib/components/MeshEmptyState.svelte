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
  const actionTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900/60 text-zinc-300 hover:border-zinc-600 hover:bg-zinc-800'
      : 'border-brand-200 bg-white/80 text-brand-700 hover:border-brand-300 hover:bg-brand-50'
  )

  const normalizedPresets = $derived.by(() => {
    if (!Array.isArray(presets)) return []
    return presets.filter((preset) => preset && (preset.presetId || preset.name))
  })
</script>

<section class="flex min-h-[420px] flex-col items-center justify-center gap-6 py-6" data-testid="mesh-empty-state">
  <header class="max-w-xl space-y-1.5 text-center">
    <h3 class="text-lg font-semibold {titleTone}">Start a Team</h3>
    <p class="text-sm {subtitleTone}">
      Choose a preset to start, or build from scratch
    </p>
  </header>

  {#if normalizedPresets.length > 0}
    <div class="grid w-full max-w-5xl grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
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

  <div class="flex flex-wrap items-center justify-center gap-2">
    <button
      class="rounded-[12px] border px-3 py-1.5 text-xs font-medium transition-colors {actionTone}"
      type="button"
      onclick={() => onBrowseTemplates()}
      data-testid="mesh-template-browse-catalog"
    >
      Browse Catalog
    </button>
    <button
      class="rounded-[12px] border px-3 py-1.5 text-xs font-medium transition-colors {actionTone}"
      type="button"
      onclick={() => onStartCustom()}
      data-testid="mesh-template-build-custom"
    >
      Build Custom
    </button>
  </div>
</section>
