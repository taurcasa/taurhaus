<script>
  import { focusFirstInteractiveElement, handleModalKeydown, registerModalLayer } from './a11y.js'
  import {
    scanDirectory,
    registerProjectsBatch,
    listProjects,
    removeProject,
    validateProjectPath,
    createProject,
    getSettings,
    updateSettings,
  } from './ipc.js'
  import CreateWorkflow from './CreateWorkflow.svelte'
  import ManualWorkflow from './ManualWorkflow.svelte'
  import ScanWorkflow from './ScanWorkflow.svelte'
  import { describeScanDirectoryError } from './errorCopy.js'
  import { formatUserFacingError } from './format.js'
  import { themeTokens } from './themeTokens.js'

  let {
    dark = false,
    onClose = () => {},
    onProjectsChanged = () => {},
    onProjectCreated = () => {},
  } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const modalBg       = $derived(dark ? 'bg-zinc-900' : 'bg-white')
  const hoverRow      = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-white border-zinc-300 text-zinc-900')
  const badgeBg       = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-500')
  const sectionLabel  = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')

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
  const WORKFLOW_STATES = {
    SCAN: 'scan',
    MANUAL: 'manual',
    CREATE: 'create',
  }
  const DEFAULT_CREATE_PARENT_DIR = '~/projects'
  let addMode = $state(WORKFLOW_STATES.SCAN)
  let manualPath = $state('')
  let manualError = $state(null)
  let addSuccess = $state(null) // "3 projects added" message
  let addFailureSummary = $state(null)
  let addFailureDetails = $state([])
  let createProjectName = $state('')
  let createParentDir = $state(DEFAULT_CREATE_PARENT_DIR)
  let createError = $state(null)
  let creating = $state(false)
  let settingsCache = $state(null)
  let rememberedProjectDialogPath = $state('')

  // Validation state
  let validation = $state(null)   // { exists, isGitRepo, isRegistered } or null
  let validating = $state(false)

  const registeredPaths = $derived(new Set(registered.map(p => p.path)))
  const selectableProjects = $derived(discovered.filter(p => !registeredPaths.has(p.path)))
  const selectedCount = $derived(selected.size)
  const allSelected = $derived(selectableProjects.length > 0 && selected.size === selectableProjects.length)

  let dialogEl = $state(null)
  let modalRootEl = $state(null)
  let restoreFocusElement = null

  // Load registered projects on mount
  $effect(() => {
    loadRegistered()
    loadProjectDialogPathMemory()
  })

  $effect(() => {
    return () => {
      if (confirmTimeout) {
        clearTimeout(confirmTimeout)
        confirmTimeout = null
      }
    }
  })

  // Keyboard trap + escape key
  $effect(() => {
    if (!dialogEl || !modalRootEl) return
    if (
      !restoreFocusElement
      && document.activeElement instanceof HTMLElement
      && !modalRootEl.contains(document.activeElement)
    ) {
      restoreFocusElement = document.activeElement
    }

    const unregisterModal = registerModalLayer(modalRootEl)
    focusFirstInteractiveElement(dialogEl)

    function handleKeydown(e) {
      handleModalKeydown(e, dialogEl, onClose)
    }
    window.addEventListener('keydown', handleKeydown)
    return () => {
      unregisterModal()
      window.removeEventListener('keydown', handleKeydown)
      if (restoreFocusElement?.isConnected) {
        restoreFocusElement.focus()
      }
      restoreFocusElement = null
    }
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

  async function loadProjectDialogPathMemory() {
    try {
      const settings = await getSettings()
      settingsCache = settings
      const remembered = settings?.project_dialog_last_path?.trim() ?? ''
      if (!remembered) return
      rememberedProjectDialogPath = remembered
      if (!manualPath.trim()) {
        manualPath = remembered
      }
      if (!createParentDir.trim() || createParentDir === DEFAULT_CREATE_PARENT_DIR) {
        createParentDir = remembered
      }
    } catch (error) {
      console.error('[projects] failed to load dialog path memory:', error)
    }
  }

  async function persistProjectDialogPathMemory(rawPath) {
    const path = rawPath.trim()
    if (!path || path === rememberedProjectDialogPath) return

    try {
      const settings = settingsCache ?? await getSettings()
      const next = await updateSettings({
        ...settings,
        project_dialog_last_path: path,
      })
      settingsCache = next
      rememberedProjectDialogPath = next.project_dialog_last_path ?? path
    } catch (error) {
      console.error('[projects] failed to persist dialog path memory:', error)
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
    addFailureSummary = null
    addFailureDetails = []
    try {
      const results = await scanDirectory('~/projects')
      discovered = results
      // Pre-select all unregistered git repos
      selected = new Set(
        results.filter(p => p.has_git && !registeredPaths.has(p.path)).map(p => p.path)
      )
    } catch (e) {
      scanError = describeScanDirectoryError(e)
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
    addFailureSummary = null
    addFailureDetails = []

    try {
      const results = await registerProjectsBatch(paths)
      const succeeded = results.filter((result) => result.success)
      const failed = results
        .filter((result) => !result.success)
        .map((result) => ({
          path: result.path,
          error: formatUserFacingError(result.error, 'Could not add this project.'),
        }))
      const count = succeeded.length
      addSuccess = count > 0 ? `${count} project${count !== 1 ? 's' : ''} added` : null
      addFailureSummary = failed.length > 0
        ? `${failed.length} project${failed.length !== 1 ? 's' : ''} could not be added`
        : null
      addFailureDetails = failed
      await loadRegistered()
      if (count > 0) {
        onProjectsChanged()
      }
      if (failed.length === 0) {
        discovered = []
        selected = new Set()
        showAddSection = false
      } else {
        selected = new Set(failed.map((result) => result.path))
      }
    } catch (e) {
      scanError = formatUserFacingError(e, 'Registration failed')
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
        await persistProjectDialogPathMemory(path)
        addSuccess = '1 project added'
        manualPath = ''
        addMode = WORKFLOW_STATES.SCAN
        validation = null
        await loadRegistered()
        showAddSection = false
        onProjectsChanged()
      } else {
        manualError = result?.error || 'Failed to register project'
      }
    } catch (e) {
      manualError = formatUserFacingError(e, 'Registration failed')
    } finally {
      registering = false
    }
  }

  // ═══ ADD: CREATE NEW ═══

  function isValidProjectName(name) {
    const trimmed = name.trim()
    if (!trimmed) return false
    if (trimmed === '.' || trimmed === '..') return false
    return !(/[\\/]/.test(trimmed) || /\0/.test(trimmed))
  }

  function joinProjectPath(parent, name) {
    const trimmedParent = parent.trim()
    if (trimmedParent === '/') return `/${name}`
    const normalized = trimmedParent.replace(/[\\/]+$/, '')
    if (normalized.includes('\\')) return `${normalized}\\${name}`
    return `${normalized}/${name}`
  }

  async function handleCreateProject() {
    const name = createProjectName.trim()
    const parent = createParentDir.trim()
    createError = null

    if (!isValidProjectName(name)) {
      createError = 'Enter a valid project name'
      return
    }
    if (!parent) {
      createError = 'Choose a parent directory'
      return
    }

    creating = true
    try {
      const parentValidation = await validateProjectPath(parent)
      if (!parentValidation?.exists) {
        createError = 'Parent directory not found'
        return
      }

      const targetPath = joinProjectPath(parent, name)
      const targetValidation = await validateProjectPath(targetPath)
      if (targetValidation?.exists) {
        createError = 'Target directory already exists'
        return
      }

      const created = await createProject(name, parent)
      await persistProjectDialogPathMemory(parent)
      await loadRegistered()
      onProjectsChanged()
      onProjectCreated(created)
      onClose()
    } catch (e) {
      createError = formatUserFacingError(e, 'Failed to create project')
    } finally {
      creating = false
    }
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

  function transitionWorkflow(nextMode) {
    addMode = nextMode
    if (nextMode !== WORKFLOW_STATES.MANUAL) {
      manualError = null
      validation = null
    }
    if (nextMode !== WORKFLOW_STATES.CREATE) {
      createError = null
    }
    if (nextMode !== WORKFLOW_STATES.SCAN) {
      scanError = null
    }
  }
</script>

<!-- Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  bind:this={modalRootEl}
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
  onmousedown={handleBackdropClick}
  data-shell-overlay
  data-testid="manage-projects-backdrop"
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
    <div class="flex items-center justify-between px-5 py-4 border-b {t.keyline}">
      <h2 id="manage-projects-title" class="text-[16px] font-semibold {t.textPrimary}">Manage Projects</h2>
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
            <p class="text-[13px] {t.textTertiary}">No projects registered yet.</p>
          </div>
        {:else}
          <div class="border {t.keyline} rounded-lg overflow-hidden max-h-[260px] overflow-y-auto" data-testid="registered-list">
            {#each registered as project (project.id)}
              <div
                class="flex items-center gap-3 px-3 py-2 border-b last:border-b-0 {t.keyline} transition-all {removingId === project.id ? 'opacity-30' : ''}"
              >
                <span class="w-[7px] h-[7px] rounded-full shrink-0 {dots[project.activityState] || 'bg-zinc-500'}"></span>
                <div class="min-w-0 flex-1">
                  <div class="text-[13px] font-medium {t.textPrimary} truncate">{project.name}</div>
                  <div class="text-[11px] {t.textTertiary} truncate font-mono">{project.path}</div>
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

      {#if addFailureSummary}
        <div class="mx-5 mb-3 rounded-md border border-warning-500/30 {dark ? 'bg-warning-500/10 text-warning-200' : 'bg-warning-50 text-warning-900'} px-3 py-2 text-[12px]" data-testid="add-failure-summary">
          <div class="flex items-start gap-2">
            <svg class="mt-0.5 h-3.5 w-3.5 shrink-0 text-warning-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z"/></svg>
            <div class="min-w-0 flex-1">
              <p>{addFailureSummary}.</p>
              {#if addFailureDetails.length > 0}
                <details class="mt-2" data-testid="add-failure-details">
                  <summary class="cursor-pointer font-medium">Show failed paths</summary>
                  <div class="mt-2 space-y-1">
                    {#each addFailureDetails as failed}
                      <div class="rounded border border-white/10 px-2 py-1">
                        <div class="truncate font-mono text-[11px]">{failed.path}</div>
                        <div class="mt-0.5 text-[11px] {dark ? 'text-warning-100/80' : 'text-warning-900/80'}">{failed.error}</div>
                      </div>
                    {/each}
                  </div>
                </details>
              {/if}
            </div>
          </div>
        </div>
      {/if}

      <!-- ═══ ADD PROJECTS SECTION ═══ -->
      {#if !showAddSection}
        <div class="px-5 pb-4">
          <button
            class="w-full py-2 rounded-lg border border-dashed {t.keyline} text-[13px] {t.textTertiary} hover:border-brand-500 hover:text-brand-500 transition-colors flex items-center justify-center gap-2"
            onclick={() => {
                showAddSection = true
                transitionWorkflow(WORKFLOW_STATES.SCAN)
                manualError = null
                scanError = null
                createError = null
                handleScan()
            }}
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
              class="text-[11px] {t.textTertiary} hover:text-zinc-600 transition-colors"
              onclick={() => {
                showAddSection = false
                transitionWorkflow(WORKFLOW_STATES.SCAN)
                manualError = null
                scanError = null
                createError = null
              }}
            >Close</button>
          </div>

          <div class="grid grid-cols-3 gap-2 mb-3">
            <button
              class="h-8 rounded-md text-[12px] font-medium border transition-colors {addMode === WORKFLOW_STATES.SCAN ? 'bg-brand-600 text-white border-brand-600' : dark ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800' : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'}"
              onclick={() => { transitionWorkflow(WORKFLOW_STATES.SCAN); if (discovered.length === 0 && !scanning) handleScan() }}
              data-testid="mode-scan"
            >Scan</button>
            <button
              class="h-8 rounded-md text-[12px] font-medium border transition-colors {addMode === WORKFLOW_STATES.MANUAL ? 'bg-brand-600 text-white border-brand-600' : dark ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800' : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'}"
              onclick={() => { transitionWorkflow(WORKFLOW_STATES.MANUAL) }}
              data-testid="mode-manual"
            >Manual</button>
            <button
              class="h-8 rounded-md text-[12px] font-medium border transition-colors {addMode === WORKFLOW_STATES.CREATE ? 'bg-brand-600 text-white border-brand-600' : dark ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800' : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'}"
              onclick={() => { transitionWorkflow(WORKFLOW_STATES.CREATE) }}
              data-testid="mode-create"
            >Create New</button>
          </div>

          {#if addMode === WORKFLOW_STATES.MANUAL}
            <ManualWorkflow
              {dark}
              {t}
              {inputBg}
              {manualPath}
              {validating}
              {validationMessage}
              {manualError}
              {pathIsValid}
              {registering}
              onManualPathInput={(value) => {
                manualPath = value
              }}
              onManualPathBlur={() => {
                manualPath = manualPath.trim()
                validatePath(manualPath)
                persistProjectDialogPathMemory(manualPath)
              }}
              onManualEnter={handleManualAdd}
              onManualAdd={handleManualAdd}
              onBackToScan={() => transitionWorkflow(WORKFLOW_STATES.SCAN)}
              onManualDirectorySelect={(path) => {
                manualPath = path
                validatePath(path)
                persistProjectDialogPathMemory(path)
              }}
            />
          {:else if addMode === WORKFLOW_STATES.CREATE}
            <CreateWorkflow
              {dark}
              {t}
              {inputBg}
              {createProjectName}
              {createParentDir}
              {createError}
              {creating}
              canCreate={isValidProjectName(createProjectName) && Boolean(createParentDir.trim())}
              onCreateNameInput={(value) => {
                createProjectName = value
              }}
              onCreateParentInput={(value) => {
                createParentDir = value
              }}
              onCreateParentBlur={() => {
                createParentDir = createParentDir.trim()
                persistProjectDialogPathMemory(createParentDir)
              }}
              onCreateEnter={handleCreateProject}
              onCreateParentSelect={(path) => {
                createParentDir = path
                persistProjectDialogPathMemory(path)
              }}
              onCreateProject={handleCreateProject}
              onBackToScan={() => transitionWorkflow(WORKFLOW_STATES.SCAN)}
            />
          {:else}
            <ScanWorkflow
              {dark}
              {t}
              {hoverRow}
              {badgeBg}
              {scanning}
              {scanError}
              {discovered}
              {selectableProjects}
              {selected}
              {selectedCount}
              {allSelected}
              {registering}
              onToggleProject={toggleProject}
              onSelectAll={selectAll}
              onDeselectAll={deselectAll}
              onRegister={handleRegister}
              onEnterManualMode={() => transitionWorkflow(WORKFLOW_STATES.MANUAL)}
              onEnterCreateMode={() => transitionWorkflow(WORKFLOW_STATES.CREATE)}
              onRetryScan={handleScan}
            />
          {/if}
        </div>
      {/if}

    </div>

    <!-- Footer -->
    <div class="flex items-center justify-end px-5 py-3 border-t {t.keyline}">
      <button
        class="px-4 py-2 rounded-lg bg-brand-600 text-white text-[13px] font-medium hover:bg-brand-700 transition-colors"
        onclick={onClose}
        data-testid="done-button"
      >Done</button>
    </div>
  </div>
</div>
