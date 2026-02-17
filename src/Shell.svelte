<script>
  import { listProjects, getProject } from './lib/ipc.js'

  let dark = $state(false)
  let preview = $state(false)

  /*
   * Layout dimensions
   * - Titlebar: 46px tall, holds logo + tab pill + controls
   * - Sidebar:  252px wide, matches logo area in titlebar
   * - Gap:      6px (gap-1.5) between sidebar and main panel
   * - Frame:    6px (p-1.5) padding around panels inside the dark frame
   */

  // Sidebar status dots — bright for visibility against dark bg
  const dotColor     = { active: 'bg-success-300', recent: 'bg-info-300', stale: 'bg-warning-300', dormant: 'bg-zinc-400' }
  const dotColorDark = { active: 'bg-success-300', recent: 'bg-info-300', stale: 'bg-warning-300', dormant: 'bg-zinc-500' }

  // Main content panel — all dark-mode switching via $derived tokens
  const mainBg         = $derived(dark ? 'bg-zinc-950' : 'bg-white')
  const textPrimary    = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary  = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary   = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textMuted      = $derived(dark ? 'text-zinc-600' : 'text-zinc-500')
  const textBody       = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline        = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const statusColor    = $derived(dark ? 'text-success-400' : 'text-success-600')
  const linkColor      = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')
  const dangerColor    = $derived(dark ? 'text-danger-400/70 hover:text-danger-400' : 'text-danger-600/60 hover:text-danger-600')
  const hoverRow       = $derived(dark ? 'hover:bg-zinc-900' : 'hover:bg-zinc-50')
  const hashColor      = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')
  const timeColor      = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')
  const dashBorder     = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const tabSeparator   = $derived(dark ? 'bg-zinc-700' : 'bg-zinc-200')
  const sessionTint    = $derived(dark ? 'bg-brand-500/[0.03]' : 'bg-brand-50/40')
  const sessionBorder  = $derived(dark ? 'border-brand-400' : 'border-brand-500')
  const tagBg          = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-600')
  const dots           = $derived(dark ? dotColorDark : dotColor)
  const panelBorder    = $derived(dark ? 'border border-zinc-800' : '')

  // Activity state groups for sidebar ordering
  const groups = [
    { key: 'active', label: 'ACTIVE' },
    { key: 'recent', label: 'RECENT' },
    { key: 'stale', label: 'STALE' },
    { key: 'dormant', label: 'DORMANT' },
  ]

  // --- Data state ---
  let projects = $state([])
  let selectedProject = $state(null)
  let sidebarLoading = $state(true)
  let sidebarError = $state(null)
  let detailLoading = $state(false)

  // Load projects on mount
  $effect(() => {
    loadProjects()
  })

  async function loadProjects() {
    sidebarLoading = true
    sidebarError = null
    try {
      projects = await listProjects()
      // Auto-select first project if none selected
      if (!selectedProject && projects.length > 0) {
        await selectProject(projects[0])
      }
    } catch (e) {
      sidebarError = e.message || 'Failed to load projects'
    } finally {
      sidebarLoading = false
    }
  }

  async function selectProject(project) {
    detailLoading = true
    try {
      selectedProject = await getProject(project.id)
    } catch {
      // On error, use the summary data we already have
      selectedProject = project
    } finally {
      detailLoading = false
    }
  }

  // Dev-only: fullscreen preview simulates Tauri desktop experience
  function togglePreview() {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen()
      preview = true
    } else {
      document.exitFullscreen()
      preview = false
    }
  }

  $effect(() => {
    const handler = () => {
      if (!document.fullscreenElement) preview = false
    }
    document.addEventListener('fullscreenchange', handler)
    return () => document.removeEventListener('fullscreenchange', handler)
  })
</script>

