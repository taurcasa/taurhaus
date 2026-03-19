<script>
  import { focusFirstInteractiveElement, getFocusableElements, registerModalLayer } from './a11y.js'
  import { search } from './ipc.js'
  import { themeTokens } from './themeTokens.js'

  let { open = $bindable(false), dark = false, onNavigate = () => {} } = $props()

  let query = $state('')
  let results = $state([])
  let loading = $state(false)
  let debounceTimer = $state(null)
  let inputEl = $state(null)
  let overlayEl = $state(null)
  let rootEl = $state(null)
  let selectedIndex = $state(-1)
  let searchRequestId = 0
  let restoreFocusElement = null

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const overlayBg = $derived(dark ? 'bg-zinc-900/95' : 'bg-white/95')
  const borderColor = $derived(dark ? 'border-zinc-700' : 'border-zinc-200')
  const textSecondary = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const hoverBg = $derived(dark ? 'hover:bg-zinc-800' : 'hover:bg-zinc-50')
  const selectedBg = $derived(dark ? 'bg-zinc-800' : 'bg-zinc-100')
  const groupLabel = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const snippetColor = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const footerText = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const kbdStyle = $derived(dark
    ? 'bg-zinc-700 text-zinc-400 border-zinc-600'
    : 'bg-zinc-200 text-zinc-500 border-zinc-300')

  const GROUP_LABELS = {
    document: 'Documents',
    session: 'Sessions',
    commit: 'Commits',
  }

  const GROUP_ORDER = ['document', 'session', 'commit']

  // Group results by entity_type and precompute flat indices once per result set.
  const groupedResults = $derived.by(() => {
    const groups = {}
    for (const result of results) {
      if (!groups[result.entity_type]) groups[result.entity_type] = []
      groups[result.entity_type].push(result)
    }

    const indexed = {}
    const flat = []
    let flatIndex = 0

    for (const type of GROUP_ORDER) {
      const entries = groups[type] ?? []
      indexed[type] = entries.map((result) => {
        const item = { result, flatIndex }
        flat.push(result)
        flatIndex += 1
        return item
      })
    }

    return { indexed, flat }
  })

  // Flat list of results for keyboard navigation (document -> session -> commit).
  const flatResults = $derived.by(() => groupedResults.flat)

  // Reset overlay-local state when closed
  $effect(() => {
    if (open) return
    if (debounceTimer) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }
    searchRequestId += 1
    query = ''
    results = []
    loading = false
    selectedIndex = -1
  })

  function handleInput(e) {
    const nextQuery = e.target.value
    query = nextQuery
    selectedIndex = -1

    if (debounceTimer) {
      clearTimeout(debounceTimer)
      debounceTimer = null
    }

    searchRequestId += 1
    const requestId = searchRequestId

    if (!nextQuery.trim()) {
      results = []
      loading = false
      return
    }

    loading = true
    debounceTimer = setTimeout(async () => {
      debounceTimer = null
      try {
        const nextResults = await search(nextQuery, 20)
        if (requestId !== searchRequestId) return
        results = nextResults
      } catch {
        if (requestId !== searchRequestId) return
        results = []
      } finally {
        if (requestId !== searchRequestId) return
        loading = false
      }
    }, 150)
  }

  function handleSearchKeydown(e) {
    if (!open) return
    const active = document.activeElement
    const activeInsideOverlay = active instanceof HTMLElement && overlayEl?.contains(active)

    if (e.key === 'Escape') {
      e.preventDefault()
      close()
      return
    }

    if (e.key === 'Tab') {
      const focusable = getFocusableElements(overlayEl)
      if (focusable.length === 0) {
        e.preventDefault()
        overlayEl?.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]

      if (!activeInsideOverlay) {
        e.preventDefault()
        ;(e.shiftKey ? last : first).focus()
        return
      }

      if (!e.shiftKey && active === last) {
        e.preventDefault()
        first.focus()
      } else if (e.shiftKey && active === first) {
        e.preventDefault()
        last.focus()
      }
      return
    }

    if (!activeInsideOverlay) return
    const flat = flatResults

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      selectedIndex = Math.min(selectedIndex + 1, flat.length - 1)
      return
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault()
      selectedIndex = Math.max(selectedIndex - 1, -1)
      return
    }

    if (e.key === 'Enter' && selectedIndex >= 0 && selectedIndex < flat.length) {
      e.preventDefault()
      navigateTo(flat[selectedIndex])
      return
    }
  }

  $effect(() => {
    if (!open) return
    if (
      !restoreFocusElement
      && document.activeElement instanceof HTMLElement
      && !rootEl?.contains(document.activeElement)
    ) {
      restoreFocusElement = document.activeElement
    }

    const unregisterModal = registerModalLayer(rootEl)

    const rafId = requestAnimationFrame(() => {
      focusFirstInteractiveElement(overlayEl, inputEl)
    })

    window.addEventListener('keydown', handleSearchKeydown)
    return () => {
      cancelAnimationFrame(rafId)
      unregisterModal()
      window.removeEventListener('keydown', handleSearchKeydown)
      if (restoreFocusElement?.isConnected) {
        restoreFocusElement.focus()
      }
      restoreFocusElement = null
    }
  })

  function navigateTo(result) {
    const action = mapResultToNavigation(result)
    onNavigate(action)
    close()
  }

  function mapResultToNavigation(result) {
    switch (result.entity_type) {
      case 'document':
        return { tab: 'files', filePath: result.file_path, projectId: result.project_id }
      case 'session':
        return { tab: 'overview', section: 'session', projectId: result.project_id }
      case 'commit':
        return { tab: 'overview', section: 'commits', projectId: result.project_id }
      default:
        return { tab: 'overview' }
    }
  }

  function close() {
    open = false
  }
