<script>
  import { resetVisualHostState } from './mockState.js'
  import { getRegistryEntry, viewportPresets, visualRegistry } from './registry.js'

  let selectedComponentId = $state(visualRegistry[0]?.id ?? '')
  let selectedScenarioName = $state(visualRegistry[0]?.scenarios?.[0]?.name ?? '')
  let selectedViewportId = $state(viewportPresets[0]?.id ?? 'desktop')
  let selectedTheme = $state(visualRegistry[0]?.scenarios?.[0]?.theme ?? 'light')
  let renderVersion = $state(0)

  const selectedEntry = $derived(getRegistryEntry(selectedComponentId))
  const selectedViewport = $derived(
    viewportPresets.find((preset) => preset.id === selectedViewportId) ?? viewportPresets[0]
  )
  const selectedScenario = $derived(
    selectedEntry?.scenarios?.find((scenario) => scenario.name === selectedScenarioName)
      ?? selectedEntry?.scenarios?.[0]
      ?? null
  )

  $effect(() => {
    if (!selectedEntry) return
    if (!selectedEntry.scenarios.some((scenario) => scenario.name === selectedScenarioName)) {
      selectedScenarioName = selectedEntry.scenarios[0]?.name ?? ''
    }
  })

  $effect(() => {
    if (!selectedScenario) return
    selectedTheme = selectedScenario.theme ?? 'light'
  })

  $effect(() => {
    resetVisualHostState()
    selectedEntry?.applyMocks?.(selectedScenario, {
      theme: selectedTheme,
      viewport: selectedViewport,
    })
    renderVersion += 1
  })

  const chromeTone = $derived(
    selectedTheme === 'dark'
      ? 'bg-[#0a1318] text-zinc-100'
      : 'bg-[#f3f8f7] text-zinc-900'
  )
  const panelTone = $derived(
    selectedTheme === 'dark'
      ? 'border-white/10 bg-white/[0.04]'
      : 'border-brand-900/10 bg-white/88'
  )
  const viewportTone = $derived(
    selectedTheme === 'dark'
      ? 'border-white/10 bg-[#061015]'
      : 'border-brand-900/10 bg-white'
  )
</script>

<svelte:head>
  <title>taurhaus Visual Host</title>
</svelte:head>

<main class={`min-h-screen w-full ${chromeTone}`}>
  <div class="mx-auto flex min-h-screen max-w-[1600px] flex-col gap-6 px-6 py-6">
    <header class={`rounded-3xl border p-5 ${panelTone}`}>
      <div class="flex flex-wrap items-center gap-3">
        <div class="min-w-[220px] flex-1">
          <p class="text-[11px] font-semibold uppercase tracking-[0.18em] text-brand-500">Visual Host</p>
          <h1 class="mt-2 text-[28px] font-semibold tracking-[-0.03em]">
            Manual fixture browser for component states
          </h1>
        </div>

        <label class="min-w-[220px] text-sm">
          <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.14em] opacity-60">Component</span>
          <select
            bind:value={selectedComponentId}
            class={`w-full rounded-2xl border px-3 py-2.5 outline-none ${panelTone}`}
            data-testid="visual-host-component-select"
          >
            {#each visualRegistry as entry}
              <option value={entry.id}>{entry.label}</option>
            {/each}
          </select>
        </label>

        <label class="min-w-[220px] text-sm">
          <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.14em] opacity-60">Scenario</span>
          <select
            bind:value={selectedScenarioName}
            class={`w-full rounded-2xl border px-3 py-2.5 outline-none ${panelTone}`}
            data-testid="visual-host-scenario-select"
          >
            {#each selectedEntry?.scenarios ?? [] as scenario}
              <option value={scenario.name}>{scenario.name}</option>
            {/each}
          </select>
        </label>

        <label class="min-w-[220px] text-sm">
          <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.14em] opacity-60">Viewport</span>
          <select
            bind:value={selectedViewportId}
            class={`w-full rounded-2xl border px-3 py-2.5 outline-none ${panelTone}`}
            data-testid="visual-host-viewport-select"
          >
            {#each viewportPresets as preset}
              <option value={preset.id}>{preset.label}</option>
            {/each}
          </select>
        </label>

        <div class="min-w-[180px]">
          <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.14em] opacity-60">Theme</span>
          <div class={`inline-flex rounded-2xl border p-1 ${panelTone}`}>
            <button
              type="button"
              class={`rounded-xl px-3 py-2 text-sm font-medium ${selectedTheme === 'light' ? 'bg-white text-zinc-900 shadow-sm' : 'opacity-70'}`}
              onclick={() => { selectedTheme = 'light' }}
              data-testid="visual-host-theme-light"
            >
              Light
            </button>
            <button
              type="button"
              class={`rounded-xl px-3 py-2 text-sm font-medium ${selectedTheme === 'dark' ? 'bg-brand-500 text-white shadow-sm' : 'opacity-70'}`}
              onclick={() => { selectedTheme = 'dark' }}
              data-testid="visual-host-theme-dark"
            >
              Dark
            </button>
          </div>
        </div>
      </div>
    </header>

    <section class={`flex-1 rounded-[32px] border p-5 ${panelTone}`}>
      <div class="mb-3 flex items-center justify-between gap-4 text-sm opacity-70">
        <p>{selectedEntry?.label} / {selectedScenario?.name ?? 'no scenario'}</p>
        <p>{selectedViewport.width} x {selectedViewport.height}</p>
      </div>

      <div
        class={`overflow-auto rounded-[28px] border p-6 ${viewportTone}`}
        data-testid="visual-host-viewport"
        style={`height: min(72vh, ${selectedViewport.height + 80}px);`}
      >
        <div
          class="mx-auto rounded-[24px] border border-dashed border-brand-500/20 p-6"
          style={`width: min(100%, ${selectedViewport.width}px); min-height: ${selectedViewport.height}px;`}
        >
          {#if selectedScenario}
            {#key `${selectedEntry?.id}:${selectedScenario.name}:${selectedTheme}:${renderVersion}`}
              <selectedEntry.component
                scenario={selectedScenario}
                theme={selectedTheme}
                viewport={selectedViewport}
              />
            {/key}
          {/if}
        </div>
      </div>
    </section>
  </div>
</main>