<div class="h-full bg-brand-950 flex flex-col font-sans antialiased">

  <!-- ═══ TITLEBAR ═══ -->
  <div class="h-[46px] flex items-end shrink-0 pl-1.5">

    <!-- Logo area (width matches sidebar panel below) -->
    <div class="w-[252px] flex items-center px-4 pb-2 shrink-0">
      <div class="flex items-center gap-2.5">
        <div class="w-[22px] h-[22px] rounded-[5px] bg-brand-500 flex items-center justify-center">
          <span class="text-[10px] font-bold text-white leading-none">t</span>
        </div>
        <span class="text-[13px] font-semibold text-white/90 tracking-[-0.01em]">taurhaus</span>
      </div>
    </div>

    <!-- Tab pill + drag space + controls -->
    <div class="flex-1 flex items-end min-w-0">

      <!-- Tab pill — shares bg with main panel (Manila Folder pattern) -->
      <div class="flex items-center px-4 h-[36px] {mainBg} rounded-t-lg ml-1.5">
        <button class="px-3 py-1 text-[13px] font-medium {textPrimary} border-b-2 border-brand-500">Overview</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button class="px-3 py-1 text-[13px] {textTertiary} hover:text-zinc-500 transition-colors border-b-2 border-transparent">Files</button>
      </div>

      <!-- Right scoop: inverse radius where tab pill meets dark frame -->
      <div class="w-2.5 h-2.5 {mainBg} self-end overflow-hidden shrink-0">
        <div class="w-full h-full bg-brand-950 rounded-bl-full"></div>
      </div>

      <!-- Drag region (data-tauri-drag-region in production) -->
      <div class="flex-1 h-full" data-tauri-drag-region></div>

      <!-- Titlebar controls -->
      <div class="flex items-center gap-0.5 pb-2 pr-3 shrink-0">
        <button
          class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors
            {!dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
          onclick={() => dark = false}
        >Light</button>
        <button
          class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors
            {dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
          onclick={() => dark = true}
        >Dark</button>
        <button
          class="ml-1.5 px-2 py-0.5 rounded text-[11px] font-medium text-brand-400/60 hover:text-brand-400 transition-colors"
          onclick={togglePreview}
        >{preview ? 'Exit' : 'Preview'}</button>
      </div>
    </div>
  </div>

  <!-- ═══ BODY — panels floating inside the dark frame ═══ -->
  <div class="flex-1 flex gap-1.5 p-1.5 pt-0 min-h-0">

    <!-- ═══ SIDEBAR ═══ -->
    <aside class="w-[252px] bg-brand-950 rounded-lg flex flex-col shrink-0 border border-white/[0.06] overflow-hidden">

      <!-- Filter -->
      <div class="px-3 pt-3 pb-1">
        <div class="flex items-center gap-2 px-3 h-[32px] rounded-md bg-white/[0.05] border border-white/[0.07] text-[13px] text-white/25 transition-colors hover:bg-white/[0.07]">
          <svg class="w-[13px] h-[13px]" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"/></svg>
          Filter...
        </div>
      </div>

      <!-- Project list -->
      <div class="flex-1 overflow-y-auto px-1.5 pt-1">
        {#if sidebarLoading}
          <!-- Loading skeleton -->
          <div class="px-3 pt-3 space-y-1" data-testid="sidebar-skeleton">
            {#each Array(5) as _}
              <div class="flex items-center gap-2 h-[34px] px-3">
                <div class="w-[7px] h-[7px] rounded-full bg-white/[0.06] animate-pulse"></div>
                <div class="h-3 rounded bg-white/[0.06] animate-pulse flex-1"></div>
              </div>
            {/each}
          </div>
        {:else if sidebarError}
          <!-- Error state -->
          <div class="px-4 pt-6 text-center" data-testid="sidebar-error">
            <p class="text-[12px] text-white/40">{sidebarError}</p>
            <button
              class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors"
              onclick={loadProjects}
            >Retry</button>
          </div>
        {:else if projects.length === 0}
          <!-- Empty state -->
          <div class="px-4 pt-8 text-center" data-testid="sidebar-empty">
            <svg class="w-10 h-10 text-white/10 mx-auto" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"/></svg>
            <p class="mt-2 text-[12px] text-white/40">No projects yet</p>
            <button class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors">Scan for projects</button>
          </div>
        {:else}
          {#each groups as group}
            {@const items = projects.filter(p => p.activity_state === group.key)}
            {#if items.length > 0}
              <div class="px-3.5 pt-3 pb-1">
                <span class="text-[10px] font-medium uppercase tracking-[0.06em] text-white/20">{group.label}</span>
              </div>
              {#each items as project}
                {@const selected = selectedProject && project.id === selectedProject.id}
                <button
                  class="w-full flex items-center gap-2 px-3 h-[34px] rounded-md text-left transition-all duration-75
                    {selected ? 'bg-white/[0.08]' : 'hover:bg-white/[0.04]'}"
                  onclick={() => selectProject(project)}
                >
                  {#if selected}
                    <span class="w-[2px] h-3.5 bg-brand-400 rounded-full shrink-0 -ml-1 mr-0.5"></span>
                  {/if}
                  <span class="w-[7px] h-[7px] rounded-full shrink-0 {dots[project.activity_state]} shadow-[0_0_4px_rgba(255,255,255,0.15)]"></span>
                  <span class="text-[13px] truncate flex-1 {selected ? 'font-medium text-white' : 'text-white/60'}">{project.name}</span>
                  <span class="text-[10px] font-mono shrink-0 {selected ? 'text-white/30' : 'text-white/15'}">{project.branch || ''}</span>
                  {#if project.is_dirty}
                    <span class="w-[5px] h-[5px] rounded-full bg-warning-400 shrink-0"></span>
                  {/if}
                </button>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>

      <!-- Footer -->
      <div class="h-[44px] flex items-center justify-between px-4 border-t border-white/[0.06]">
        <button class="w-7 h-7 flex items-center justify-center rounded-md text-white/20 hover:text-white/40 hover:bg-white/[0.06] transition-colors" aria-label="Add project">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/></svg>
        </button>
        <button class="w-7 h-7 flex items-center justify-center rounded-md text-white/20 hover:text-white/40 hover:bg-white/[0.06] transition-colors" aria-label="Settings">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 0 1 0 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 0 1 0-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z"/><path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"/></svg>
        </button>
      </div>
    </aside>

    <!-- ═══ MAIN PANEL ═══ -->
    <main class="flex-1 {mainBg} {textBody} rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden {panelBorder}">
      {#if !selectedProject}
        <!-- No project selected -->
        <div class="flex-1 flex items-center justify-center">
          <p class="text-[13px] {textTertiary}">Select a project</p>
        </div>
      {:else}
        <!-- Project header -->
        <div class="px-7 pt-5 pb-4 shrink-0">
          <div class="flex items-baseline gap-3">
            <h1 class="text-[18px] font-semibold {textPrimary} tracking-[-0.02em]">{selectedProject.name}</h1>
            <span class="text-[11px] font-mono {textTertiary}">{selectedProject.branch || ''}</span>
            {#if selectedProject.activity_state}
              <span class="text-[11px] {statusColor} font-medium capitalize">{selectedProject.activity_state}</span>
            {/if}
          </div>
          {#if selectedProject.description}
            <p class="mt-0.5 text-[13px] {textTertiary}">{selectedProject.description}</p>
          {/if}
        </div>

        <!-- Scrollable content -->
        <div class="flex-1 overflow-y-auto">
          <div class="max-w-[700px] px-7 pb-8">

            <!-- Project Info -->
            <section class="py-6 border-b {keyline}">
              <span class="text-[11px] {textTertiary}">Project info</span>
              <div class="mt-2 space-y-1 text-[13px]">
                <div class="flex items-center gap-3">
                  <span class="{textTertiary} w-8">Path</span>
                  <span class="font-mono text-[12px] {textMuted}">{selectedProject.path}</span>
                </div>
                {#if selectedProject.created_at}
                  <div class="flex items-center gap-3">
                    <span class="{textTertiary} w-14">Created</span>
                    <span class="text-[12px] {textMuted}">{new Date(selectedProject.created_at).toLocaleDateString()}</span>
                  </div>
                {/if}
              </div>
              <div class="mt-3 flex gap-3">
                <button class="text-[11px] {textTertiary}">Edit</button>
                <button class="text-[11px] {dangerColor}">Remove</button>
              </div>
            </section>

            <!-- Placeholder for sections built in later phases -->
            <section class="py-6">
              <span class="text-[11px] {textTertiary}">Sessions, commits, and relationships will appear here once those modules are implemented (Phases 5B-5F).</span>
            </section>

          </div>
        </div>
      {/if}
    </main>
  </div>
</div>
