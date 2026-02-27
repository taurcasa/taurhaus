<script>
  import { onMount } from 'svelte'
  import { listDirectory, getSystemRoots } from './ipc.js'
  import { themeTokens } from './themeTokens.js'

  let {
    dark = false,
    onSelect = () => {},
    selectedPath = '',
    initialPath = '~',
    maxHeight = '180px',
  } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const hoverRow = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-50')
  const iconColor = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

  // ═══ DIRECTORY TREE STATE ═══

  let treeChildren = $state({})   // { path: [{name, path, isExpandable}] }
  let treeExpanded = $state(new Set())
  let treeLoading = $state(new Set())
  let treeRoot = $state(initialPath)
  let showDrives = $state(false)
  let systemRoots = $state([])

  // ═══ TREE FUNCTIONS ═══

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

  /** Check if path is a filesystem root (/ on Linux, C:\ on Windows, or WSL root) */
  function isSystemRoot(path) {
    if (path === '/') return true
    // Windows drive root: C:\ or C:/
    if (/^[A-Z]:[/\\]?$/.test(path)) return true
    // WSL root: \\wsl.localhost\Distro or \\wsl$\Distro (no further segments)
    if (/^\\\\wsl[.$]/.test(path)) {
      const segments = path.replace(/^\\\\/, '').split(/[/\\]/).filter(Boolean)
      return segments.length <= 2
    }
    return false
  }

  async function navigateUp() {
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
      const normalized = treeRoot.replace(/\\/g, '/')
      const parts = normalized.split('/')
      if (parts.length <= 2) {
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

  // Init once on mount (not $effect — initTree writes reactive state it also reads,
  // which would cause an infinite re-trigger loop)
  onMount(() => {
    initTree()
  })
</script>

<!-- Directory tree browser -->
<div class="border {t.keyline} rounded-lg overflow-hidden overflow-y-auto" style="max-height: {maxHeight}" data-testid="directory-tree">
  {#snippet treeNode(entries, depth)}
    {#each entries as entry}
      <div>
        <button
          class="w-full flex items-center gap-1.5 px-2 h-[30px] text-left text-[13px] transition-colors
            {selectedPath === entry.path ? (dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700') : hoverRow + ' ' + t.textPrimary}"
          style="padding-left: {8 + depth * 16}px"
          onclick={() => onSelect(entry.path)}
          ondblclick={() => entry.isExpandable && toggleTreeDir(entry.path)}
        >
          <!-- Expand/collapse chevron -->
          {#if entry.isExpandable}
            <span
              class="w-4 h-4 flex items-center justify-center shrink-0 cursor-pointer rounded hover:bg-white/10 {iconColor}"
              role="button"
              tabindex="0"
              aria-label="Toggle folder"
              onclick={(e) => { e.stopPropagation(); toggleTreeDir(entry.path) }}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); toggleTreeDir(entry.path) } }}
            >
              <svg class="w-3 h-3 transition-transform {treeExpanded.has(entry.path) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
            </span>
          {:else}
            <span class="w-4 shrink-0"></span>
          {/if}
          <!-- Folder icon -->
          <svg class="w-3.5 h-3.5 shrink-0 {iconColor}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z"/></svg>
          <span class="truncate font-mono">{entry.name}</span>
        </button>
        <!-- Children (if expanded) -->
        {#if treeExpanded.has(entry.path)}
          {#if treeLoading.has(entry.path)}
            <div class="flex items-center gap-2 h-[28px]" style="padding-left: {24 + depth * 16}px">
              <div class="w-3 h-3 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
              <span class="text-[11px] {t.textTertiary}">Loading...</span>
            </div>
          {:else if treeChildren[entry.path]?.length > 0}
            {@render treeNode(treeChildren[entry.path], depth + 1)}
          {:else if treeChildren[entry.path]}
            <div class="h-[28px] flex items-center" style="padding-left: {24 + depth * 16}px">
              <span class="text-[11px] {t.textTertiary}">Empty</span>
            </div>
          {/if}
        {/if}
      </div>
    {/each}
  {/snippet}

  {#if showDrives}
    <!-- Drive/root selector -->
    {#each systemRoots as root}
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-left text-[13px] font-mono transition-colors {hoverRow} {t.textPrimary}"
        onclick={() => selectDrive(root.path)}
      >
        <svg class="w-4 h-4 shrink-0 {t.textTertiary}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M21.75 17.25v-.228a4.5 4.5 0 0 0-.12-1.03l-2.268-9.64a3.375 3.375 0 0 0-3.285-2.602H7.923a3.375 3.375 0 0 0-3.285 2.602l-2.268 9.64a4.5 4.5 0 0 0-.12 1.03v.228m19.5 0a3 3 0 0 1-3 3H5.25a3 3 0 0 1-3-3m19.5 0a3 3 0 0 0-3-3H5.25a3 3 0 0 0-3 3m16.5 0h.008v.008h-.008v-.008Zm-3 0h.008v.008h-.008v-.008Z"/></svg>
        <span>{root.name}</span>
      </button>
    {/each}
    {#if systemRoots.length === 0}
      <div class="text-[12px] {t.textTertiary} py-3 px-3">Loading drives...</div>
    {/if}
  {:else}
    <!-- Navigate up -->
    <button
      class="w-full flex items-center gap-1.5 px-2 h-[28px] text-left text-[12px] transition-colors {hoverRow} {t.textTertiary}"
      onclick={navigateUp}
      data-testid="tree-navigate-up"
    >
      <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 15.75 7.5-7.5 7.5 7.5"/></svg>
      <span class="font-mono">..</span>
    </button>
    <!-- Root directory -->
    <button
      class="w-full flex items-center gap-1.5 px-2 h-[30px] text-left text-[13px] transition-colors font-medium
        {selectedPath === treeRoot ? (dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700') : hoverRow + ' ' + t.textPrimary}"
      onclick={() => { onSelect(treeRoot); toggleTreeDir(treeRoot) }}
    >
      <span class="w-4 h-4 flex items-center justify-center shrink-0 rounded hover:bg-white/10 {iconColor}">
        <svg class="w-3 h-3 transition-transform {treeExpanded.has(treeRoot) ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m8.25 4.5 7.5 7.5-7.5 7.5"/></svg>
      </span>
      <svg class="w-3.5 h-3.5 shrink-0 {iconColor}" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z"/></svg>
      <span class="truncate font-mono">{treeRoot}</span>
    </button>
    {#if treeExpanded.has(treeRoot)}
      {#if treeLoading.has(treeRoot)}
        <div class="flex items-center gap-2 h-[28px] pl-6">
          <div class="w-3 h-3 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
          <span class="text-[11px] {t.textTertiary}">Loading...</span>
        </div>
      {:else if treeChildren[treeRoot]?.length > 0}
        {@render treeNode(treeChildren[treeRoot], 1)}
      {:else if treeChildren[treeRoot]}
        <div class="h-[28px] flex items-center pl-6">
          <span class="text-[11px] {t.textTertiary}">No subdirectories found</span>
        </div>
      {/if}
    {/if}
  {/if}
</div>
