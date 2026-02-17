<script>
  import { scanDirectory, registerProjectsBatch, isFirstRun } from './ipc.js'

  let { dark = false, onComplete = () => {} } = $props()

  // Color tokens
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textBody      = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const linkColor     = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const cardBg        = $derived(dark ? 'bg-zinc-900' : 'bg-zinc-50')
  const mainBg        = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const checkBg       = $derived(dark ? 'bg-zinc-800 border-zinc-600' : 'bg-white border-zinc-300')
  const hoverRow      = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-zinc-50 border-zinc-300 text-zinc-900')

  // Wizard state
  let step = $state(1)  // 1=welcome, 2=selection, 3=progress, 4=complete
  let scanning = $state(false)
  let discovered = $state([])
  let selected = $state(new Set())
  let registering = $state(false)
  let progressIndex = $state(0)
  let progressTotal = $state(0)
  let progressName = $state('')
  let registeredCount = $state(0)
  let manualMode = $state(false)
  let manualPath = $state('')
  let scanError = $state(null)

  const selectedCount = $derived(selected.size)
  const allSelected = $derived(discovered.length > 0 && selected.size === discovered.length)

  async function handleScan() {
    scanning = true
    scanError = null
    try {
      const results = await scanDirectory('~/projects')
      discovered = results
      // Pre-select all git repos
      selected = new Set(results.filter(p => p.has_git).map(p => p.path))
      step = 2
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
    step = 3

    try {
      const results = await registerProjectsBatch(paths)
      registeredCount = results.filter(r => r.success).length
      step = 4
      // Auto-transition after 2 seconds
      setTimeout(() => {
        onComplete()
      }, 2000)
    } catch (e) {
      console.error('Registration failed:', e)
    } finally {
      registering = false
    }
  }

  async function handleManualAdd() {
    if (!manualPath.trim()) return
    registering = true
    step = 3
    progressTotal = 1
    progressIndex = 0
    progressName = manualPath.trim()

    try {
      const results = await registerProjectsBatch([manualPath.trim()])
      registeredCount = results.filter(r => r.success).length
      step = 4
      setTimeout(() => {
        onComplete()
      }, 2000)
    } catch (e) {
      console.error('Registration failed:', e)
    } finally {
      registering = false
    }
  }
</script>

<div class="h-full {mainBg} flex items-center justify-center" data-testid="first-run-wizard">
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

        {#if manualMode}
          <div class="text-left mb-4">
            <label for="manual-path" class="text-[13px] {textSecondary} mb-1.5 block">Project path</label>
            <input
              id="manual-path"
              type="text"
              placeholder="/home/user/my-project"
              bind:value={manualPath}
              class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
              onkeydown={(e) => e.key === 'Enter' && handleManualAdd()}
            />
          </div>
          <button
            class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50 mb-3"
            onclick={handleManualAdd}
            disabled={!manualPath.trim() || registering}
          >Add project</button>
          <button
            class="text-[13px] {linkColor} transition-colors"
            onclick={() => manualMode = false}
          >Back to scan</button>
        {:else}
          <button
            class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50 mb-3"
            onclick={handleScan}
            disabled={scanning}
            data-testid="scan-button"
          >
            {scanning ? 'Scanning...' : 'Scan ~/projects/'}
          </button>

          {#if scanError}
            <p class="text-[12px] text-danger-500 mb-2">{scanError}</p>
          {/if}

          <button
            class="text-[13px] {linkColor} transition-colors"
            onclick={() => manualMode = true}
            data-testid="manual-add-link"
          >Or add a project manually</button>
        {/if}
      </div>

    {:else if step === 2}
      <!-- ═══ STEP 2: PROJECT SELECTION ═══ -->
      <div data-testid="wizard-step-2">
        <h2 class="text-[18px] font-semibold {textPrimary} mb-1">
          Found {discovered.length} repositor{discovered.length === 1 ? 'y' : 'ies'}
        </h2>
        <p class="text-[13px] {textSecondary} mb-4">in ~/projects/</p>

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
              <div class="w-4 h-4 rounded border {checkBg} flex items-center justify-center shrink-0 {selected.has(project.path) ? 'bg-brand-600 border-brand-600' : ''}">
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
          class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
          onclick={handleRegister}
          disabled={selectedCount === 0}
          data-testid="register-button"
        >
          Register {selectedCount} project{selectedCount !== 1 ? 's' : ''}
        </button>
      </div>

    {:else if step === 3}
      <!-- ═══ STEP 3: INDEXING PROGRESS ═══ -->
      <div class="text-center" data-testid="wizard-step-3">
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

    {:else if step === 4}
      <!-- ═══ STEP 4: COMPLETION ═══ -->
      <div class="text-center" data-testid="wizard-step-4">
        <!-- Checkmark circle -->
        <div class="w-14 h-14 rounded-full bg-success-500/10 flex items-center justify-center mx-auto mb-5">
          <svg class="w-7 h-7 text-success-500" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
        </div>

        <h2 class="text-[18px] font-semibold {textPrimary} mb-2">
          {registeredCount} project{registeredCount !== 1 ? 's' : ''} registered
        </h2>
        <p class="text-[13px] {textSecondary} mb-6">You're all set.</p>

        <button
          class="w-full py-2.5 rounded-lg bg-brand-600 text-white text-[14px] font-medium hover:bg-brand-700 transition-colors"
          onclick={onComplete}
          data-testid="go-to-dashboard"
        >Go to dashboard</button>
      </div>
    {/if}

  </div>
</div>