</script>

{#if open}
  <div
    bind:this={rootEl}
    class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/40 backdrop-blur-sm"
    data-testid="search-overlay"
    data-shell-overlay
  >
    <button
      type="button"
      class="absolute inset-0 bg-transparent"
      onclick={close}
      tabindex="-1"
      aria-label="Close search overlay"
    ></button>

    <!-- Search panel -->
    <div
      bind:this={overlayEl}
      class="relative z-10 w-full max-w-[540px] {overlayBg} rounded-xl shadow-2xl border {borderColor} overflow-hidden backdrop-blur-xl"
      role="dialog"
      aria-modal="true"
      aria-label="Search across all projects"
      data-testid="search-dialog"
    >

      <!-- Search input -->
      <div class="flex items-center gap-3 px-4 h-[52px] border-b {borderColor}">
        <svg class="w-[18px] h-[18px] shrink-0 {t.textMuted}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"/>
        </svg>
        <input
          bind:this={inputEl}
          type="text"
          value={query}
          oninput={handleInput}
          placeholder="Search across all projects..."
          class="flex-1 rounded-sm bg-transparent text-[15px] {t.textPrimary} outline-none placeholder:{t.textMuted} focus:ring-1 focus:ring-brand-500"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          aria-label="Search across all projects"
          data-testid="search-input"
        />
        {#if loading}
          <div class="w-4 h-4 border-2 border-brand-500/30 border-t-brand-500 rounded-full animate-spin shrink-0"></div>
        {:else if query}
          <button
            class="w-5 h-5 flex items-center justify-center rounded {t.textMuted} hover:{textSecondary} transition-colors"
            onclick={() => { query = ''; results = []; selectedIndex = -1; inputEl?.focus() }}
            aria-label="Clear search"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/>
            </svg>
          </button>
        {:else}
          <kbd class="text-[11px] font-mono px-1.5 py-0.5 rounded border {kbdStyle}">esc</kbd>
        {/if}
      </div>

      <!-- Results -->
      <div class="max-h-[400px] overflow-y-auto" data-testid="search-results">
        {#if !query.trim()}
          <!-- Empty state -->
          <div class="px-4 py-8 text-center">
            <p class="text-[13px] {t.textMuted}">Type to search across all projects</p>
          </div>
        {:else if loading}
          <!-- Loading skeleton -->
          <div class="px-4 py-3 space-y-2">
            {#each Array(3) as _}
              <div class="flex items-center gap-3 h-[40px]">
                <div class="w-4 h-4 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'} animate-pulse"></div>
                <div class="flex-1">
                  <div class="h-3 w-2/3 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'} animate-pulse"></div>
                </div>
              </div>
            {/each}
          </div>
        {:else if results.length === 0 && query.trim()}
          <!-- No results -->
          <div class="px-4 py-8 text-center">
            <p class="text-[13px] {t.textMuted}">No matches found</p>
          </div>
        {:else}
          <!-- Grouped results -->
          {@const indexedGroups = groupedResults.indexed}
          {#each GROUP_ORDER as type}
            {#if indexedGroups[type]?.length > 0}
              <div class="px-3 pt-3 pb-1">
                <span class="text-[10px] font-medium uppercase tracking-[0.06em] {groupLabel}">{GROUP_LABELS[type]}</span>
              </div>
              {#each indexedGroups[type] as entry}
                {@const result = entry.result}
                {@const isSelected = entry.flatIndex === selectedIndex}
                <button
                  class="w-full flex items-start gap-3 px-3 py-2 text-left transition-colors rounded-md mx-0
                    focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand-500 focus-visible:ring-inset
                    {isSelected ? selectedBg : hoverBg}"
                  onclick={() => navigateTo(result)}
                  data-testid="search-result"
                >
                  <!-- Entity type icon -->
                  <div class="w-4 h-4 shrink-0 mt-0.5 {t.textMuted}">
                    {#if type === 'document'}
                      <svg fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                    {:else if type === 'session'}
                      <svg fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"/></svg>
                    {:else}
                      <svg fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M17.25 6.75 22.5 12l-5.25 5.25m-10.5 0L1.5 12l5.25-5.25m7.5-3-4.5 16.5"/></svg>
                    {/if}
                  </div>

                  <div class="flex-1 min-w-0">
                    <div class="text-[13px] {t.textPrimary} truncate">{result.title || result.file_path}</div>
                    {#if result.snippet}
                      <div class="text-[12px] {snippetColor} truncate mt-0.5">{result.snippet}</div>
                    {/if}
                  </div>

                  <!-- File path for documents -->
                  {#if type === 'document' && result.file_path}
                    <span class="text-[11px] font-mono {t.textMuted} shrink-0 mt-0.5">{result.file_path}</span>
                  {/if}
                </button>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>

      <!-- Footer hint -->
      {#if results.length > 0}
        <div class="px-4 py-2 border-t {borderColor} flex items-center gap-4">
          <span class="text-[11px] {footerText} flex items-center gap-1">
            <kbd class="text-[10px] font-mono px-1 py-0.5 rounded border {kbdStyle}">&uarr;</kbd>
            <kbd class="text-[10px] font-mono px-1 py-0.5 rounded border {kbdStyle}">&darr;</kbd>
            navigate
          </span>
          <span class="text-[11px] {footerText} flex items-center gap-1">
            <kbd class="text-[10px] font-mono px-1 py-0.5 rounded border {kbdStyle}">&crarr;</kbd>
            open
          </span>
          <span class="text-[11px] {footerText} flex items-center gap-1">
            <kbd class="text-[10px] font-mono px-1 py-0.5 rounded border {kbdStyle}">esc</kbd>
            close
          </span>
        </div>
      {/if}
    </div>
  </div>
{/if}
