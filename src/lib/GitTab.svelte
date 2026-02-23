<script>
  import { getAllCommits, getCommitFiles, getCommitsInRange } from './ipc.js'

  /** @type {{ projectPath: string, projectId: string, dark: boolean, gitNavTarget: object|null, onNavigateToFile?: (path: string) => void, onClearNavTarget?: () => void }} */
  let { projectPath, projectId, dark, gitNavTarget = null, onNavigateToFile, onClearNavTarget } = $props()

  // Dark mode tokens (same pattern as Shell.svelte)
  const textPrimary   = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textSecondary = $derived(dark ? 'text-zinc-300' : 'text-zinc-600')
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const textMuted     = $derived(dark ? 'text-zinc-600' : 'text-zinc-500')
  const textBody      = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const keyline       = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const listBg        = $derived(dark ? 'bg-zinc-900' : 'bg-zinc-50')
  const listHover     = $derived(dark ? 'hover:bg-zinc-800' : 'hover:bg-zinc-100')
  const listSelected  = $derived(dark ? 'bg-brand-900/40 text-brand-300' : 'bg-brand-100/80 text-brand-700')
  const hashColor     = $derived(dark ? 'text-brand-400' : 'text-brand-600')
  const timeColor     = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')
  const sectionBg     = $derived(dark ? 'bg-zinc-900/30' : 'bg-zinc-50/50')
  const fileBg        = $derived(dark ? 'hover:bg-zinc-800/50' : 'hover:bg-zinc-100/80')
  const filterBg      = $derived(dark ? 'bg-brand-900/30 border-brand-500/30' : 'bg-brand-50 border-brand-200')
  const filterText    = $derived(dark ? 'text-brand-300' : 'text-brand-700')
  const linkColor     = $derived(dark ? 'text-brand-400 hover:text-brand-300' : 'text-brand-600 hover:text-brand-700')

  // State
  let commits = $state([])
  let loading = $state(true)
  let selectedHash = $state(null)
  let commitFiles = $state([])
  let filesLoading = $state(false)
  let rangeFilter = $state(null) // { after, before } or null

  // Load commits on mount or projectId change
  $effect(() => {
    if (!projectId) return
    let cancelled = false

    async function load() {
      loading = true
      selectedHash = null
      commitFiles = []
      rangeFilter = null
      try {
        const result = await getAllCommits(projectId, 50)
        if (!cancelled) {
          commits = result
          loading = false
        }
      } catch {
        if (!cancelled) {
          commits = []
          loading = false
        }
      }
    }

    load()
    return () => { cancelled = true }
  })

  // Handle cross-tab navigation target
  $effect(() => {
    if (!gitNavTarget) return

    if (gitNavTarget.type === 'commit') {
      // Select the specific commit
      selectCommit(gitNavTarget.hash)
      onClearNavTarget?.()
    } else if (gitNavTarget.type === 'range') {
      // Load commits in the time range
      loadRange(gitNavTarget.after, gitNavTarget.before)
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
    getAllCommits(projectId, 50).then(result => {
      commits = result
      loading = false
    }).catch(() => {
      commits = []
      loading = false
    })
  }

  async function selectCommit(hash) {
    selectedHash = hash
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

  function handleFileClick(path) {
    onNavigateToFile?.(path)
  }

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
</script>

<div class="flex-1 flex min-h-0" data-testid="git-tab">

  <!-- Commit list (left panel, 250px) -->
  <div class="w-[250px] shrink-0 {listBg} border-r {keyline} flex flex-col overflow-hidden">

    <!-- Range filter indicator -->
    {#if rangeFilter}
      <div class="px-3 py-2 border-b {keyline} {filterBg}" data-testid="range-filter">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-medium {filterText}">Filtered to session</span>
          <button
            class="text-[10px] {linkColor} transition-colors"
            onclick={clearFilter}
          >Clear</button>
        </div>
        <div class="text-[10px] {textTertiary} mt-0.5">
          {formatRangeDate(rangeFilter.after)} — {formatRangeDate(rangeFilter.before)}
        </div>
      </div>
    {/if}

    <!-- Commit list -->
    <div class="flex-1 overflow-y-auto pt-1">
      {#if loading}
        <div class="px-3 space-y-1" data-testid="git-loading">
          {#each Array(8) as _}
            <div class="flex items-center h-[30px] gap-2 px-2">
              <div class="h-2 w-14 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
              <div class="h-2 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse"></div>
            </div>
          {/each}
        </div>
      {:else if commits.length === 0}
        <div class="px-4 pt-8 text-center" data-testid="git-empty">
          <svg class="w-8 h-8 {textMuted} mx-auto opacity-40" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p class="mt-2 text-[12px] {textMuted}">
            {rangeFilter ? 'No commits in this range' : 'No commits found'}
          </p>
        </div>
      {:else}
        {#each commits as commit (commit.hash)}
          {@const isSelected = selectedHash === commit.hash}
          <button
            class="w-full flex items-center gap-2 h-[30px] text-left text-[13px] rounded mx-1 transition-colors
              {isSelected ? listSelected : `${dark ? 'text-zinc-400' : 'text-zinc-600'} ${listHover}`}"
            style="width: calc(100% - 8px)"
            onclick={() => selectCommit(commit.hash)}
            data-testid="commit-row"
            aria-current={isSelected ? 'true' : undefined}
          >
            <span class="font-mono text-[11px] {isSelected ? '' : hashColor} w-[58px] shrink-0 pl-2">{commit.hash}</span>
            <span class="truncate flex-1 {isSelected ? '' : textBody}">{commit.message}</span>
            <span class="text-[10px] {timeColor} shrink-0 pr-2">{commit.date}</span>
          </button>
        {/each}
      {/if}
    </div>
  </div>

  <!-- Commit detail (right panel) -->
  <div class="flex-1 flex flex-col min-w-0 content-enter">
    {#if !selectedHash}
      <div class="flex-1 flex items-center justify-center">
        <p class="text-[13px] {textMuted}">Select a commit to view details</p>
      </div>
    {:else if selectedCommit}
      <!-- Commit header -->
      <div class="px-6 pt-5 pb-4 border-b {keyline} shrink-0">
        <div class="flex items-baseline gap-3">
          <span class="font-mono text-[13px] {hashColor}">{selectedCommit.hash}</span>
          <span class="text-[11px] {textTertiary}">{selectedCommit.author}</span>
          <span class="text-[11px] {timeColor}">{selectedCommit.date}</span>
        </div>
        <p class="mt-2 text-[14px] {textPrimary}">{selectedCommit.message}</p>
      </div>

      <!-- Files changed -->
      <div class="flex-1 overflow-y-auto">
        <div class="px-6 py-4">
          <div class="flex items-center justify-between mb-3">
            <span class="text-[11px] font-medium uppercase tracking-[0.06em] {textTertiary}">Files changed</span>
            {#if !filesLoading}
              <span class="text-[11px] {textTertiary}">{commitFiles.length} file{commitFiles.length !== 1 ? 's' : ''}</span>
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
            <p class="text-[12px] {textMuted}">No files changed</p>
          {:else}
            <div class="space-y-0.5">
              {#each commitFiles as file}
                {@const display = STATUS_DISPLAY[file.status] || STATUS_DISPLAY.modified}
                <button
                  class="w-full text-left flex items-center gap-2 h-[28px] px-2 rounded transition-colors {fileBg}"
                  onclick={() => handleFileClick(file.path)}
                  data-testid="commit-file"
                >
                  <span class="w-3 text-center font-mono text-[11px] font-bold {display.color} shrink-0">{display.icon}</span>
                  <span class="text-[12px] font-mono {textBody} truncate">{file.path}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
