<script>
  import { scanDirectory, registerProjectsBatch, listProjects } from './ipc.js'

  let { dark = false, onClose = () => {}, onProjectsAdded = () => {} } = $props()

  // Color tokens — same as Settings/Shell
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textBody      = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const linkColor     = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const modalBg       = $derived(dark ? 'bg-zinc-900' : 'bg-white')
  const checkBg       = $derived(dark ? 'bg-zinc-800 border-zinc-600' : 'bg-white border-zinc-300')
  const hoverRow      = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-white border-zinc-300 text-zinc-900')
  const badgeBg       = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-500')
  const registeredBg  = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')

  // State
  let scanning = $state(false)
  let discovered = $state([])
  let selected = $state(new Set())
  let registeredPaths = $state(new Set())
  let registering = $state(false)
  let progressIndex = $state(0)
  let progressTotal = $state(0)
  let scanError = $state(null)
  let manualMode = $state(false)
  let manualPath = $state('')
  let manualError = $state(null)
  let done = $state(false)
  let registeredCount = $state(0)

  const selectableProjects = $derived(discovered.filter(p => !registeredPaths.has(p.path)))
  const selectedCount = $derived(selected.size)
  const allSelected = $derived(selectableProjects.length > 0 && selected.size === selectableProjects.length)

  let dialogEl = $state(null)

  // Load registered projects to filter scan results, then auto-scan
  $effect(() => {
    loadRegisteredAndScan()
  })

  // Focus trap + escape key
  $effect(() => {
    if (!dialogEl) return
    dialogEl.focus()

    function handleKeydown(e) {
      if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeydown)
    return () => window.removeEventListener('keydown', handleKeydown)
  })

  async function loadRegisteredAndScan() {
    try {
      const existing = await listProjects()
      registeredPaths = new Set(existing.map(p => p.path))
    } catch {
      // If we can't load existing projects, proceed without filtering
    }
    await handleScan()
  }

  async function handleScan() {
    scanning = true
    scanError = null
    discovered = []
    selected = new Set()
    try {
      const results = await scanDirectory('~/projects')
      discovered = results
      // Pre-select all unregistered git repos
      selected = new Set(
        results.filter(p => p.has_git && !registeredPaths.has(p.path)).map(p => p.path)
      )
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
    selected = new Set(selectableProjects.map(p => p.path))
  }

  function deselectAll() {
    selected = new Set()
  }

  async function handleRegister() {
    const paths = [...selected]
    if (paths.length === 0) return
    registering = true
    progressTotal = paths.length
    progressIndex = 0

    try {
      const results = await registerProjectsBatch(paths)
      registeredCount = results.filter(r => r.success).length
      done = true
      onProjectsAdded()
    } catch (e) {
      scanError = e?.toString() || 'Registration failed'
    } finally {
      registering = false
    }
  }

  async function handleManualAdd() {
    const path = manualPath.trim()
    if (!path) return
    manualError = null
    registering = true
    progressTotal = 1
    progressIndex = 0

    try {
      const results = await registerProjectsBatch([path])
      const result = results[0]
      if (result?.success) {
        registeredCount = 1
        done = true
        onProjectsAdded()
      } else {
        manualError = result?.error || 'Failed to register project'
      }
    } catch (e) {
      manualError = e?.toString() || 'Registration failed'
    } finally {
      registering = false
    }
  }

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) {
      onClose()
    }
  }
</script>

<!-- Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
  onmousedown={handleBackdropClick}
