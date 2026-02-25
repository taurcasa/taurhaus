<script>
  import { scanDirectory, registerProjectsBatch, isTauri } from './ipc.js'
  import { themeTokens } from './themeTokens.js'
  import DirectoryBrowser from './DirectoryBrowser.svelte'

  let { dark = false, onComplete = () => {} } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens (different from shared)
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const hoverRow      = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-zinc-50 border-zinc-300 text-zinc-900')

  // Wizard state
  let step = $state(1)  // 1=welcome, 2=browse, 3=selection, 4=progress, 5=complete
  let scanning = $state(false)
  let discovered = $state([])
  let selected = $state(new Set())
  let registering = $state(false)
  let progressIndex = $state(0)
  let progressTotal = $state(0)
  let progressName = $state('')
  let registeredCount = $state(0)
  let failedPaths = $state([])
  let scanError = $state(null)
  let scanPath = $state('')

  const selectedCount = $derived(selected.size)

  // ═══ SCAN + REGISTER ═══

  async function handleScanPath() {
    if (!scanPath.trim()) return
    scanning = true
    scanError = null
    try {
      const results = await scanDirectory(scanPath.trim())
      discovered = results
      selected = new Set(results.filter(p => p.has_git).map(p => p.path))
      step = 3
    } catch (e) {
      scanError = e?.toString() || 'Failed to scan directory'
    } finally {
      scanning = false
    }
  }

  function toggleProject(path) {
    const next = new Set(selected)
    if (next.has(path)) {
      next.delete(path)
    } else {
      next.add(path)
    }
    selected = next
  }

  function selectAll() {
    selected = new Set(discovered.map(p => p.path))
  }

  function deselectAll() {
    selected = new Set()
  }

  async function handleRegister() {
    const paths = [...selected]
    progressTotal = paths.length
    progressIndex = 0
    registering = true
    step = 4

    // Listen for per-project progress events from the Rust backend
    let unlisten = null
    if (isTauri()) {
      try {
        const { listen } = await import('@tauri-apps/api/event')
        unlisten = await listen('batch-registration-progress', (event) => {
          progressIndex = (event.payload.index ?? 0) + 1
          progressName = event.payload.project_name || ''
        })
      } catch { /* non-critical */ }
    }

    try {
      const results = await registerProjectsBatch(paths)
      registeredCount = results.filter(r => r.success).length
      failedPaths = results.filter(r => !r.success).map(r => ({ path: r.path, error: r.error }))
      step = 5
      if (failedPaths.length === 0) {
        setTimeout(() => {
          onComplete()
        }, 2000)
      }
    } catch (e) {
      console.error('Registration failed:', e)
    } finally {
      registering = false
      if (unlisten) unlisten()
    }
  }
</script>

