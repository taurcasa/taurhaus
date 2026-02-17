<script>
  import { getSettings, updateSettings, getIndexStatus, rebuildIndex } from './ipc.js'

  let { dark = false, onClose = () => {} } = $props()

  // Color tokens — $derived based on dark prop
  const mainBg        = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textBody      = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const linkColor     = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const cardBg        = $derived(dark ? 'bg-zinc-900' : 'bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-white border-zinc-300 text-zinc-900')
  const addonBg       = $derived(dark ? 'bg-zinc-700/50 text-zinc-400' : 'bg-zinc-100 text-zinc-500')
  const labelColor    = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

  // Settings state
  let settings = $state(null)
  let loading = $state(true)
  let saving = $state(false)
  let loadError = $state(null)

  // Index state
  let indexStatus = $state(null)
  let rebuilding = $state(false)
  let rebuildError = $state(null)

  // Edit modes
  let editingScanDirs = $state(false)
  let editingIgnore = $state(false)
  let scanDirsText = $state('')
  let ignoreText = $state('')

  // Load settings on mount
  $effect(() => {
    loadSettings()
    loadIndexStatus()
  })

  async function loadSettings() {
    loading = true
    loadError = null
    try {
      settings = await getSettings()
    } catch (e) {
      loadError = e?.message || 'Failed to load settings'
      // Provide defaults so the UI is still usable
      settings = {
        scan_directories: ['~/projects'],
        thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
        ignore_patterns: ['node_modules', '.git', 'target', 'dist'],
      }
    } finally {
      loading = false
    }
  }

  async function loadIndexStatus() {
    try {
      indexStatus = await getIndexStatus()
    } catch (e) {
      console.error('Failed to load index status:', e)
    }
  }

  async function saveSettings() {
    if (!settings) return
    saving = true
    try {
      settings = await updateSettings(settings)
    } catch (e) {
      console.error('Failed to save settings:', e)
    } finally {
      saving = false
    }
  }

  function handleThresholdBlur(field, value) {
    const num = parseInt(value, 10)
    if (isNaN(num) || num < 1) return
    settings.thresholds[field] = num
    saveSettings()
  }

  function startEditScanDirs() {
    scanDirsText = settings.scan_directories.join('\n')
    editingScanDirs = true
  }

  function saveScanDirs() {
    settings.scan_directories = scanDirsText
      .split('\n')
      .map(s => s.trim())
      .filter(s => s.length > 0)
    editingScanDirs = false
    saveSettings()
  }

  function startEditIgnore() {
    ignoreText = settings.ignore_patterns.join('\n')
    editingIgnore = true
  }

  function saveIgnore() {
    settings.ignore_patterns = ignoreText
      .split('\n')
      .map(s => s.trim())
      .filter(s => s.length > 0)
    editingIgnore = false
    saveSettings()
  }

  async function handleRebuildIndex() {
    rebuilding = true
    rebuildError = null
    try {
      await rebuildIndex()
      await loadIndexStatus()
    } catch (e) {
      rebuildError = e?.message || 'Failed to rebuild index'
    } finally {
      rebuilding = false
    }
  }

  // Keyboard: Escape closes settings
  $effect(() => {
    const handler = (e) => {
      if (e.key === 'Escape') {
        if (editingScanDirs) {
          editingScanDirs = false
        } else if (editingIgnore) {
          editingIgnore = false
        } else {
          onClose()
        }
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  })
</script>

<div class="flex-1 overflow-y-auto" data-testid="settings-view">
  <div class="max-w-[640px] mx-auto px-6 py-6">
    <!-- Header -->
    <div class="mb-6">
      <button
        class="text-[13px] {linkColor} transition-colors mb-3 flex items-center gap-1"
        onclick={onClose}
        data-testid="settings-back"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5"/></svg>
        Back to projects
      </button>
      <h1 class="text-[20px] font-semibold {textPrimary}">Settings</h1>
    </div>

    {#if loading}
      <div class="space-y-4">
        {#each Array(3) as _}
          <div class="{cardBg} rounded-lg p-4 animate-pulse">
            <div class="h-3 w-24 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'} mb-3"></div>
            <div class="h-8 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
          </div>
        {/each}
      </div>
    {:else if settings}
      <div class="space-y-4">
        {#if loadError}
          <div class="px-3 py-2 rounded-md text-[13px] {dark ? 'bg-warning-500/10 text-warning-400 border border-warning-500/20' : 'bg-warning-50 text-warning-700 border border-warning-200'}" data-testid="settings-load-error">
            Could not load saved settings — showing defaults. {loadError}
          </div>
        {/if}

        <!-- ═══ PROJECT SCANNING ═══ -->
        <section class="{cardBg} rounded-lg border {keyline} p-4" data-testid="settings-scanning">
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {labelColor} mb-3">Project Scanning</h2>

          <!-- Scan directories -->
          <div class="mb-4">
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-[13px] {textSecondary}">Scan directories</span>
              {#if !editingScanDirs}
                <button
                  class="text-[12px] {linkColor} transition-colors"
                  onclick={startEditScanDirs}
                >Edit</button>
              {/if}
            </div>
            {#if editingScanDirs}
              <textarea
                class="w-full h-24 px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 resize-none font-mono"
                bind:value={scanDirsText}
                placeholder="One directory per line"
              ></textarea>
              <div class="flex gap-2 mt-2">
                <button
                  class="px-3 py-1 text-[12px] rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors"
                  onclick={saveScanDirs}
                >Save</button>
                <button
                  class="px-3 py-1 text-[12px] rounded-md {textTertiary} hover:text-zinc-600 transition-colors"
                  onclick={() => editingScanDirs = false}
                >Cancel</button>
              </div>
            {:else}
              {#if settings.scan_directories.length === 0}
                <p class="text-[13px] {textTertiary} italic">No directories configured</p>
              {:else}
                <div class="space-y-1">
                  {#each settings.scan_directories as dir}
                    <div class="text-[13px] {textBody} font-mono py-0.5">{dir}</div>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>

          <!-- Ignore patterns -->
          <div>
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-[13px] {textSecondary}">Ignore patterns</span>
              {#if !editingIgnore}
                <button
                  class="text-[12px] {linkColor} transition-colors"
                  onclick={startEditIgnore}
                >Edit</button>
              {/if}
            </div>
            {#if editingIgnore}
              <textarea
                class="w-full h-24 px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 resize-none font-mono"
                bind:value={ignoreText}
                placeholder="One pattern per line"
              ></textarea>
              <div class="flex gap-2 mt-2">
                <button
                  class="px-3 py-1 text-[12px] rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors"
                  onclick={saveIgnore}
                >Save</button>
                <button
                  class="px-3 py-1 text-[12px] rounded-md {textTertiary} hover:text-zinc-600 transition-colors"
                  onclick={() => editingIgnore = false}
                >Cancel</button>
              </div>
            {:else}
              {#if settings.ignore_patterns.length === 0}
                <p class="text-[13px] {textTertiary} italic">No patterns configured</p>
              {:else}
                <div class="flex flex-wrap gap-1.5">
                  {#each settings.ignore_patterns as pattern}
                    <span class="text-[12px] px-2 py-0.5 rounded-md font-mono {dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-600'}">{pattern}</span>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>
        </section>

        <!-- ═══ DISPLAY ═══ -->
        <section class="{cardBg} rounded-lg border {keyline} p-4" data-testid="settings-display">
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {labelColor} mb-3">Display</h2>

          <p class="text-[13px] {textSecondary} mb-4">Activity state thresholds (days since last activity)</p>

          <div class="space-y-3">
            <div class="flex items-center gap-3">
              <label for="threshold-active" class="text-[13px] {textBody} w-20">Active</label>
              <div class="flex items-stretch rounded-md border {dark ? 'border-zinc-700' : 'border-zinc-300'} overflow-hidden">
                <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-r {dark ? 'border-zinc-700' : 'border-zinc-300'}">{"<"}</span>
                <input
                  id="threshold-active"
                  type="number"
                  min="1"
                  value={settings.thresholds.active_days}
                  onblur={(e) => handleThresholdBlur('active_days', e.target.value)}
                  class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} focus:outline-none text-center border-none"
                  data-testid="threshold-active"
                />
                <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-l {dark ? 'border-zinc-700' : 'border-zinc-300'}">days</span>
              </div>
            </div>

            <div class="flex items-center gap-3">
              <label for="threshold-recent" class="text-[13px] {textBody} w-20">Recent</label>
              <div class="flex items-stretch rounded-md border {dark ? 'border-zinc-700' : 'border-zinc-300'} overflow-hidden">
                <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-r {dark ? 'border-zinc-700' : 'border-zinc-300'}">{"<"}</span>
                <input
                  id="threshold-recent"
                  type="number"
                  min="1"
                  value={settings.thresholds.recent_days}
                  onblur={(e) => handleThresholdBlur('recent_days', e.target.value)}
                  class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} focus:outline-none text-center border-none"
                  data-testid="threshold-recent"
                />
                <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-l {dark ? 'border-zinc-700' : 'border-zinc-300'}">days</span>
              </div>
            </div>

            <div class="flex items-center gap-3">
              <label for="threshold-stale" class="text-[13px] {textBody} w-20">Stale</label>
              <div class="flex items-stretch rounded-md border {dark ? 'border-zinc-700' : 'border-zinc-300'} overflow-hidden">
                <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-r {dark ? 'border-zinc-700' : 'border-zinc-300'}">{"<"}</span>
                <input
                  id="threshold-stale"
                  type="number"
                  min="1"
                  value={settings.thresholds.stale_days}
                  onblur={(e) => handleThresholdBlur('stale_days', e.target.value)}
                  class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} focus:outline-none text-center border-none"
                  data-testid="threshold-stale"
                />
                <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-l {dark ? 'border-zinc-700' : 'border-zinc-300'}">days</span>
              </div>
            </div>
          </div>

          {#if saving}
            <p class="text-[12px] {textTertiary} mt-2">Saving...</p>
          {/if}
        </section>

        <!-- ═══ SEARCH INDEX ═══ -->
        <section class="{cardBg} rounded-lg border {keyline} p-4" data-testid="settings-index">
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {labelColor} mb-3">Search Index</h2>

          {#if indexStatus}
            <div class="flex items-center gap-4 mb-3">
              <div>
                <p class="text-[13px] {textBody}">
                  {indexStatus.doc_count} document{indexStatus.doc_count !== 1 ? 's' : ''} indexed
                </p>
              </div>
            </div>
          {/if}

          <button
            class="px-3 py-1.5 text-[13px] rounded-md border {keyline} {textSecondary} transition-colors disabled:opacity-50 {dark ? 'hover:bg-zinc-800 hover:border-zinc-700' : 'hover:bg-zinc-100 hover:border-zinc-300'}"
            onclick={handleRebuildIndex}
            disabled={rebuilding}
            data-testid="rebuild-index-btn"
          >
            {rebuilding ? 'Rebuilding...' : 'Rebuild index'}
          </button>
          {#if rebuildError}
            <p class="mt-2 text-[12px] text-danger-500" data-testid="rebuild-error">{rebuildError}
              <button class="ml-1 underline" onclick={handleRebuildIndex}>Retry</button>
            </p>
          {/if}
        </section>

      </div>
    {/if}
  </div>
</div>
