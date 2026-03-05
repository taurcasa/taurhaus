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
  const subtitleTone = $derived(dark ? 'text-zinc-500' : 'text-brand-700/70')
  
  const actionTone = $derived(
    dark
      ? 'bg-white/[0.05] border-white/[0.08] text-zinc-300 hover:text-white hover:bg-white/[0.1] active:scale-95'
      : 'bg-zinc-100 border-zinc-200 text-zinc-700 hover:bg-zinc-200 active:scale-95'
  )

  const normalizedPresets = $derived.by(() => {
    if (!Array.isArray(presets)) return []
    return presets.filter((preset) => preset && (preset.presetId || preset.name))
  })
</script>

<section class="flex min-h-[420px] flex-col items-center justify-center gap-8 py-12 animate-in fade-in duration-500" data-testid="mesh-empty-state">
  <header class="max-w-xl space-y-2 text-center animate-in fade-in slide-in-from-top-2 duration-300">
    <h3 class="text-2xl font-bold tracking-tight {titleTone}">Start a Team</h3>
    <p class="text-sm font-medium {subtitleTone}">
      Choose a curated preset or design a custom mesh for your project.
    </p>
  </header>

  {#if normalizedPresets.length > 0}
    <div class="grid w-full max-w-5xl grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 animate-in fade-in slide-in-from-bottom-2 duration-500 delay-150">
      {#each normalizedPresets as preset, i}
        <div class="transition-all" style:transition-delay={`${200 + (i * 50)}ms`}>
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
        </div>
      {/each}
    </div>
  {/if}

  <div class="flex flex-wrap items-center justify-center gap-3 animate-in fade-in duration-300 delay-500">
    <button
      class="h-10 px-6 rounded-lg border font-bold text-xs transition-all {actionTone}"
      type="button"
      onclick={() => onBrowseTemplates()}
      data-testid="mesh-template-browse-catalog"
    >
      Browse Catalog
    </button>
    <button
      class="h-10 px-6 rounded-lg border font-bold text-xs transition-all {actionTone}"
      type="button"
      onclick={() => onStartCustom()}
      data-testid="mesh-template-build-custom"
    >
      Build Custom
    </button>
  </div>
</section>