<div class="h-full {t.mainBg} flex items-center justify-center" data-testid="first-run-wizard" data-tauri-drag-region>
  <div class="max-w-[480px] w-full px-6">

    {#if step === 1}
      <!-- ═══ STEP 1: WELCOME ═══ -->
      <div class="text-center" data-testid="wizard-step-1">
        <!-- Logo -->
        <div class="w-14 h-14 rounded-xl bg-brand-500 flex items-center justify-center mx-auto mb-5">
          <span class="text-[22px] font-bold text-white leading-none">t</span>
        </div>

        <h1 class="text-[24px] font-semibold {t.textPrimary} mb-2">taurhaus</h1>
        <p class="text-[15px] {t.textSecondary} mb-1">AI Project Management</p>
        <p class="text-[13px] {t.textBody} mb-8 leading-relaxed">
          One clear view into all your projects — code, docs, progress, history — so you never lose context between sessions.
        </p>

        <button
          class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors"
          onclick={() => step = 2}
          data-testid="get-started-button"
        >Get started</button>
      </div>
    {/if}

    {#if step >= 2}
      <!-- Step 2 content kept mounted to preserve DirectoryBrowser tree state -->
      <div class:hidden={step !== 2} data-testid="wizard-step-2">
        <h2 class="text-[18px] font-semibold {t.textPrimary} mb-1">Where are your projects?</h2>
        <p class="text-[13px] {t.textSecondary} mb-4">Browse to the folder that contains your project directories, or type the path directly.</p>

        <!-- Path input -->
        <div class="mb-3">
          <input
            type="text"
            placeholder="/home/user/projects"
            bind:value={scanPath}
            class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
            onkeydown={(e) => e.key === 'Enter' && handleScanPath()}
          />
        </div>

        <!-- Directory tree -->
        <div class="mb-4">
          <DirectoryBrowser {dark} selectedPath={scanPath} onSelect={(path) => scanPath = path} maxHeight="280px" />
        </div>

        {#if scanError}
          <p class="text-[12px] text-danger-500 mb-3">{scanError}</p>
        {/if}

        <button
          class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50 mb-3"
          onclick={handleScanPath}
          disabled={!scanPath.trim() || scanning}
          data-testid="scan-button"
        >
          {scanning ? 'Scanning...' : `Scan ${scanPath || '...'}`}
        </button>

        <button
          class="text-[13px] {t.linkColor} transition-colors"
          onclick={() => step = 1}
        >Back</button>
      </div>
    {/if}

    {#if step === 3}
      <!-- ═══ STEP 3: PROJECT SELECTION ═══ -->
      <div data-testid="wizard-step-3">
        {#if discovered.length === 0}
          <!-- Empty scan results -->
          <div class="text-center" data-testid="empty-scan">
            <h2 class="text-[18px] font-semibold {t.textPrimary} mb-2">No projects found</h2>
            <p class="text-[13px] {t.textSecondary} mb-6">No git repositories were found in {scanPath}. Try a different directory.</p>
            <button
              class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors mb-3"
              onclick={() => step = 2}
            >Browse again</button>
          </div>
        {:else}
          <h2 class="text-[18px] font-semibold {t.textPrimary} mb-1">
            Found {discovered.length} repositor{discovered.length === 1 ? 'y' : 'ies'}
          </h2>
          <p class="text-[13px] {t.textSecondary} mb-4">in {scanPath}</p>

          <!-- Select all / Deselect all -->
          <div class="flex items-center gap-3 mb-3">
            <button
              class="text-[12px] {t.linkColor} transition-colors"
              onclick={selectAll}
            >Select all</button>
            <span class="text-[12px] {textTertiary}">|</span>
            <button
              class="text-[12px] {t.linkColor} transition-colors"
              onclick={deselectAll}
            >Deselect all</button>
            <span class="flex-1"></span>
            <span class="text-[12px] {textTertiary}">{selectedCount} selected</span>
          </div>

          <!-- Project list -->
          <div class="border {t.keyline} rounded-lg overflow-hidden mb-4 max-h-[320px] overflow-y-auto">
            {#each discovered as project}
              <button
                class="w-full flex items-center gap-3 px-3 py-2.5 text-left border-b last:border-b-0 {t.keyline} {hoverRow} transition-colors"
                onclick={() => toggleProject(project.path)}
              >
                <div class="w-4 h-4 rounded border flex items-center justify-center shrink-0 {selected.has(project.path) ? 'bg-brand-600 border-brand-600' : t.checkBg}">
                  {#if selected.has(project.path)}
                    <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke-width="3" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
                  {/if}
                </div>
                <div class="min-w-0 flex-1">
                  <div class="text-[13px] font-medium {t.textPrimary} truncate">{project.name}</div>
                  <div class="text-[12px] {textTertiary} truncate font-mono">{project.path}</div>
                </div>
                {#if project.has_git}
                  <span class="text-[11px] px-1.5 py-0.5 rounded {dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-500'}">git</span>
                {/if}
              </button>
            {/each}
          </div>

          <button
            class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50 mb-3"
            onclick={handleRegister}
            disabled={selectedCount === 0}
            data-testid="register-button"
          >
            Register {selectedCount} project{selectedCount !== 1 ? 's' : ''}
          </button>

          <button
            class="text-[13px] {t.linkColor} transition-colors"
            onclick={() => step = 2}
          >Browse again</button>
        {/if}
      </div>
    {/if}

    {#if step === 4}
      <!-- ═══ STEP 4: INDEXING PROGRESS ═══ -->
      <div class="text-center" data-testid="wizard-step-4">
        <h2 class="text-[18px] font-semibold {t.textPrimary} mb-4">Setting up taurhaus...</h2>

        <!-- Progress bar -->
        <div class="w-full h-2 rounded-full {dark ? 'bg-zinc-800' : 'bg-zinc-200'} overflow-hidden mb-3">
          <div
            class="h-full bg-brand-500 rounded-full transition-all duration-300"
            style="width: {progressTotal > 0 ? (progressIndex / progressTotal * 100) : 0}%"
          ></div>
        </div>

        <p class="text-[13px] {t.textSecondary}">
          {progressIndex} / {progressTotal} projects
        </p>
        {#if progressName}
          <p class="text-[12px] {textTertiary} mt-1">Indexing: {progressName}</p>
        {/if}
      </div>
    {/if}

    {#if step === 5}
      <!-- ═══ STEP 5: COMPLETION ═══ -->
      <div class="text-center" data-testid="wizard-step-5">
        <!-- Checkmark circle -->
        <div class="w-14 h-14 rounded-full bg-success-500/10 flex items-center justify-center mx-auto mb-5">
          <svg class="w-7 h-7 text-success-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
        </div>

        <h2 class="text-[18px] font-semibold {t.textPrimary} mb-2">
          {registeredCount} project{registeredCount !== 1 ? 's' : ''} registered
        </h2>
        {#if failedPaths.length > 0}
          <p class="text-[13px] {t.textSecondary} mb-2">{failedPaths.length} project{failedPaths.length !== 1 ? 's' : ''} could not be registered.</p>
          <div class="text-left mb-4 max-h-[120px] overflow-y-auto">
            {#each failedPaths as failed}
              <div class="text-[12px] text-danger-500 py-0.5 font-mono truncate" title={failed.error}>{failed.path}</div>
            {/each}
          </div>
        {:else}
          <p class="text-[13px] {t.textSecondary} mb-6">You're all set.</p>
        {/if}

        <button
          class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors"
          onclick={onComplete}
          data-testid="go-to-dashboard"
        >Go to dashboard</button>
      </div>
    {/if}

  </div>
</div>
