<script>
  import { themeTokens } from '../../themeTokens.js'

  let {
    dark = false,
    activeTab = 'overview',
    settingsOpen = false,
    accountsOpen = false,
    projectsOpen = false,
    onSwitchTab = () => {},
    onToggleSearch = () => {},
    onSetDarkMode = () => {},
    onMinimizeWindow = () => {},
    onToggleMaximize = () => {},
    onCloseWindow = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const tabSeparator = $derived(dark ? 'bg-zinc-700' : 'bg-zinc-200')
  const focusRing = $derived('focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/40 focus-visible:ring-offset-1 focus-visible:ring-offset-brand-950')
  const searchShortcutTitle = $derived(
    typeof navigator !== 'undefined' && navigator.platform?.includes('Mac')
      ? 'Search (⌘K)'
      : 'Search (Ctrl+K)'
  )
  const TAB_ORDER = ['overview', 'files', 'tasks', 'mesh', 'git']
  const takeoverLabel = $derived(
    settingsOpen ? 'Settings' : accountsOpen ? 'Accounts' : projectsOpen ? 'Projects' : null
  )

  let overviewTabEl = $state(null)
  let filesTabEl = $state(null)
  let tasksTabEl = $state(null)
  let meshTabEl = $state(null)
  let gitTabEl = $state(null)

  function tabButtonClass(tab) {
    return activeTab === tab
      ? `font-medium ${t.textPrimary} border-brand-500`
      : `${t.textTertiary} hover:text-zinc-500 border-transparent`
  }

  function focusTab(tab) {
    const tabs = {
      overview: overviewTabEl,
      files: filesTabEl,
      tasks: tasksTabEl,
      mesh: meshTabEl,
      git: gitTabEl,
    }
    tabs[tab]?.focus()
  }

  function handleTabKeydown(event, tab) {
    const currentIndex = TAB_ORDER.indexOf(tab)
    if (currentIndex === -1) return

    let targetIndex = currentIndex
    if (event.key === 'ArrowRight') {
      targetIndex = (currentIndex + 1) % TAB_ORDER.length
    } else if (event.key === 'ArrowLeft') {
      targetIndex = (currentIndex - 1 + TAB_ORDER.length) % TAB_ORDER.length
    } else if (event.key === 'Home') {
      targetIndex = 0
    } else if (event.key === 'End') {
      targetIndex = TAB_ORDER.length - 1
    } else {
      return
    }

    event.preventDefault()
    const nextTab = TAB_ORDER[targetIndex]
    onSwitchTab(nextTab)
    focusTab(nextTab)
  }
</script>

<div class="h-[46px] flex items-end shrink-0 pl-1.5" data-tauri-drag-region>
  <div class="w-[252px] flex items-center px-4 pb-2 shrink-0" data-tauri-drag-region>
    <div class="flex items-center gap-2.5">
      <img src="/logo-22.png" alt="taurhaus" width="22" height="22" class="block" />
      <span class="text-[13px] font-semibold text-white/90 tracking-[-0.01em]">taurhaus</span>
    </div>
  </div>

  <div class="flex-1 flex items-end min-w-0" data-tauri-drag-region>
    <div class="shell-main-surface flex items-center px-4 h-[36px] rounded-t-lg ml-1.5" role="tablist" aria-label="Project sections">
      {#if takeoverLabel}
        <span class="px-3 py-1 text-[13px] font-medium {t.textPrimary}">{takeoverLabel}</span>
      {:else}
        <button
          bind:this={overviewTabEl}
          id="shell-tab-overview"
          data-testid="tab-overview"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {focusRing} {tabButtonClass('overview')}"
          role="tab"
          tabindex={activeTab === 'overview' ? '0' : '-1'}
          aria-selected={activeTab === 'overview'}
          aria-controls="shell-panel-overview"
          onclick={() => onSwitchTab('overview')}
          onkeydown={(event) => handleTabKeydown(event, 'overview')}
        >Overview</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          bind:this={filesTabEl}
          id="shell-tab-files"
          data-testid="tab-files"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {focusRing} {tabButtonClass('files')}"
          role="tab"
          tabindex={activeTab === 'files' ? '0' : '-1'}
          aria-selected={activeTab === 'files'}
          aria-controls="shell-panel-files"
          onclick={() => onSwitchTab('files')}
          onkeydown={(event) => handleTabKeydown(event, 'files')}
        >Files</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          bind:this={tasksTabEl}
          id="shell-tab-tasks"
          data-testid="tab-tasks"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {focusRing} {tabButtonClass('tasks')}"
          role="tab"
          tabindex={activeTab === 'tasks' ? '0' : '-1'}
          aria-selected={activeTab === 'tasks'}
          aria-controls="shell-panel-tasks"
          onclick={() => onSwitchTab('tasks')}
          onkeydown={(event) => handleTabKeydown(event, 'tasks')}
        >Tasks</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          bind:this={meshTabEl}
          id="shell-tab-mesh"
          data-testid="tab-mesh"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {focusRing} {tabButtonClass('mesh')}"
          role="tab"
          tabindex={activeTab === 'mesh' ? '0' : '-1'}
          aria-selected={activeTab === 'mesh'}
          aria-controls="shell-panel-mesh"
          onclick={() => onSwitchTab('mesh')}
          onkeydown={(event) => handleTabKeydown(event, 'mesh')}
        >Mesh</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          bind:this={gitTabEl}
          id="shell-tab-git"
          data-testid="tab-git"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {focusRing} {tabButtonClass('git')}"
          role="tab"
          tabindex={activeTab === 'git' ? '0' : '-1'}
          aria-selected={activeTab === 'git'}
          aria-controls="shell-panel-git"
          onclick={() => onSwitchTab('git')}
          onkeydown={(event) => handleTabKeydown(event, 'git')}
        >Git</button>
      {/if}
    </div>

    <div class="shell-main-surface w-2.5 h-2.5 self-end overflow-hidden shrink-0">
      <div class="shell-frame-fill w-full h-full rounded-bl-full"></div>
    </div>

    <div class="flex-1 h-full" data-tauri-drag-region></div>

    <div class="flex items-center gap-0.5 pb-2 pr-1 shrink-0">
      <button
        data-testid="search-btn"
        class="w-7 h-7 flex items-center justify-center rounded text-white/30 hover:text-white/60 hover:bg-white/10 transition-colors mr-1 {focusRing}"
        onclick={onToggleSearch}
        title={searchShortcutTitle}
        aria-label="Open search"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
        </svg>
      </button>
      <button
        data-testid="theme-light"
        class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors {focusRing} {!dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
        onclick={() => onSetDarkMode(false)}
      >Light</button>
      <button
        data-testid="theme-dark"
        class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors {focusRing} {dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
        onclick={() => onSetDarkMode(true)}
      >Dark</button>

      <div class="flex items-center ml-2">
        <button
          class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-white/10 transition-colors {focusRing}"
          onclick={onMinimizeWindow}
          title="Minimize"
          aria-label="Minimize window"
        >
          <svg width="10" height="1" viewBox="0 0 10 1" aria-hidden="true"><rect width="10" height="1" fill="currentColor"/></svg>
        </button>
        <button
          class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-white/10 transition-colors {focusRing}"
          onclick={onToggleMaximize}
          title="Maximize"
          aria-label="Maximize window"
        >
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none" aria-hidden="true"><rect x="0.5" y="0.5" width="8" height="8" rx="1" stroke="currentColor"/></svg>
        </button>
        <button
          class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-red-500/80 transition-colors {focusRing}"
          onclick={onCloseWindow}
          title="Close"
          aria-label="Close window"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.2"/></svg>
        </button>
      </div>
    </div>
  </div>
</div>
