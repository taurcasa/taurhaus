<script>
  import { scanDirectory, registerProjectsBatch, listDirectory, getSystemRoots, isTauri } from './ipc.js'

  let { dark = false, onComplete = () => {} } = $props()

  // Color tokens
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textBody      = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const linkColor     = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const mainBg        = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const checkBg       = $derived(dark ? 'bg-zinc-800 border-zinc-600' : 'bg-white border-zinc-300')
  const hoverRow      = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-zinc-50 border-zinc-300 text-zinc-900')
  const treeBg        = $derived(dark ? 'bg-zinc-900/50' : 'bg-zinc-50')

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

  // ═══ DIRECTORY TREE BROWSER ═══

  let treeChildren = $state({})
  let treeExpanded = $state(new Set())
  let treeLoading = $state(new Set())
  let treeRoot = $state('~')
  let showDrives = $state(false)
  let systemRoots = $state([])

  async function loadTreeDir(dirPath) {
    const loadingSet = new Set(treeLoading)
    loadingSet.add(dirPath)
    treeLoading = loadingSet

    try {
      const entries = await listDirectory(dirPath)
      treeChildren = { ...treeChildren, [dirPath]: entries }
    } catch {
      treeChildren = { ...treeChildren, [dirPath]: [] }
    } finally {
      const done = new Set(treeLoading)
      done.delete(dirPath)
      treeLoading = done
    }
  }

  function toggleTreeDir(dirPath) {
    const next = new Set(treeExpanded)
    if (next.has(dirPath)) {
      next.delete(dirPath)
    } else {
      next.add(dirPath)
      if (!treeChildren[dirPath]) {
        loadTreeDir(dirPath)
      }
    }
    treeExpanded = next
  }

  function selectTreeDir(dirPath) {
    scanPath = dirPath
  }

  /** Check if path is a filesystem root (/ on Linux, C:\ on Windows, or WSL root) */
  function isSystemRoot(path) {
    if (path === '/') return true
    // Windows drive root: C:\ or C:/
    if (/^[A-Z]:[/\\]?$/.test(path)) return true
    // WSL root: \\wsl.localhost\Distro or \\wsl$\Distro (no further segments)
    if (/^\\\\wsl[.$]/.test(path)) {
      const segments = path.replace(/^\\\\/, '').split(/[/\\]/).filter(Boolean)
      return segments.length <= 2 // e.g. ["wsl.localhost", "Ubuntu"]
    }
    return false
  }

  async function navigateUp() {
    // If at a system root or ~, show the drives/roots picker
    if (treeRoot === '~' || isSystemRoot(treeRoot)) {
      systemRoots = await getSystemRoots()
      showDrives = true
      return
    }
    let parent
    if (treeRoot.startsWith('~/')) {
      const parts = treeRoot.split('/')
      parent = parts.length <= 2 ? '~' : parts.slice(0, -1).join('/')
    } else {
      // Handle both / and \ separators (Windows paths from backend use \)
      const normalized = treeRoot.replace(/\\/g, '/')
      const parts = normalized.split('/')
      if (parts.length <= 2) {
        // e.g. C:/Users → C:/
        parent = parts[0] + '/'
      } else {
        parent = parts.slice(0, -1).join('/')
      }
    }
    treeRoot = parent
    showDrives = false
    if (!treeChildren[parent]) {
      loadTreeDir(parent)
    }
    const next = new Set(treeExpanded)
    next.add(parent)
    treeExpanded = next
  }

  function selectDrive(drivePath) {
    treeRoot = drivePath
    showDrives = false
    treeExpanded = new Set()
    if (!treeChildren[drivePath]) {
      loadTreeDir(drivePath)
    }
    const next = new Set(treeExpanded)
    next.add(drivePath)
    treeExpanded = next
  }

  const canNavigateUp = $derived(true)

  async function initTree() {
    if (!treeChildren[treeRoot]) {
      loadTreeDir(treeRoot)
    }
    const next = new Set(treeExpanded)
    next.add(treeRoot)
    treeExpanded = next
    // Pre-fetch roots so we have them ready
    systemRoots = await getSystemRoots()
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

<div class="h-full {mainBg} flex items-center justify-center" data-testid="first-run-wizard" data-tauri-drag-region>
  <div class="max-w-[480px] w-full px-6">

    {#if step === 1}
      <!-- ═══ STEP 1: WELCOME ═══ -->
      <div class="text-center" data-testid="wizard-step-1">
        <!-- Logo -->
        <div class="w-14 h-14 rounded-xl bg-brand-500 flex items-center justify-center mx-auto mb-5">
          <span class="text-[22px] font-bold text-white leading-none">t</span>
        </div>

        <h1 class="text-[24px] font-semibold {textPrimary} mb-2">taurhaus</h1>
        <p class="text-[15px] {textSecondary} mb-1">AI Project Management</p>
        <p class="text-[13px] {textBody} mb-8 leading-relaxed">
          One clear view into all your projects — code, docs, progress, history — so you never lose context between sessions.
        </p>

        <button
          class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors"
          onclick={() => { step = 2; initTree() }}
          data-testid="get-started-button"
        >Get started</button>
      </div>

    {:else if step === 2}
      <!-- ═══ STEP 2: BROWSE FOR PROJECTS FOLDER ═══ -->
      <div data-testid="wizard-step-2">
        <h2 class="text-[18px] font-semibold {textPrimary} mb-1">Where are your projects?</h2>
        <p class="text-[13px] {textSecondary} mb-4">Browse to the folder that contains your project directories, or type the path directly.</p>

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
        <div class="border {keyline} rounded-lg overflow-hidden mb-4 max-h-[280px] overflow-y-auto {treeBg}">

          {#if showDrives}
            <!-- Drive/root selector -->
            {#each systemRoots as root}
              <button
                class="w-full flex items-center gap-2 px-3 py-2 text-left text-[13px] font-mono transition-colors {hoverRow} {textPrimary}"
                onclick={() => selectDrive(root.path)}
              >
                <svg class="w-4 h-4 shrink-0 {textTertiary}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21.75 17.25v-.228a4.5 4.5 0 0 0-.12-1.03l-2.268-9.64a3.375 3.375 0 0 0-3.285-2.602H7.923a3.375 3.375 0 0 0-3.285 2.602l-2.268 9.64a4.5 4.5 0 0 0-.12 1.03v.228m19.5 0a3 3 0 0 1-3 3H5.25a3 3 0 0 1-3-3m19.5 0a3 3 0 0 0-3-3H5.25a3 3 0 0 0-3 3m16.5 0h.008v.008h-.008v-.008Zm-3 0h.008v.008h-.008v-.008Z"/></svg>
                <span>{root.name}</span>
              </button>
            {/each}
            {#if systemRoots.length === 0}
              <div class="text-[12px] {textTertiary} py-3 px-3">Loading drives...</div>
            {/if}
          {:else}

            {#snippet treeNode(entries, depth)}
              {#each entries as entry}
                <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
                <div>
                  <button
                    class="w-full flex items-center gap-1.5 px-2 py-1 text-left text-[12px] font-mono transition-colors
                      {scanPath === entry.path ? (dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700') : hoverRow + ' ' + textPrimary}"
                    style="padding-left: {depth * 16 + 8}px"
                    onclick={() => selectTreeDir(entry.path)}
                    ondblclick={() => entry.isExpandable && toggleTreeDir(entry.path)}
                  >
                    {#if entry.isExpandable}
                      <span
                        class="w-4 h-4 flex items-center justify-center shrink-0 rounded hover:bg-white/10"
                        onclick={(e) => { e.stopPropagation(); toggleTreeDir(entry.path) }}
                      >
                        <svg class="w-3 h-3 transition-transform {treeExpanded.has(entry.path) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
                      </span>
                    {:else}
                      <span class="w-4 h-4 shrink-0"></span>
                    {/if}
                    <span class="truncate">{entry.name}</span>
                  </button>

                  {#if treeExpanded.has(entry.path)}
                    {#if treeLoading.has(entry.path)}
                      <div class="text-[11px] {textTertiary} py-1" style="padding-left: {(depth + 1) * 16 + 28}px">Loading...</div>
                    {:else if treeChildren[entry.path]?.length > 0}
                      {@render treeNode(treeChildren[entry.path], depth + 1)}
                    {:else if treeChildren[entry.path]}
                      <div class="text-[11px] {textTertiary} py-1" style="padding-left: {(depth + 1) * 16 + 28}px">Empty</div>
                    {/if}
                  {/if}
                </div>
              {/each}
            {/snippet}

            <!-- Navigate up -->
            <button
              class="w-full flex items-center gap-1.5 px-2 py-1 text-left text-[12px] font-mono {hoverRow} {textTertiary} transition-colors"
              onclick={navigateUp}
            >
              <span class="w-4 h-4 flex items-center justify-center shrink-0">
                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 15.75l7.5-7.5 7.5 7.5"/></svg>
              </span>
              ..
            </button>

            <!-- Root entry -->
            <button
              class="w-full flex items-center gap-1.5 px-2 py-1 text-left text-[12px] font-mono transition-colors
                {scanPath === treeRoot ? (dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700') : hoverRow + ' ' + textPrimary}"
              onclick={() => selectTreeDir(treeRoot)}
              ondblclick={() => toggleTreeDir(treeRoot)}
            >
              <span
                class="w-4 h-4 flex items-center justify-center shrink-0 rounded hover:bg-white/10"
                onclick={(e) => { e.stopPropagation(); toggleTreeDir(treeRoot) }}
              >
                <svg class="w-3 h-3 transition-transform {treeExpanded.has(treeRoot) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
              </span>
              <span class="truncate font-mono">{treeRoot}</span>
            </button>
            {#if treeExpanded.has(treeRoot)}
              {#if treeLoading.has(treeRoot)}
                <div class="text-[11px] {textTertiary} py-1 pl-9">Loading...</div>
              {:else if treeChildren[treeRoot]?.length > 0}
                {@render treeNode(treeChildren[treeRoot], 1)}
              {:else if treeChildren[treeRoot]}
                <div class="text-[11px] {textTertiary} py-1 pl-9">Empty</div>
              {/if}
            {/if}
          {/if}
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
          class="text-[13px] {linkColor} transition-colors"
          onclick={() => step = 1}
        >Back</button>
      </div>

    {:else if step === 3}
      <!-- ═══ STEP 3: PROJECT SELECTION ═══ -->
      <div data-testid="wizard-step-3">
        {#if discovered.length === 0}
          <!-- Empty scan results -->
          <div class="text-center" data-testid="empty-scan">
            <h2 class="text-[18px] font-semibold {textPrimary} mb-2">No projects found</h2>
            <p class="text-[13px] {textSecondary} mb-6">No git repositories were found in {scanPath}. Try a different directory.</p>
            <button
              class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors mb-3"
              onclick={() => step = 2}
            >Browse again</button>
          </div>
        {:else}
          <h2 class="text-[18px] font-semibold {textPrimary} mb-1">
            Found {discovered.length} repositor{discovered.length === 1 ? 'y' : 'ies'}
          </h2>
          <p class="text-[13px] {textSecondary} mb-4">in {scanPath}</p>

          <!-- Select all / Deselect all -->
          <div class="flex items-center gap-3 mb-3">
            <button
              class="text-[12px] {linkColor} transition-colors"
              onclick={selectAll}
            >Select all</button>
            <span class="text-[12px] {textTertiary}">|</span>
            <button
              class="text-[12px] {linkColor} transition-colors"
              onclick={deselectAll}
            >Deselect all</button>
            <span class="flex-1"></span>
            <span class="text-[12px] {textTertiary}">{selectedCount} selected</span>
          </div>

          <!-- Project list -->
          <div class="border {keyline} rounded-lg overflow-hidden mb-4 max-h-[320px] overflow-y-auto">
            {#each discovered as project}
              <button
                class="w-full flex items-center gap-3 px-3 py-2.5 text-left border-b last:border-b-0 {keyline} {hoverRow} transition-colors"
                onclick={() => toggleProject(project.path)}
              >
                <div class="w-4 h-4 rounded border flex items-center justify-center shrink-0 {selected.has(project.path) ? 'bg-brand-600 border-brand-600' : checkBg}">
                  {#if selected.has(project.path)}
                    <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke-width="3" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
                  {/if}
                </div>
                <div class="min-w-0 flex-1">
                  <div class="text-[13px] font-medium {textPrimary} truncate">{project.name}</div>
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
            class="text-[13px] {linkColor} transition-colors"
            onclick={() => step = 2}
          >Browse again</button>
        {/if}
      </div>

    {:else if step === 4}
      <!-- ═══ STEP 4: INDEXING PROGRESS ═══ -->
      <div class="text-center" data-testid="wizard-step-4">
        <h2 class="text-[18px] font-semibold {textPrimary} mb-4">Setting up taurhaus...</h2>

        <!-- Progress bar -->
        <div class="w-full h-2 rounded-full {dark ? 'bg-zinc-800' : 'bg-zinc-200'} overflow-hidden mb-3">
          <div
            class="h-full bg-brand-500 rounded-full transition-all duration-300"
            style="width: {progressTotal > 0 ? (progressIndex / progressTotal * 100) : 0}%"
          ></div>
        </div>

        <p class="text-[13px] {textSecondary}">
          {progressIndex} / {progressTotal} projects
        </p>
        {#if progressName}
          <p class="text-[12px] {textTertiary} mt-1">Indexing: {progressName}</p>
        {/if}
      </div>

    {:else if step === 5}
      <!-- ═══ STEP 5: COMPLETION ═══ -->
      <div class="text-center" data-testid="wizard-step-5">
        <!-- Checkmark circle -->
        <div class="w-14 h-14 rounded-full bg-success-500/10 flex items-center justify-center mx-auto mb-5">
          <svg class="w-7 h-7 text-success-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
        </div>

        <h2 class="text-[18px] font-semibold {textPrimary} mb-2">
          {registeredCount} project{registeredCount !== 1 ? 's' : ''} registered
        </h2>
        {#if failedPaths.length > 0}
          <p class="text-[13px] {textSecondary} mb-2">{failedPaths.length} project{failedPaths.length !== 1 ? 's' : ''} could not be registered.</p>
          <div class="text-left mb-4 max-h-[120px] overflow-y-auto">
            {#each failedPaths as failed}
              <div class="text-[12px] text-danger-500 py-0.5 font-mono truncate" title={failed.error}>{failed.path}</div>
            {/each}
          </div>
        {:else}
          <p class="text-[13px] {textSecondary} mb-6">You're all set.</p>
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
