<script>
  import { themeTokens } from '../../themeTokens.js'

  let {
    dark = false,
    activeTab = 'overview',
    settingsOpen = false,
    onSwitchTab = () => {},
    onToggleSearch = () => {},
    onSetDarkMode = () => {},
    onMinimizeWindow = () => {},
    onToggleMaximize = () => {},
    onCloseWindow = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const tabSeparator = $derived(dark ? 'bg-zinc-700' : 'bg-zinc-200')
  const searchShortcutTitle = $derived(
    typeof navigator !== 'undefined' && navigator.platform?.includes('Mac')
      ? 'Search (⌘K)'
      : 'Search (Ctrl+K)'
  )

  function tabButtonClass(tab) {
    return activeTab === tab
      ? `font-medium ${t.textPrimary} border-brand-500`
      : `${t.textTertiary} hover:text-zinc-500 border-transparent`
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
    <div class="shell-main-surface flex items-center px-4 h-[36px] rounded-t-lg ml-1.5">
      {#if settingsOpen}
        <span class="px-3 py-1 text-[13px] font-medium {t.textPrimary}">Settings</span>
      {:else}
        <button
          data-testid="tab-overview"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {tabButtonClass('overview')}"
          onclick={() => onSwitchTab('overview')}
        >Overview</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          data-testid="tab-files"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {tabButtonClass('files')}"
          onclick={() => onSwitchTab('files')}
        >Files</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          data-testid="tab-tasks"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {tabButtonClass('tasks')}"
          onclick={() => onSwitchTab('tasks')}
        >Tasks</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          data-testid="tab-mesh"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {tabButtonClass('mesh')}"
          onclick={() => onSwitchTab('mesh')}
        >Mesh</button>
        <span class="w-px h-3.5 {tabSeparator} mx-1"></span>
        <button
          data-testid="tab-git"
          class="px-3 py-1 text-[13px] transition-colors border-b-2 {tabButtonClass('git')}"
          onclick={() => onSwitchTab('git')}
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
        class="w-7 h-7 flex items-center justify-center rounded text-white/30 hover:text-white/60 hover:bg-white/10 transition-colors mr-1"
        onclick={onToggleSearch}
        title={searchShortcutTitle}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
        </svg>
      </button>
      <button
        data-testid="theme-light"
        class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors {!dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
        onclick={() => onSetDarkMode(false)}
      >Light</button>
      <button
        data-testid="theme-dark"
        class="px-2 py-0.5 rounded text-[11px] font-medium transition-colors {dark ? 'bg-white/10 text-white/90' : 'text-white/30 hover:text-white/60'}"
        onclick={() => onSetDarkMode(true)}
      >Dark</button>

      <div class="flex items-center ml-2">
        <button
          class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-white/10 transition-colors"
          onclick={onMinimizeWindow}
          title="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor"/></svg>
        </button>
        <button
          class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-white/10 transition-colors"
          onclick={onToggleMaximize}
          title="Maximize"
        >
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none"><rect x="0.5" y="0.5" width="8" height="8" rx="1" stroke="currentColor"/></svg>
        </button>
        <button
          class="w-7 h-7 flex items-center justify-center rounded text-white/40 hover:text-white/80 hover:bg-red-500/80 transition-colors"
          onclick={onCloseWindow}
          title="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.2"/></svg>
        </button>
      </div>
    </div>
  </div>
</div>
