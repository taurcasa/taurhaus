<script>
  import { getAllCommits, getCommitFiles, getCommitsInRange, getCommitDiff } from './ipc.js'
  import { themeTokens } from './themeTokens.js'
  import ContextMenu from './ContextMenu.svelte'

  /** @type {{ projectPath: string, projectId: string, dark: boolean, navTarget: object|null, position: object|null, onNavigateToFile?: (path: string) => void, onClearNavTarget?: () => void }} */
  let { projectPath, projectId, dark, navTarget = null, position = $bindable(null), onNavigateToFile, onClearNavTarget } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens
  const timeColor     = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const filterBg      = $derived(dark ? 'bg-brand-900/30 border-brand-500/30' : 'bg-brand-50 border-brand-200')
  const filterText    = $derived(dark ? 'text-brand-300' : 'text-brand-700')
  const groupHeaderBg = $derived(dark ? 'bg-zinc-900/50' : 'bg-zinc-50')
  const groupHeaderText = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const commitMsg     = $derived(dark ? 'text-zinc-200 font-medium' : 'text-zinc-800 font-medium')
  const commitMeta    = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')
  const rowBorder     = $derived(dark ? 'border-white/5' : 'border-zinc-100')

  // Diff view tokens
  const diffAddBg     = $derived(dark ? 'bg-success-500/10' : 'bg-success-50')
  const diffDelBg     = $derived(dark ? 'bg-danger-500/10' : 'bg-danger-50')
  const diffAddText   = $derived(dark ? 'text-success-300' : 'text-success-700')
  const diffDelText   = $derived(dark ? 'text-danger-300' : 'text-danger-700')
  const hunkHeaderBg  = $derived(dark ? 'bg-info-500/10' : 'bg-info-50')
  const hunkHeaderText = $derived(dark ? 'text-info-400' : 'text-info-600')
  const lineNoBg      = $derived(dark ? 'bg-zinc-900/50' : 'bg-zinc-50')
  const lineNoText    = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')
  const filePillBg    = $derived(dark ? 'bg-zinc-800 hover:bg-zinc-700' : 'bg-zinc-100 hover:bg-zinc-200')
  const filePillActive = $derived(dark ? 'bg-brand-900/50 text-brand-300' : 'bg-brand-100 text-brand-700')

  // State
  let commits = $state([])
  let loading = $state(true)
  let selectedHash = $state(null)
  let commitFiles = $state([])
  let filesLoading = $state(false)
  let rangeFilter = $state(null) // { after, before } or null

  // Diff view state
  let selectedFilePath = $state(null)
  let diffHunks = $state([])
  let diffLoading = $state(false)

  // Infinite scroll state
  const PAGE_SIZE = 50
  let loadingMore = $state(false)
  let hasMore = $state(true)
  let currentOffset = $state(0)
  let sentinelEl = $state(null)

  // Sync position outward for Shell's per-project position memory
  $effect(() => {
    position = { selectedHash, rangeFilter }
  })

  // Load commits on mount or projectId change
  $effect(() => {
    if (!projectId) return
    let cancelled = false

    async function load() {
      loading = true
      selectedHash = null
      commitFiles = []
      rangeFilter = null
      currentOffset = 0
      hasMore = true
      try {
        const result = await getAllCommits(projectId, PAGE_SIZE, 0)
        if (!cancelled) {
          commits = result
          currentOffset = result.length
          hasMore = result.length >= PAGE_SIZE
          loading = false
        }
      } catch {
        if (!cancelled) {
          commits = []
          hasMore = false
          loading = false
        }
      }
    }

    load()
    return () => { cancelled = true }
  })

  // Handle cross-tab navigation target
  $effect(() => {
    if (!navTarget) return

    if (navTarget.type === 'commit') {
      // Select the specific commit
      selectCommit(navTarget.hash)
      onClearNavTarget?.()
    } else if (navTarget.type === 'range') {
      // Load commits in the time range
      loadRange(navTarget.after, navTarget.before)
      onClearNavTarget?.()
    }
  })

  async function loadRange(after, before) {
    loading = true
    selectedHash = null
    commitFiles = []
    rangeFilter = { after, before }
    try {
      const result = await getCommitsInRange(projectPath, after, before)
      commits = result.commits || []
    } catch {
      commits = []
    } finally {
      loading = false
    }
  }

  function clearFilter() {
    rangeFilter = null
    loading = true
    selectedHash = null
    commitFiles = []
    currentOffset = 0
    hasMore = true
    getAllCommits(projectId, PAGE_SIZE, 0).then(result => {
      commits = result
      currentOffset = result.length
      hasMore = result.length >= PAGE_SIZE
      loading = false
    }).catch(() => {
      commits = []
      hasMore = false
      loading = false
    })
  }

  async function selectCommit(hash) {
    selectedHash = hash
    selectedFilePath = null
    diffHunks = []
    filesLoading = true
    commitFiles = []
    try {
      commitFiles = await getCommitFiles(projectPath, hash)
    } catch {
      commitFiles = []
    } finally {
      filesLoading = false
    }
  }

  async function handleFileClick(path) {
    if (selectedFilePath === path) {
      // Toggle off
      selectedFilePath = null
      diffHunks = []
      return
    }
    selectedFilePath = path
    diffLoading = true
    diffHunks = []
    try {
      diffHunks = await getCommitDiff(projectPath, selectedHash, path)
    } catch {
      diffHunks = []
    } finally {
      diffLoading = false
    }
  }

  function handleOpenFile(path) {
    // Find the first changed line number from diff for line-jump
    const firstAddedLine = diffHunks
      .flatMap(h => h.lines)
      .find(l => l.origin === '+' && l.new_lineno != null)
    onNavigateToFile?.(path, firstAddedLine?.new_lineno)
  }

  function backToFiles() {
    selectedFilePath = null
    diffHunks = []
  }

  /** Get the basename of a file path. */
  function basename(path) {
    return path.split('/').pop() || path
  }

  async function loadMore() {
    if (loadingMore || !hasMore || rangeFilter) return
    loadingMore = true
    try {
      const batch = await getAllCommits(projectId, PAGE_SIZE, currentOffset)
      commits = [...commits, ...batch]
      currentOffset += batch.length
      if (batch.length < PAGE_SIZE) hasMore = false
    } catch {
      hasMore = false
    } finally {
      loadingMore = false
    }
  }

  // IntersectionObserver for infinite scroll sentinel
  $effect(() => {
    if (!sentinelEl) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) loadMore()
      },
      { rootMargin: '100px' }
    )
    observer.observe(sentinelEl)
    return () => observer.disconnect()
  })

  /** Format a range date for display. */
  function formatRangeDate(iso) {
    if (!iso) return ''
    try {
      return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
    } catch {
      return iso
    }
  }

  /** Status display config per file change type. */
  const STATUS_DISPLAY = {
    added:    { icon: '+', color: 'text-success-400' },
    modified: { icon: '~', color: 'text-warning-400' },
    deleted:  { icon: '-', color: 'text-danger-400' },
    renamed:  { icon: '>', color: 'text-info-400' },
  }

  /** Get the selected commit object. */
  const selectedCommit = $derived(commits.find(c => c.hash === selectedHash))

  /** Group commits by date for visual grouping headers. */
  function getDateLabel(ts) {
    const d = new Date(ts * 1000)
    const now = new Date()
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
    const commitDay = new Date(d.getFullYear(), d.getMonth(), d.getDate())
    const diffDays = Math.round((today - commitDay) / 86400000)
    if (diffDays === 0) return 'Today'
    if (diffDays === 1) return 'Yesterday'
    if (diffDays < 7) return d.toLocaleDateString('en-US', { weekday: 'long' })
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }

  /** Get author initial for avatar circle. */
  function authorInitial(name) {
    return (name || '?')[0].toUpperCase()
  }

  /** Deterministic hue from author string (0-360). Simple DJB2 hash. */
  function authorHue(author) {
    let h = 5381
    for (let i = 0; i < (author || '').length; i++) {
      h = ((h << 5) + h + author.charCodeAt(i)) | 0
    }
    return Math.abs(h) % 360
  }

  /** HSL avatar colors tuned for dark/light backgrounds. */
  function avatarStyle(author) {
    const hue = authorHue(author)
    if (dark) {
      return `background: hsl(${hue} 50% 20%); color: hsl(${hue} 70% 75%);`
    }
    return `background: hsl(${hue} 55% 90%); color: hsl(${hue} 60% 35%);`
  }

  // --- Commit context menu ---
  let commitCtxMenu = $state(null) // { x, y, hash, message }

  const CTX_ICON_COPY = '<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75"/></svg>'

  function openCommitContextMenu(e, commit) {
    e.preventDefault()
    commitCtxMenu = { x: e.clientX, y: e.clientY, hash: commit.hash, message: commit.message }
  }

  function closeCommitContextMenu() {
    commitCtxMenu = null
  }

  const commitCtxMenuItems = $derived(commitCtxMenu ? [
    { label: 'Copy Commit Hash', action: () => { navigator.clipboard.writeText(commitCtxMenu.hash); closeCommitContextMenu() }, icon: CTX_ICON_COPY },
    { label: 'Copy Commit Message', action: () => { navigator.clipboard.writeText(commitCtxMenu.message); closeCommitContextMenu() }, icon: CTX_ICON_COPY },
  ] : [])