>
  <!-- Modal -->
  <div
    bind:this={dialogEl}
    class="w-[600px] max-h-[calc(100vh-48px)] {modalBg} rounded-xl shadow-2xl flex flex-col overflow-hidden"
    role="dialog"
    aria-modal="true"
    aria-labelledby="add-project-title"
    tabindex="-1"
    data-testid="add-project-modal"
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-4 border-b {keyline}">
      <h2 id="add-project-title" class="text-[16px] font-semibold {textPrimary}">Add Projects</h2>
      <button
        class="w-7 h-7 flex items-center justify-center rounded-md {dark ? 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800' : 'text-zinc-400 hover:text-zinc-600 hover:bg-zinc-100'} transition-colors"
        onclick={onClose}
        aria-label="Close"
        data-testid="modal-close"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/></svg>
      </button>
    </div>

    <!-- Body -->
    <div class="flex-1 overflow-y-auto px-5 py-4">
      {#if done}
        <!-- Success -->
        <div class="text-center py-6" data-testid="registration-success">
          <div class="w-10 h-10 rounded-full bg-success-500/10 flex items-center justify-center mx-auto mb-3">
            <svg class="w-5 h-5 text-success-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
          </div>
          <h3 class="text-[15px] font-semibold {textPrimary} mb-1">
            {registeredCount} project{registeredCount !== 1 ? 's' : ''} added
          </h3>
          <p class="text-[13px] {textSecondary}">Your sidebar has been updated.</p>
        </div>

      {:else if registering}
        <!-- Registering progress -->
        <div class="text-center py-6" data-testid="registration-progress">
          <div class="w-full h-1.5 rounded-full {dark ? 'bg-zinc-800' : 'bg-zinc-200'} overflow-hidden mb-3">
            <div
              class="h-full bg-brand-500 rounded-full transition-all duration-300"
              style="width: {progressTotal > 0 ? (progressIndex / progressTotal * 100) : 50}%"
            ></div>
          </div>
          <p class="text-[13px] {textSecondary}">Registering projects...</p>
        </div>

      {:else if manualMode}
        <!-- Manual path entry -->
        <div>
          <label for="manual-path" class="text-[13px] {textSecondary} mb-1.5 block">Project path</label>
          <input
            id="manual-path"
            type="text"
            placeholder="~/projects/my-project"
            bind:value={manualPath}
            class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
            onkeydown={(e) => e.key === 'Enter' && handleManualAdd()}
            data-testid="manual-path-input"
          />
          {#if manualError}
            <p class="text-[12px] text-danger-500 mt-1.5">{manualError}</p>
          {/if}
        </div>

      {:else if scanning}
        <!-- Scanning -->
        <div class="text-center py-8" data-testid="scanning-state">
          <div class="w-5 h-5 border-2 border-brand-500 border-t-transparent rounded-full animate-spin mx-auto mb-3"></div>
          <p class="text-[14px] {textSecondary}">Scanning ~/projects/...</p>
        </div>

      {:else if scanError}
        <!-- Error -->
        <div class="text-center py-6" data-testid="scan-error">
          <p class="text-[14px] {textPrimary} mb-2">Scan failed</p>
          <p class="text-[12px] text-danger-500 mb-4">{scanError}</p>
          <button
            class="text-[13px] {linkColor} transition-colors"
            onclick={handleScan}
          >Try again</button>
        </div>

      {:else if selectableProjects.length === 0 && discovered.length > 0}
        <!-- All projects already registered -->
        <div class="text-center py-6" data-testid="all-registered">
          <p class="text-[14px] {textPrimary} mb-1">All projects already registered</p>
          <p class="text-[13px] {textSecondary}">Every project found in ~/projects/ is already in your sidebar.</p>
        </div>

      {:else if discovered.length === 0}
        <!-- Empty -->
        <div class="text-center py-6" data-testid="empty-scan">
          <p class="text-[14px] {textPrimary} mb-1">No projects found</p>
          <p class="text-[13px] {textSecondary}">No directories were found in ~/projects/.</p>
        </div>

      {:else}
        <!-- Scan results -->
        <div class="flex items-center gap-3 mb-3">
          <p class="text-[13px] {textSecondary}">
            Found {selectableProjects.length} new project{selectableProjects.length !== 1 ? 's' : ''}
            {#if registeredPaths.size > 0}
              <span class="text-[12px] {textTertiary}">({discovered.length - selectableProjects.length} already registered)</span>
            {/if}
          </p>
          <span class="flex-1"></span>
          {#if selectableProjects.length > 1}
            <button class="text-[12px] {linkColor} transition-colors" onclick={allSelected ? deselectAll : selectAll}>
              {allSelected ? 'Deselect all' : 'Select all'}
            </button>
          {/if}
        </div>

        <div class="border {keyline} rounded-lg overflow-hidden max-h-[320px] overflow-y-auto" data-testid="project-list">
          {#each discovered as project}
            {@const isRegistered = registeredPaths.has(project.path)}
            {@const isSelected = selected.has(project.path)}
            <button
              class="w-full flex items-center gap-3 px-3 py-2.5 text-left border-b last:border-b-0 {keyline} transition-colors {isRegistered ? 'opacity-50 cursor-default' : hoverRow}"
              onclick={() => !isRegistered && toggleProject(project.path)}
              disabled={isRegistered}
            >
              <div class="w-4 h-4 rounded border flex items-center justify-center shrink-0 {isRegistered ? 'border-transparent bg-transparent' : isSelected ? 'bg-brand-600 border-brand-600' : checkBg}">
                {#if isSelected && !isRegistered}
                  <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke-width="3" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
                {/if}
              </div>
              <div class="min-w-0 flex-1">
                <div class="text-[13px] font-medium {isRegistered ? registeredBg : textPrimary} truncate">{project.name}</div>
                <div class="text-[12px] {textTertiary} truncate font-mono">{project.path}</div>
              </div>
              {#if isRegistered}
                <span class="text-[11px] {registeredBg} italic">registered</span>
              {:else if project.has_git}
                <span class="text-[11px] px-1.5 py-0.5 rounded {badgeBg}">git</span>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="flex items-center justify-between px-5 py-3 border-t {keyline}">
      {#if done}
        <span></span>
        <button
          class="px-4 py-2 rounded-lg bg-brand-600 text-white text-[13px] font-medium hover:bg-brand-700 transition-colors"
          onclick={onClose}
          data-testid="done-button"
        >Done</button>
      {:else if manualMode}
        <button
          class="text-[13px] {linkColor} transition-colors"
          onclick={() => { manualMode = false; manualError = null }}
        >Back to scan results</button>
        <button
          class="px-4 py-2 rounded-lg bg-brand-600 text-white text-[13px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
          onclick={handleManualAdd}
          disabled={!manualPath.trim() || registering}
          data-testid="manual-add-button"
        >Add project</button>
      {:else if !scanning && !registering}
        <button
          class="text-[13px] {linkColor} transition-colors"
          onclick={() => manualMode = true}
        >Enter path manually</button>
        <button
          class="px-4 py-2 rounded-lg bg-brand-600 text-white text-[13px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
          onclick={handleRegister}
          disabled={selectedCount === 0}
          data-testid="register-button"
        >
          Register {selectedCount} project{selectedCount !== 1 ? 's' : ''}
        </button>
      {:else}
        <span></span>
        <span></span>
      {/if}
    </div>
  </div>
</div>
