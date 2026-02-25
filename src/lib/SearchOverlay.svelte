<script>
  import { search } from './ipc.js'

  let { open = $bindable(false), dark = false, onNavigate = () => {} } = $props()

  let query = $state('')
  let results = $state([])
  let loading = $state(false)
  let debounceTimer = $state(null)
  let inputEl = $state(null)
  let selectedIndex = $state(-1)

  // Tokens
  const overlayBg = $derived(dark ? 'bg-zinc-900/95' : 'bg-white/95')
  const inputBg = $derived(dark ? 'bg-zinc-800 text-zinc-100 placeholder:text-zinc-500' : 'bg-zinc-100 text-zinc-900 placeholder:text-zinc-400')
  const borderColor = $derived(dark ? 'border-zinc-700' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const textMuted = $derived(dark ? 'text-zinc-600' : 'text-zinc-500')
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

  // Group results by entity_type
  const grouped = $derived(() => {
    const groups = {}
    for (const r of results) {
      if (!groups[r.entity_type]) groups[r.entity_type] = []
      groups[r.entity_type].push(r)
    }
    return groups
  })

  // Flat list of results for keyboard navigation
  const flatResults = $derived(() => {
    const flat = []
    for (const type of GROUP_ORDER) {
      const g = grouped()
      if (g[type]) flat.push(...g[type])
    }
    return flat
  })

  // Focus input when overlay opens
  $effect(() => {
    if (open && inputEl) {
      // Small delay to ensure the element is mounted
      requestAnimationFrame(() => inputEl?.focus())
    }
    if (!open) {
      query = ''
      results = []
      loading = false
      selectedIndex = -1
    }
  })

  function handleInput(e) {
    query = e.target.value
    selectedIndex = -1

    if (debounceTimer) clearTimeout(debounceTimer)

    if (!query.trim()) {
      results = []
      loading = false
      return
    }

    loading = true
    debounceTimer = setTimeout(async () => {
      try {
        results = await search(query, 20)
      } catch {
        results = []
      } finally {
        loading = false
      }
    }, 150)
  }

  function handleKeydown(e) {
    const flat = flatResults()

    if (e.key === 'Escape') {
      e.preventDefault()
      close()
      return
    }

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

  function navigateTo(result) {
    const action = mapResultToNavigation(result)
    onNavigate(action)
    close()
  }

  function mapResultToNavigation(result) {
    switch (result.entity_type) {
      case 'document':
        return { tab: 'files', filePath: result.file_path }
      case 'session':
        return { tab: 'overview', section: 'session' }
      case 'commit':
        return { tab: 'overview', section: 'commits' }
      default:
        return { tab: 'overview' }
    }
  }

  function close() {
    open = false
  }

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) close()
  }
</script>

{#if open}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/40 backdrop-blur-sm"
    onclick={handleBackdropClick}
    data-testid="search-overlay"
  >
    <!-- Search panel -->
    <div class="w-full max-w-[540px] {overlayBg} rounded-xl shadow-2xl border {borderColor} overflow-hidden backdrop-blur-xl">

      <!-- Search input -->
      <div class="flex items-center gap-3 px-4 h-[52px] border-b {borderColor}">
        <svg class="w-[18px] h-[18px] shrink-0 {textMuted}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"/>
        </svg>
        <input
          bind:this={inputEl}
          type="text"
          value={query}
          oninput={handleInput}
          onkeydown={handleKeydown}
          placeholder="Search across all projects..."
          class="flex-1 bg-transparent text-[15px] {textPrimary} outline-none placeholder:{textMuted}"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          data-testid="search-input"
        />
        {#if loading}
          <div class="w-4 h-4 border-2 border-brand-500/30 border-t-brand-500 rounded-full animate-spin shrink-0"></div>
        {:else if query}
          <button
            class="w-5 h-5 flex items-center justify-center rounded {textMuted} hover:{textSecondary} transition-colors"
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
            <p class="text-[13px] {textMuted}">Type to search across all projects</p>
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
            <p class="text-[13px] {textMuted}">No matches found</p>
          </div>
        {:else}
          <!-- Grouped results -->
          {@const groups = grouped()}
          {#each GROUP_ORDER as type}
            {#if groups[type]?.length > 0}
              <div class="px-3 pt-3 pb-1">
                <span class="text-[10px] font-medium uppercase tracking-[0.06em] {groupLabel}">{GROUP_LABELS[type]}</span>
              </div>
              {#each groups[type] as result, i}
                {@const flatIdx = flatResults().indexOf(result)}
                {@const isSelected = flatIdx === selectedIndex}
                <button
                  class="w-full flex items-start gap-3 px-3 py-2 text-left transition-colors rounded-md mx-0
                    {isSelected ? selectedBg : hoverBg}"
                  onclick={() => navigateTo(result)}
                  data-testid="search-result"
                >
                  <!-- Entity type icon -->
                  <div class="w-4 h-4 shrink-0 mt-0.5 {textMuted}">
                    {#if type === 'document'}
                      <svg fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/></svg>
                    {:else if type === 'session'}
                      <svg fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"/></svg>
                    {:else}
                      <svg fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M17.25 6.75 22.5 12l-5.25 5.25m-10.5 0L1.5 12l5.25-5.25m7.5-3-4.5 16.5"/></svg>
                    {/if}
                  </div>

                  <div class="flex-1 min-w-0">
                    <div class="text-[13px] {textPrimary} truncate">{result.title || result.file_path}</div>
                    {#if result.snippet}
                      <div class="text-[12px] {snippetColor} truncate mt-0.5">{result.snippet}</div>
                    {/if}
                  </div>

                  <!-- File path for documents -->
                  {#if type === 'document' && result.file_path}
                    <span class="text-[11px] font-mono {textMuted} shrink-0 mt-0.5">{result.file_path}</span>
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
