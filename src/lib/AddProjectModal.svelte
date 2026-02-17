<script>
  import { scanDirectory, registerProjectsBatch, listProjects, removeProject, listDirectory, validateProjectPath } from './ipc.js'

  let { dark = false, onClose = () => {}, onProjectsChanged = () => {} } = $props()

  // Color tokens
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const linkColor     = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const modalBg       = $derived(dark ? 'bg-zinc-900' : 'bg-white')
  const checkBg       = $derived(dark ? 'bg-zinc-800 border-zinc-600' : 'bg-white border-zinc-300')
  const hoverRow      = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-white border-zinc-300 text-zinc-900')
  const badgeBg       = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-500')
  const sectionLabel  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

  // Activity dots (same as Shell sidebar)
  const dots = {
    active: 'bg-success-400',
    recent: 'bg-info-400',
    stale: 'bg-warning-400',
    dormant: 'bg-zinc-500',
  }

  // ═══ STATE ═══

  // Registered projects (top section)
  let registered = $state([])
  let loadingRegistered = $state(true)
  let confirmRemoveId = $state(null)
  let confirmTimeout = $state(null)
  let removingId = $state(null)

  // Add projects (bottom section)
  let showAddSection = $state(false)
  let scanning = $state(false)
  let discovered = $state([])
  let selected = $state(new Set())
  let registering = $state(false)
  let scanError = $state(null)
  let manualMode = $state(false)
  let manualPath = $state('')
  let manualError = $state(null)
  let addSuccess = $state(null) // "3 projects added" message

  // Directory tree state
  let treeChildren = $state({})   // { path: [{name, path, isExpandable}] }
  let treeExpanded = $state(new Set())
  let treeLoading = $state(new Set())
  let treeRoot = $state('~/projects')

  // Validation state
  let validation = $state(null)   // { exists, isGitRepo, isRegistered } or null
  let validating = $state(false)

  const registeredPaths = $derived(new Set(registered.map(p => p.path)))
  const selectableProjects = $derived(discovered.filter(p => !registeredPaths.has(p.path)))
  const selectedCount = $derived(selected.size)
  const allSelected = $derived(selectableProjects.length > 0 && selected.size === selectableProjects.length)

  let dialogEl = $state(null)

  // Load registered projects on mount
  $effect(() => {
    loadRegistered()
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

  async function loadRegistered() {
    loadingRegistered = true
    try {
      registered = await listProjects()
    } catch {
      registered = []
    } finally {
      loadingRegistered = false
    }
  }

  // ═══ REMOVE ═══

  function startRemove(id) {
    // Clear any existing timeout
    if (confirmTimeout) clearTimeout(confirmTimeout)

    confirmRemoveId = id
    // Auto-reset after 3 seconds
    confirmTimeout = setTimeout(() => {
      confirmRemoveId = null
      confirmTimeout = null
    }, 3000)
  }

  async function confirmRemove(id) {
    if (confirmTimeout) clearTimeout(confirmTimeout)
    confirmRemoveId = null
    confirmTimeout = null
    removingId = id

    try {
      await removeProject(id)
      registered = registered.filter(p => p.id !== id)
      onProjectsChanged()
    } catch (e) {
      console.error('Failed to remove project:', e)
    } finally {
      removingId = null
    }
  }

  // ═══ ADD: SCAN ═══

  async function handleScan() {
    scanning = true
    scanError = null
    discovered = []
    selected = new Set()
    addSuccess = null
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

    try {
      const results = await registerProjectsBatch(paths)
      const count = results.filter(r => r.success).length
      addSuccess = `${count} project${count !== 1 ? 's' : ''} added`
      // Refresh the registered list
      await loadRegistered()
      // Reset add state
      discovered = []
      selected = new Set()
      showAddSection = false
      onProjectsChanged()
    } catch (e) {
      scanError = e?.toString() || 'Registration failed'
    } finally {
      registering = false
    }
  }

  // ═══ ADD: MANUAL ═══

  async function handleManualAdd() {
    const path = manualPath.trim()
    if (!path || !pathIsValid) return
    manualError = null
    registering = true

    try {
      const results = await registerProjectsBatch([path])
      const result = results[0]
      if (result?.success) {
        addSuccess = '1 project added'
        manualPath = ''
        manualMode = false
        validation = null
        await loadRegistered()
        showAddSection = false
        onProjectsChanged()
      } else {
        manualError = result?.error || 'Failed to register project'
      }
    } catch (e) {
      manualError = e?.toString() || 'Registration failed'
    } finally {
      registering = false
    }
  }

  // ═══ DIRECTORY TREE ═══

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
    manualPath = dirPath
    validatePath(dirPath)
  }

  // Navigate up to parent directory
  function navigateUp() {
    if (treeRoot === '/') return
    // ~/projects → ~, ~ → /, /foo/bar → /foo, /foo → /
    let parent
    if (treeRoot === '~') {
      parent = '/'
    } else if (treeRoot.startsWith('~/')) {
      const parts = treeRoot.split('/')
      parent = parts.length <= 2 ? '~' : parts.slice(0, -1).join('/')
    } else {
      const parts = treeRoot.split('/')
      parent = parts.length <= 2 ? '/' : parts.slice(0, -1).join('/')
    }
    treeRoot = parent
    if (!treeChildren[parent]) {
      loadTreeDir(parent)
    }
    const next = new Set(treeExpanded)
    next.add(parent)
    treeExpanded = next
  }

  const canNavigateUp = $derived(treeRoot !== '/')

  // Load root on entering manual mode
  function initTree() {
    if (!treeChildren[treeRoot]) {
      loadTreeDir(treeRoot)
    }
    const next = new Set(treeExpanded)
    next.add(treeRoot)
    treeExpanded = next
  }

  // ═══ PATH VALIDATION ═══

  async function validatePath(path) {
    const trimmed = path.trim()
    if (!trimmed) {
      validation = null
      return
    }
    validating = true
    manualError = null
    try {
      validation = await validateProjectPath(trimmed)
    } catch {
      validation = null
    } finally {
      validating = false
    }
  }

  // Derived: is the path valid for registration?
  const pathIsValid = $derived(
    validation && validation.exists && validation.isGitRepo && !validation.isRegistered
  )

  // Validation message
  const validationMessage = $derived.by(() => {
    if (!validation || validating) return null
    if (!validation.exists) return { text: 'Directory not found', type: 'error' }
    if (!validation.isGitRepo) return { text: 'Not a git repository', type: 'error' }
    if (validation.isRegistered) return { text: 'Already registered', type: 'warning' }
    return { text: 'Valid git repository', type: 'success' }
  })

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
    aria-labelledby="manage-projects-title"
    tabindex="-1"
    data-testid="manage-projects-modal"
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-4 border-b {keyline}">
      <h2 id="manage-projects-title" class="text-[16px] font-semibold {textPrimary}">Manage Projects</h2>
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
    <div class="flex-1 overflow-y-auto">

      <!-- ═══ REGISTERED PROJECTS ═══ -->
      <div class="px-5 pt-4 pb-3">
        <h3 class="text-[11px] font-semibold uppercase tracking-wider {sectionLabel} mb-2">Registered Projects</h3>

        {#if loadingRegistered}
          <div class="py-4 text-center">
            <div class="w-4 h-4 border-2 border-brand-500 border-t-transparent rounded-full animate-spin mx-auto"></div>
          </div>
        {:else if registered.length === 0}
          <div class="py-4 text-center" data-testid="no-projects">
            <p class="text-[13px] {textTertiary}">No projects registered yet.</p>
          </div>
        {:else}
          <div class="border {keyline} rounded-lg overflow-hidden max-h-[260px] overflow-y-auto" data-testid="registered-list">
            {#each registered as project (project.id)}
              <div
                class="flex items-center gap-3 px-3 py-2 border-b last:border-b-0 {keyline} transition-all {removingId === project.id ? 'opacity-30' : ''}"
              >
                <span class="w-[7px] h-[7px] rounded-full shrink-0 {dots[project.activity_state] || 'bg-zinc-500'}"></span>
                <div class="min-w-0 flex-1">
                  <div class="text-[13px] font-medium {textPrimary} truncate">{project.name}</div>
                  <div class="text-[11px] {textTertiary} truncate font-mono">{project.path}</div>
                </div>
                {#if confirmRemoveId === project.id}
                  <button
                    class="text-[11px] text-danger-500 hover:text-danger-400 transition-colors shrink-0 font-medium"
                    onclick={() => confirmRemove(project.id)}
                    data-testid="confirm-remove-{project.id}"
                  >Confirm?</button>
                {:else}
                  <button
                    class="w-6 h-6 flex items-center justify-center rounded {dark ? 'text-zinc-600 hover:text-zinc-400 hover:bg-zinc-800' : 'text-zinc-300 hover:text-zinc-500 hover:bg-zinc-100'} transition-colors shrink-0"
                    onclick={() => startRemove(project.id)}
                    aria-label="Remove {project.name}"
                    data-testid="remove-{project.id}"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"/></svg>
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Success message (inline, not a separate state) -->
      {#if addSuccess}
        <div class="mx-5 mb-3 px-3 py-2 rounded-md {dark ? 'bg-success-500/10 text-success-400' : 'bg-success-50 text-success-700'} text-[12px] flex items-center gap-2" data-testid="add-success">
          <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
          {addSuccess}
        </div>
      {/if}

      <!-- ═══ ADD PROJECTS SECTION ═══ -->
      {#if !showAddSection}
        <div class="px-5 pb-4">
          <button
            class="w-full py-2 rounded-lg border border-dashed {keyline} text-[13px] {textTertiary} hover:border-brand-500 hover:text-brand-500 transition-colors flex items-center justify-center gap-2"
            onclick={() => { showAddSection = true; handleScan() }}
            data-testid="show-add-section"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>
            Add projects
          </button>
        </div>
      {:else}
        <div class="px-5 pb-4">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-[11px] font-semibold uppercase tracking-wider {sectionLabel}">Add Projects</h3>
            <button
              class="text-[11px] {textTertiary} hover:text-zinc-600 transition-colors"
              onclick={() => { showAddSection = false; manualMode = false; manualError = null; scanError = null }}
            >Close</button>
          </div>

          {#if manualMode}
            <!-- Manual path entry with directory browser -->
            <div>
              <label for="manual-path" class="text-[13px] {textSecondary} mb-1.5 block">Project path</label>
              <div class="relative">
                <input
                  id="manual-path"
                  type="text"
                  placeholder="~/projects/my-project"
                  bind:value={manualPath}
                  class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono pr-8"
                  onkeydown={(e) => e.key === 'Enter' && handleManualAdd()}
                  onblur={() => validatePath(manualPath)}
                  data-testid="manual-path-input"
                />
                {#if validating}
                  <div class="absolute right-2.5 top-1/2 -translate-y-1/2">
                    <div class="w-3.5 h-3.5 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
                  </div>
                {:else if validationMessage?.type === 'success'}
                  <div class="absolute right-2.5 top-1/2 -translate-y-1/2 text-success-500">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
                  </div>
                {/if}
              </div>

              <!-- Validation feedback -->
              {#if manualError}
                <p class="text-[12px] text-danger-500 mt-1.5" data-testid="manual-error">{manualError}</p>
              {:else if validationMessage}
                <p class="text-[12px] mt-1.5 {validationMessage.type === 'error' ? 'text-danger-500' : validationMessage.type === 'warning' ? 'text-warning-500' : 'text-success-500'}" data-testid="validation-message">{validationMessage.text}</p>
              {/if}

              <!-- Directory tree browser -->
              <div class="mt-3 border {keyline} rounded-lg max-h-[180px] overflow-y-auto" data-testid="directory-tree">
                {#snippet treeNode(entries, depth)}
                  {#each entries as entry}
                    <div>
                      <button
                        class="w-full flex items-center gap-1.5 px-2 h-[30px] text-left text-[13px] transition-colors
                          {manualPath === entry.path ? (dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700') : hoverRow + ' ' + textPrimary}"
                        style="padding-left: {8 + depth * 16}px"
                        onclick={() => selectTreeDir(entry.path)}
                        ondblclick={() => entry.isExpandable && toggleTreeDir(entry.path)}
                      >
                        <!-- Expand/collapse chevron -->
                        {#if entry.isExpandable}
                          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
                          <span
                            class="w-4 h-4 flex items-center justify-center shrink-0 cursor-pointer {dark ? 'text-zinc-500' : 'text-zinc-400'}"
                            onclick={(e) => { e.stopPropagation(); toggleTreeDir(entry.path) }}
                          >
                            <svg class="w-3 h-3 transition-transform {treeExpanded.has(entry.path) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
                          </span>
                        {:else}
                          <span class="w-4 shrink-0"></span>
                        {/if}
                        <!-- Folder icon -->
                        <svg class="w-3.5 h-3.5 shrink-0 {dark ? 'text-zinc-500' : 'text-zinc-400'}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z"/></svg>
                        <span class="truncate font-mono">{entry.name}</span>
                      </button>
                      <!-- Children (if expanded) -->
                      {#if treeExpanded.has(entry.path)}
                        {#if treeLoading.has(entry.path)}
                          <div class="flex items-center gap-2 h-[28px]" style="padding-left: {24 + depth * 16}px">
                            <div class="w-3 h-3 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
                            <span class="text-[11px] {textTertiary}">Loading...</span>
                          </div>
                        {:else if treeChildren[entry.path]?.length > 0}
                          {@render treeNode(treeChildren[entry.path], depth + 1)}
                        {:else if treeChildren[entry.path]}
                          <div class="h-[28px] flex items-center" style="padding-left: {24 + depth * 16}px">
                            <span class="text-[11px] {textTertiary}">Empty</span>
                          </div>
                        {/if}
                      {/if}
                    </div>
                  {/each}
                {/snippet}

                <!-- Navigate up -->
                {#if canNavigateUp}
                  <button
                    class="w-full flex items-center gap-1.5 px-2 h-[28px] text-left text-[12px] transition-colors {hoverRow} {textTertiary}"
                    onclick={navigateUp}
                    data-testid="tree-navigate-up"
                  >
                    <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 15.75 7.5-7.5 7.5 7.5"/></svg>
                    <span class="font-mono">..</span>
                  </button>
                {/if}
                <!-- Root directory -->
                <button
                  class="w-full flex items-center gap-1.5 px-2 h-[30px] text-left text-[13px] transition-colors font-medium
                    {manualPath === treeRoot ? (dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700') : hoverRow + ' ' + textPrimary}"
                  onclick={() => toggleTreeDir(treeRoot)}
                >
                  <span class="w-4 h-4 flex items-center justify-center shrink-0 {dark ? 'text-zinc-500' : 'text-zinc-400'}">
                    <svg class="w-3 h-3 transition-transform {treeExpanded.has(treeRoot) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
                  </span>
                  <svg class="w-3.5 h-3.5 shrink-0 {dark ? 'text-zinc-500' : 'text-zinc-400'}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z"/></svg>
                  <span class="truncate font-mono">{treeRoot}</span>
                </button>
                {#if treeExpanded.has(treeRoot)}
                  {#if treeLoading.has(treeRoot)}
                    <div class="flex items-center gap-2 h-[28px] pl-6">
                      <div class="w-3 h-3 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
                      <span class="text-[11px] {textTertiary}">Loading...</span>
                    </div>
                  {:else if treeChildren[treeRoot]?.length > 0}
                    {@render treeNode(treeChildren[treeRoot], 1)}
                  {:else if treeChildren[treeRoot]}
                    <div class="h-[28px] flex items-center pl-6">
                      <span class="text-[11px] {textTertiary}">No subdirectories found</span>
                    </div>
                  {/if}
                {/if}
              </div>

              <div class="flex items-center justify-between mt-3">
                <button
                  class="text-[12px] {linkColor} transition-colors"
                  onclick={() => { manualMode = false; manualError = null; validation = null }}
                >Back to scan</button>
                <button
                  class="px-3 py-1.5 rounded-md bg-brand-600 text-white text-[12px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
                  onclick={handleManualAdd}
                  disabled={!pathIsValid || registering}
                  data-testid="manual-add-button"
                >{registering ? 'Adding...' : 'Add project'}</button>
              </div>
            </div>

          {:else if scanning}
            <div class="text-center py-4" data-testid="scanning-state">
              <div class="w-4 h-4 border-2 border-brand-500 border-t-transparent rounded-full animate-spin mx-auto mb-2"></div>
              <p class="text-[12px] {textTertiary}">Scanning ~/projects/...</p>
            </div>

          {:else if scanError}
            <div class="text-center py-4" data-testid="scan-error">
              <p class="text-[13px] {textPrimary} mb-1">Scan failed</p>
              <p class="text-[11px] text-danger-500 mb-3">{scanError}</p>
              <button class="text-[12px] {linkColor} transition-colors" onclick={handleScan}>Try again</button>
            </div>

          {:else if selectableProjects.length === 0 && discovered.length > 0}
            <div class="text-center py-4" data-testid="all-registered">
              <p class="text-[13px] {textSecondary}">All projects in ~/projects/ are already registered.</p>
            </div>

          {:else if discovered.length === 0}
            <div class="text-center py-4" data-testid="empty-scan">
              <p class="text-[13px] {textSecondary}">No new projects found in ~/projects/.</p>
            </div>

          {:else}
            <!-- Scan results -->
            <div class="flex items-center gap-3 mb-2">
              <p class="text-[12px] {textSecondary}">
                {selectableProjects.length} new project{selectableProjects.length !== 1 ? 's' : ''}
              </p>
              <span class="flex-1"></span>
              {#if selectableProjects.length > 1}
                <button class="text-[11px] {linkColor} transition-colors" onclick={allSelected ? deselectAll : selectAll}>
                  {allSelected ? 'Deselect all' : 'Select all'}
                </button>
              {/if}
            </div>

            <div class="border {keyline} rounded-lg overflow-hidden max-h-[200px] overflow-y-auto" data-testid="discovered-list">
              {#each selectableProjects as project}
                {@const isSelected = selected.has(project.path)}
                <button
                  class="w-full flex items-center gap-3 px-3 py-2 text-left border-b last:border-b-0 {keyline} {hoverRow} transition-colors"
                  onclick={() => toggleProject(project.path)}
                >
                  <div class="w-4 h-4 rounded border flex items-center justify-center shrink-0 {isSelected ? 'bg-brand-600 border-brand-600' : checkBg}">
                    {#if isSelected}
                      <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke-width="3" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
                    {/if}
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="text-[13px] font-medium {textPrimary} truncate">{project.name}</div>
                    <div class="text-[11px] {textTertiary} truncate font-mono">{project.path}</div>
                  </div>
                  {#if project.has_git}
                    <span class="text-[10px] px-1.5 py-0.5 rounded {badgeBg}">git</span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}

          {#if !scanning && !manualMode && !scanError}
            <div class="flex items-center justify-between mt-3">
              <button
                class="text-[12px] {linkColor} transition-colors"
                onclick={() => { manualMode = true; initTree() }}
              >Enter path manually</button>
              {#if selectableProjects.length > 0}
                <button
                  class="px-3 py-1.5 rounded-md bg-brand-600 text-white text-[12px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
                  onclick={handleRegister}
                  disabled={selectedCount === 0 || registering}
                  data-testid="register-button"
                >
                  {registering ? 'Registering...' : `Register ${selectedCount}`}
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/if}

    </div>

    <!-- Footer -->
    <div class="flex items-center justify-end px-5 py-3 border-t {keyline}">
      <button
        class="px-4 py-2 rounded-lg bg-brand-600 text-white text-[13px] font-medium hover:bg-brand-700 transition-colors"
        onclick={onClose}
        data-testid="done-button"
      >Done</button>
    </div>
  </div>
</div>