</script>

<div class="flex-1 flex min-h-0" data-testid="git-tab">

  <!-- Commit detail (left panel, wide) -->
  <div class="flex-1 flex flex-col min-w-0 content-enter">
    {#if !selectedHash}
      <div class="flex-1 flex items-center justify-center">
        <p class="text-[13px] {t.textMuted}">Select a commit to view details</p>
      </div>
    {:else if selectedCommit}
      <!-- Commit header -->
      <div class="px-6 pt-5 pb-4 border-b {t.keyline} shrink-0">
        <div class="flex items-baseline gap-3">
          <span class="font-mono text-[13px] {t.hashColor}">{selectedCommit.hash}</span>
          <span class="text-[11px] {t.textTertiary}">{selectedCommit.author}</span>
          <span class="text-[11px] {timeColor}">{selectedCommit.date}</span>
        </div>
        <p class="mt-2 text-[14px] {t.textPrimary}">{selectedCommit.message}</p>
        {#if selectedCommit.body}
          <p class="mt-1.5 text-[13px] {t.textBody} whitespace-pre-wrap">{selectedCommit.body}</p>
        {/if}
      </div>

      <!-- Content area: file list or diff view -->
      <div class="flex-1 overflow-y-auto">
        {#if selectedFilePath}
          <!-- Diff view -->
          <div class="flex flex-col min-h-full" data-testid="diff-view">
            <!-- Sticky navigation header -->
            <div class="sticky top-0 z-10 {dark ? 'bg-zinc-950' : 'bg-white'}">
              <!-- Breadcrumb bar -->
              <div class="px-4 py-2 border-b {t.keyline} flex items-center gap-2">
                <button
                  class="text-[11px] {t.linkColor} transition-colors flex items-center gap-1"
                  onclick={backToFiles}
                  data-testid="back-to-files"
                >
                  <span class="text-[13px]">&larr;</span>
                  Files ({commitFiles.length})
                </button>
                <span class="text-[10px] {t.textTertiary}">/</span>
                <span class="text-[12px] font-mono {t.textBody} truncate">{selectedFilePath}</span>
                <div class="flex-1"></div>
                <button
                  class="text-[11px] {t.linkColor} transition-colors"
                  onclick={() => handleOpenFile(selectedFilePath)}
                  data-testid="open-file-btn"
                >Open file &rarr;</button>
              </div>

              <!-- File pills (compact switching) -->
              {#if commitFiles.length > 1}
                <div class="px-4 py-2 border-b {t.keyline} flex flex-wrap gap-1">
                  {#each commitFiles as file}
                    {@const isActive = selectedFilePath === file.path}
                    {@const display = STATUS_DISPLAY[file.status] || STATUS_DISPLAY.modified}
                    <button
                      class="text-[10px] px-2 py-0.5 rounded-full transition-colors font-mono
                        {isActive ? filePillActive : filePillBg}"
                      onclick={() => handleFileClick(file.path)}
                      data-testid="file-pill"
                    >
                      <span class="{display.color} mr-0.5">{display.icon}</span>{basename(file.path)}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>

            <!-- Diff content -->
            <div class="px-4 py-3">
              {#if diffLoading}
                <div class="space-y-1" data-testid="diff-loading">
                  {#each Array(8) as _}
                    <div class="flex items-center h-[20px] gap-1">
                      <div class="w-8 h-3 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                      <div class="w-8 h-3 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                      <div class="h-3 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse"></div>
                    </div>
                  {/each}
                </div>
              {:else if diffHunks.length === 0}
                <p class="text-[12px] {t.textMuted}" data-testid="diff-empty">Binary file or no changes</p>
              {:else}
                <div class="rounded border {t.keyline} overflow-hidden" data-testid="diff-content">
                  {#each diffHunks as hunk, hunkIdx}
                    <!-- Hunk header -->
                    <div class="px-3 py-1 {hunkHeaderBg} {hunkHeaderText} text-[11px] font-mono border-b {t.keyline}">
                      @@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
                    </div>
                    <!-- Diff lines -->
                    {#each hunk.lines as line}
                      {@const bgClass = line.origin === '+' ? diffAddBg : line.origin === '-' ? diffDelBg : ''}
                      {@const textClass = line.origin === '+' ? diffAddText : line.origin === '-' ? diffDelText : t.textBody}
                      <div class="flex font-mono text-[12px] leading-[20px] {bgClass}" data-testid="diff-line">
                        <span class="w-[36px] shrink-0 text-right pr-1 select-none {lineNoText} {lineNoBg} border-r {t.keyline}">{line.old_lineno ?? ''}</span>
                        <span class="w-[36px] shrink-0 text-right pr-1 select-none {lineNoText} {lineNoBg} border-r {t.keyline}">{line.new_lineno ?? ''}</span>
                        <span class="w-4 shrink-0 text-center select-none {textClass}">{line.origin}</span>
                        <span class="flex-1 px-1 whitespace-pre {textClass}">{line.content}</span>
                      </div>
                    {/each}
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        {:else}
          <!-- File list view -->
          <div class="px-6 py-4">
            <div class="flex items-center justify-between mb-3">
              <span class="text-[11px] font-medium uppercase tracking-[0.06em] {t.textTertiary}">Files changed</span>
              {#if !filesLoading}
                <span class="text-[11px] {t.textTertiary}">{commitFiles.length} file{commitFiles.length !== 1 ? 's' : ''}</span>
              {/if}
            </div>

            {#if filesLoading}
              <div class="space-y-1" data-testid="files-loading">
                {#each Array(5) as _}
                  <div class="flex items-center h-[28px] gap-2">
                    <div class="w-3 h-3 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                    <div class="h-2.5 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse"></div>
                  </div>
                {/each}
              </div>
            {:else if commitFiles.length === 0}
              <p class="text-[12px] {t.textMuted}">No files changed</p>
            {:else}
              <div class="space-y-0.5">
                {#each commitFiles as file}
                  {@const display = STATUS_DISPLAY[file.status] || STATUS_DISPLAY.modified}
                  <button
                    class="w-full text-left flex items-center gap-2 h-[28px] px-2 rounded transition-colors {t.fileBg}"
                    onclick={() => handleFileClick(file.path)}
                    data-testid="commit-file"
                  >
                    <span class="w-3 text-center font-mono text-[11px] font-bold {display.color} shrink-0">{display.icon}</span>
                    <span class="text-[12px] font-mono {t.textBody} truncate">{file.path}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Commit list (right panel) -->
  <div class="w-[320px] shrink-0 {t.listBg} border-l {t.keyline} flex flex-col overflow-hidden">

    <!-- Range filter indicator -->
    {#if rangeFilter}
      <div class="px-3 py-2 border-b {t.keyline} {filterBg}" data-testid="range-filter">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-medium {filterText}">Filtered to session</span>
          <button
            class="text-[10px] {t.linkColor} transition-colors"
            onclick={clearFilter}
          >Clear</button>
        </div>
        <div class="text-[10px] {t.textTertiary} mt-0.5">
          {formatRangeDate(rangeFilter.after)} — {formatRangeDate(rangeFilter.before)}
        </div>
      </div>
    {/if}

    <!-- Commit list -->
    <div class="flex-1 overflow-y-auto pt-1">
      {#if loading}
        <div class="px-3 space-y-0.5" data-testid="git-loading">
          {#each Array(8) as _}
            <div class="flex flex-col justify-center h-[46px] gap-1.5 px-2">
              <div class="flex items-center gap-2">
                <div class="h-2 w-8 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                <div class="h-2.5 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse"></div>
              </div>
              <div class="flex items-center gap-2">
                <div class="h-1.5 w-14 rounded {dark ? 'bg-zinc-800/40' : 'bg-zinc-100'} animate-pulse"></div>
                <div class="h-1.5 w-12 rounded {dark ? 'bg-zinc-800/30' : 'bg-zinc-100/80'} animate-pulse"></div>
              </div>
            </div>
          {/each}
        </div>
      {:else if commits.length === 0}
        <div class="flex-1 flex items-center justify-center px-4" data-testid="git-empty">
          <div class="text-center max-w-xs">
            <svg class="w-12 h-12 {t.textMuted} mx-auto opacity-30" fill="none" viewBox="0 0 24 24" stroke-width="1" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p class="mt-4 text-[15px] font-medium {t.textMuted}">
              {rangeFilter ? 'No commits in this range' : 'No commits found'}
            </p>
            <p class="mt-2 text-[13px] leading-relaxed {t.textTertiary}">
              {rangeFilter ? 'Try adjusting the date range filter.' : 'This project has no git history yet.'}
            </p>
          </div>
        </div>
      {:else}
        {#each commits as commit, idx (commit.hash)}
          {@const isSelected = selectedHash === commit.hash}
          {@const prevCommit = idx > 0 ? commits[idx - 1] : null}
          {@const currentLabel = commit.timestamp ? getDateLabel(commit.timestamp) : ''}
          {@const prevLabel = prevCommit?.timestamp ? getDateLabel(prevCommit.timestamp) : ''}
          {@const showHeader = currentLabel && currentLabel !== prevLabel}

          <!-- Date group header -->
          {#if showHeader}
            <div class="sticky top-0 z-10 px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] {groupHeaderBg} {groupHeaderText} {idx > 0 ? 'mt-1' : ''}">
              {currentLabel}
            </div>
          {/if}

          <!-- Commit row -->
          <button
            class="w-full flex items-center h-[46px] text-left px-3 gap-2.5 transition-colors border-b {rowBorder}
              {isSelected ? t.listSelected : t.listHover}"
            onclick={() => selectCommit(commit.hash)}
            oncontextmenu={(e) => openCommitContextMenu(e, commit)}
            data-testid="commit-row"
            aria-current={isSelected ? 'true' : undefined}
          >
            <!-- Author initial circle — hue derived from author string -->
            <div
              class="w-5 h-5 rounded-full flex items-center justify-center shrink-0 text-[10px] font-medium"
              style={isSelected ? '' : avatarStyle(commit.author)}
            >
              {authorInitial(commit.author)}
            </div>

            <!-- Message + metadata -->
            <div class="flex-1 flex flex-col justify-center min-w-0">
              <span class="text-[13px] truncate {isSelected ? '' : commitMsg}">{commit.message}</span>
              <div class="flex items-center gap-1.5 mt-0.5">
                <span class="font-mono text-[10px] {isSelected ? 'opacity-70' : commitMeta}">{commit.hash}</span>
                <span class="{isSelected ? 'opacity-40' : commitMeta} text-[8px]">&#183;</span>
                <span class="text-[10px] truncate {isSelected ? 'opacity-60' : commitMeta}">{commit.author}</span>
              </div>
            </div>

            <!-- Timestamp (right-aligned) -->
            <span class="text-[11px] shrink-0 {isSelected ? '' : timeColor}">{commit.date}</span>
          </button>
        {/each}
        {#if hasMore && !rangeFilter}
          <div bind:this={sentinelEl} class="h-8 flex items-center justify-center" data-testid="scroll-sentinel">
            {#if loadingMore}
              <span class="text-[10px] {t.textTertiary}">Loading...</span>
            {/if}
          </div>
        {/if}
      {/if}
    </div>
  </div>
</div>

{#if commitCtxMenu}
  <ContextMenu items={commitCtxMenuItems} x={commitCtxMenu.x} y={commitCtxMenu.y} {dark} onClose={closeCommitContextMenu} />
{/if}
