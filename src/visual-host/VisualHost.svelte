<script>
  import { resetVisualHostState } from './mockState.js'
  import { getRegistryEntry, viewportPresets, visualRegistry } from './registry.js'
  import { readVisualHostQuery } from './query.js'

  /**
   * The URL is the address of a fixture state, so a headless browser can shoot
   * one without a human touching the selects. `chrome=0` drops the controls so
   * the screenshot is the fixture panel and nothing else.
   */
  const query = readVisualHostQuery(
    typeof location === 'undefined' ? '' : location.search,
    { registry: visualRegistry, viewports: viewportPresets },
  )

  let selectedComponentId = $state(query.componentId)
  let selectedScenarioName = $state(query.scenarioName)
  let selectedViewportId = $state(query.viewportId)
  let selectedTheme = $state(query.theme)
  const showChrome = query.chrome
  /** A theme named in the URL outranks the scenario's own. */
  const themePinned = query.themePinned

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
    if (themePinned) return
    selectedTheme = selectedScenario.theme ?? 'light'
  })

  /**
   * The fixture's mocks, applied before it renders, and the key that remounts
   * it when they change.
   *
   * This ran as an effect that bumped a counter inside the `{#key}`, which
   * mounted every fixture twice: once against the default mocks, then again.
   * A component that measures itself in an effect — every popup that positions
   * against the viewport does — lost that measurement to the remount and
   * rendered at 0,0. Deriving the key runs the mocks in the render pass that
   * uses them, so a fixture mounts once, with the right data.
   */
  const fixtureKey = $derived.by(() => {
    resetVisualHostState()
    selectedEntry?.applyMocks?.(selectedScenario, {
      theme: selectedTheme,
      viewport: selectedViewport,
    })
    return `${selectedEntry?.id}:${selectedScenario?.name}:${selectedTheme}:${selectedViewport?.id}`
  })

  /**
   * The theme is a class on the document — `Shell.svelte` sets it in the app,
   * `renderVisual.js` in the browser-mode lane — and the panel surfaces in
   * `app.css` read it from there. Without it a dark shot framed a dark popup in
   * a light panel: a PNG filed under `dark` that was not one.
   */
  $effect(() => {
    document.documentElement.classList.toggle('dark', selectedTheme === 'dark')
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

<!-- The screenshot lane reads this back out of the rendered DOM: a shot is
     evidence about one component, in one scenario, at one size, in one theme,
     so the page has to say which — a fallback in any of the four is a PNG of
     something the lane was not asked for. -->
<main
  class={`min-h-screen w-full ${chromeTone}`}
  data-testid="visual-host-root"
  data-visual-host-fixture={
    `${selectedEntry?.id ?? ''}/${selectedScenario?.name ?? ''}`
    + `/${selectedViewport?.id ?? ''}/${selectedTheme}`
  }
>
  <div class={showChrome ? 'mx-auto flex min-h-screen max-w-[1600px] flex-col gap-6 px-6 py-6' : 'h-screen w-full'}>
    {#if showChrome}
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
    {/if}

    {#snippet fixture()}
      {#if selectedScenario}
        {#key fixtureKey}
          <selectedEntry.component
            scenario={selectedScenario}
            theme={selectedTheme}
            viewport={selectedViewport}
          />
        {/key}
      {/if}
    {/snippet}

    {#if showChrome}
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
            {@render fixture()}
          </div>
        </div>
      </section>
    {:else if query.unknownRequest}
      <!-- Bare mode is the screenshot lane, where a fallback is a wrong answer:
           the PNG would be filed as evidence about the fixture that was asked
           for. Say so on the page rather than render somebody else's fixture. -->
      <section
        class="flex h-screen w-full items-center justify-center p-10 text-center"
        data-testid="visual-host-unknown-fixture"
      >
        <p class="max-w-xl text-[15px] font-medium text-danger-500">
          The URL asked for a fixture that is not in the registry.
          Nearest match: {selectedEntry?.id ?? '—'} / {selectedScenario?.name ?? '—'}.
        </p>
      </section>
    {:else}
      <!-- Bare: a `fixed` popup is positioned against the browser viewport, so
           the only honest shot of one is a window sized to the preset with no
           host frame around the fixture. -->
      <section class="h-screen w-full" data-testid="visual-host-viewport">
        {@render fixture()}
      </section>
    {/if}
  </div>
</main>
