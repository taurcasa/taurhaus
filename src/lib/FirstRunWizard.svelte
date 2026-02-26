<script>
  import { scanDirectory, registerProjectsBatch, checkDaemonInstallStatus, installDaemon, isTauri } from './ipc.js'
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
  // 1=welcome, 2=daemon setup, 3=browse, 4=selection, 5=progress, 6=complete
  let step = $state(1)
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

  // Daemon setup state
  let daemonStatus = $state(null)
  let daemonChecking = $state(false)
  let daemonInstalling = $state(false)
  let daemonError = $state(null)

  const selectedCount = $derived(selected.size)

  // ═══ DAEMON SETUP ═══

  async function checkDaemon() {
    daemonChecking = true
    daemonError = null
    try {
      daemonStatus = await checkDaemonInstallStatus()
      // Auto-proceed if daemon is already installed and current
      if (daemonStatus.installed && !daemonStatus.needs_update) {
        setTimeout(() => { step = 3 }, 800)
      }
    } catch (e) {
      daemonError = e?.toString() || 'Failed to check daemon status'
    } finally {
      daemonChecking = false
    }
  }

  async function handleInstallDaemon() {
    daemonInstalling = true
    daemonError = null
    try {
      await installDaemon()
      // Re-check status after install
      daemonStatus = await checkDaemonInstallStatus()
      if (daemonStatus.installed && !daemonStatus.needs_update) {
        setTimeout(() => { step = 3 }, 800)
      }
    } catch (e) {
      daemonError = e?.toString() || 'Failed to install daemon'
    } finally {
      daemonInstalling = false
    }
  }

  // ═══ SCAN + REGISTER ═══

  async function handleScanPath() {
    if (!scanPath.trim()) return
    scanning = true
    scanError = null
    try {
      const results = await scanDirectory(scanPath.trim())
      discovered = results
      selected = new Set(results.filter(p => p.has_git).map(p => p.path))
      step = 4
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
    step = 5

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
      step = 6
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
          onclick={() => { step = 2; checkDaemon() }}
          data-testid="get-started-button"
        >Get started</button>
      </div>
    {/if}

    {#if step === 2}
      <!-- ═══ STEP 2: DAEMON SETUP ═══ -->
      <div data-testid="wizard-step-2">
        <h2 class="text-[18px] font-semibold {t.textPrimary} mb-1">Setup Helper Service</h2>
        <p class="text-[13px] {t.textSecondary} mb-5">
          taurhaus uses a helper service in WSL to watch your projects and detect AI sessions.
        </p>

        {#if daemonChecking}
          <!-- Checking state -->
          <div class="flex items-center gap-3 py-4 px-4 rounded-lg {dark ? 'bg-zinc-800/50' : 'bg-zinc-50'} mb-5" data-testid="daemon-checking">
            <svg class="w-5 h-5 {t.textSecondary} animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <span class="text-[13px] {t.textSecondary}">Checking daemon status...</span>
          </div>

        {:else if daemonStatus && !daemonStatus.wsl_available}
          <!-- No WSL -->
          <div class="py-4 px-4 rounded-lg border border-danger-500/30 {dark ? 'bg-danger-500/5' : 'bg-danger-50'} mb-5" data-testid="daemon-no-wsl">
            <div class="flex items-start gap-3">
              <svg class="w-5 h-5 text-danger-500 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z"/></svg>
              <div>
                <p class="text-[13px] font-medium text-danger-500 mb-1">WSL 2 is required</p>
                <p class="text-[12px] {t.textSecondary}">
                  {daemonStatus.error || 'WSL is not installed or not available.'}
                </p>
              </div>
            </div>
          </div>

        {:else if daemonStatus && daemonStatus.installed && !daemonStatus.needs_update}
          <!-- Already installed and current -->
          <div class="flex items-center gap-3 py-4 px-4 rounded-lg {dark ? 'bg-success-500/5' : 'bg-success-50'} border border-success-500/30 mb-5" data-testid="daemon-installed">
            <svg class="w-5 h-5 text-success-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
            <span class="text-[13px] {t.textPrimary}">
              Daemon v{daemonStatus.version} installed
            </span>
          </div>

        {:else if daemonInstalling}
          <!-- Installing -->
          <div class="flex items-center gap-3 py-4 px-4 rounded-lg {dark ? 'bg-zinc-800/50' : 'bg-zinc-50'} mb-5" data-testid="daemon-installing">
            <svg class="w-5 h-5 {t.textSecondary} animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <span class="text-[13px] {t.textSecondary}">Installing daemon...</span>
          </div>

        {:else if daemonError}
          <!-- Install failed -->
          <div class="py-4 px-4 rounded-lg border border-danger-500/30 {dark ? 'bg-danger-500/5' : 'bg-danger-50'} mb-5" data-testid="daemon-error">
            <div class="flex items-start gap-3">
              <svg class="w-5 h-5 text-danger-500 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z"/></svg>
              <div>
                <p class="text-[13px] font-medium text-danger-500 mb-1">Installation failed</p>
                <p class="text-[12px] {t.textSecondary} mb-2">{daemonError}</p>
                <p class="text-[12px] {textTertiary}">
                  Manual install: run <code class="font-mono text-[11px]">just install-daemon</code> in WSL
                </p>
              </div>
            </div>
          </div>
          <div class="flex gap-3 mb-5">
            <button
              class="flex-1 py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors"
              onclick={handleInstallDaemon}
              data-testid="daemon-retry-button"
            >Retry</button>
          </div>

        {:else if daemonStatus && daemonStatus.installed && daemonStatus.needs_update}
          <!-- Outdated -->
          <div class="flex items-center gap-3 py-4 px-4 rounded-lg border border-warning-500/30 {dark ? 'bg-warning-500/5' : 'bg-warning-50'} mb-5" data-testid="daemon-outdated">
            <svg class="w-5 h-5 text-warning-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z"/></svg>
            <span class="text-[13px] {t.textPrimary}">
              Update available: v{daemonStatus.version} → v{daemonStatus.bundled_version}
            </span>
          </div>
          <button
            class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors mb-5"
            onclick={handleInstallDaemon}
            data-testid="daemon-update-button"
          >Update</button>

        {:else if daemonStatus && !daemonStatus.installed}
          <!-- Not installed -->
          <div class="flex items-center gap-3 py-4 px-4 rounded-lg border {dark ? 'border-zinc-700 bg-zinc-800/50' : 'border-zinc-200 bg-zinc-50'} mb-5" data-testid="daemon-not-installed">
            <svg class="w-5 h-5 {t.textSecondary}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z"/></svg>
            <span class="text-[13px] {t.textPrimary}">Daemon not installed</span>
          </div>
          <button
            class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors mb-5"
            onclick={handleInstallDaemon}
            data-testid="daemon-install-button"
          >Install</button>
        {/if}

        <!-- Skip / Back buttons -->
        <div class="flex items-center justify-between">
          <button
            class="text-[13px] {t.linkColor} transition-colors"
            onclick={() => step = 1}
          >Back</button>
          {#if !daemonChecking && !(daemonStatus?.installed && !daemonStatus?.needs_update)}
            <button
              class="text-[13px] {t.linkColor} transition-colors"
              onclick={() => step = 3}
              data-testid="daemon-skip-button"
            >Skip for now</button>
          {/if}
        </div>
      </div>
    {/if}

    {#if step >= 3}
      <!-- Step 3 content kept mounted to preserve DirectoryBrowser tree state -->
      <div class:hidden={step !== 3} data-testid="wizard-step-3">
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
          onclick={() => step = 2}
        >Back</button>
      </div>
    {/if}

    {#if step === 4}
      <!-- ═══ STEP 4: PROJECT SELECTION ═══ -->
      <div data-testid="wizard-step-4">
        {#if discovered.length === 0}
          <!-- Empty scan results -->
          <div class="text-center" data-testid="empty-scan">
            <h2 class="text-[18px] font-semibold {t.textPrimary} mb-2">No projects found</h2>
            <p class="text-[13px] {t.textSecondary} mb-6">No git repositories were found in {scanPath}. Try a different directory.</p>
            <button
              class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors mb-3"
              onclick={() => step = 3}
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
            onclick={() => step = 3}
          >Browse again</button>
        {/if}
      </div>
    {/if}

    {#if step === 5}
      <!-- ═══ STEP 5: INDEXING PROGRESS ═══ -->
      <div class="text-center" data-testid="wizard-step-5">
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

    {#if step === 6}
      <!-- ═══ STEP 6: COMPLETION ═══ -->
      <div class="text-center" data-testid="wizard-step-6">
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
