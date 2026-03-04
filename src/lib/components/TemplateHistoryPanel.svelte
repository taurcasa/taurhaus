<script>
  import {
    getTemplateDiff,
    getTemplateHistory,
    getTemplateStorageStatus,
    revertTemplateVersion,
  } from '../ipc.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    selectedTemplateId = '',
    selectedTemplateKind = '',
    onReverted = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const frameTone = $derived(
    dark ? 'border-zinc-700/70 bg-zinc-900/60' : 'border-zinc-200 bg-zinc-50/80'
  )
  const cardTone = $derived(
    dark ? 'border-zinc-700/60 bg-zinc-900/40' : 'border-zinc-200 bg-white'
  )
  const neutralButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )

  const PAGE_SIZE = 50
  let loadingStatus = $state(false)
  let loadingHistory = $state(false)
  let loadingDiff = $state(false)
  let loadingMore = $state(false)
  let reverting = $state(false)
  let errorMessage = $state('')
  let infoMessage = $state('')

  let storageStatus = $state({
    mode: 'plain_filesystem',
    repoInitialized: false,
    dirty: false,
    pendingActions: [],
    lastCommit: null,
  })
  let commits = $state([])
  let nextCursor = $state(null)
  let selectedCommitId = $state('')
  let diff = $state({
    commitId: '',
    files: [],
    stats: { filesChanged: 0, insertions: 0, deletions: 0 },
  })
  let selectedDiffPath = $state('')
  let scope = $state('global')

  const selectedCommit = $derived.by(() =>
    commits.find((commit) => commit.commitId === selectedCommitId) ?? null
  )

  const templatePathCandidates = $derived.by(() => {
    const id = String(selectedTemplateId ?? '').trim()
    if (!id) return []
    if (selectedTemplateKind === 'role') return [`roles/${id}.yaml`]
    if (selectedTemplateKind === 'preset') return [`presets/${id}.yaml`]
    return [`roles/${id}.yaml`, `presets/${id}.yaml`]
  })

  function commitTouchesSelectedTemplate(commit) {
    const changedPaths = commit?.changedPaths ?? []
    if (templatePathCandidates.length === 0) return false
    return templatePathCandidates.some((candidate) =>
      changedPaths.some((changed) => changed === candidate)
    )
  }

  const visibleCommits = $derived.by(() => {
    if (scope === 'template') {
      return commits.filter((commit) => commitTouchesSelectedTemplate(commit))
    }
    return commits
  })

  const selectedDiffFile = $derived.by(() =>
    diff.files.find((file) => file.path === selectedDiffPath) ?? null
  )

  function normalizeStatus(value) {
    return {
      mode: value?.mode ?? 'plain_filesystem',
      repoInitialized: Boolean(value?.repoInitialized ?? value?.repo_initialized),
      dirty: Boolean(value?.dirty),
      pendingActions: Array.isArray(value?.pendingActions ?? value?.pending_actions)
        ? value?.pendingActions ?? value?.pending_actions
        : [],
      lastCommit: value?.lastCommit ?? value?.last_commit ?? null,
    }
  }

  function normalizeCommit(value) {
    return {
      commitId: value?.commitId ?? value?.commit_id ?? '',
      shortId: value?.shortId ?? value?.short_id ?? '',
      message: value?.message ?? '',
      author: value?.author ?? 'unknown',
      timestamp: Number(value?.timestamp ?? 0),
      changedPaths: Array.isArray(value?.changedPaths ?? value?.changed_paths)
        ? value?.changedPaths ?? value?.changed_paths
        : [],
    }
  }

  function normalizeDiff(value) {
    return {
      commitId: value?.commitId ?? value?.commit_id ?? '',
      files: Array.isArray(value?.files) ? value.files : [],
      stats: {
        filesChanged: value?.stats?.filesChanged ?? value?.stats?.files_changed ?? 0,
        insertions: value?.stats?.insertions ?? 0,
        deletions: value?.stats?.deletions ?? 0,
      },
    }
  }

  function formatTimestamp(unixSeconds) {
    if (!unixSeconds) return 'Unknown time'
    try {
      return new Date(unixSeconds * 1000).toLocaleString()
    } catch {
      return String(unixSeconds)
    }
  }

  async function loadStatus() {
    loadingStatus = true
    try {
      storageStatus = normalizeStatus(await getTemplateStorageStatus())
    } catch (error) {
      errorMessage = error?.message || 'Failed to load template storage status.'
    } finally {
      loadingStatus = false
    }
  }

  async function loadHistory() {
    loadingHistory = true
    errorMessage = ''
    infoMessage = ''
    try {
      const page = await getTemplateHistory(PAGE_SIZE, null)
      commits = Array.isArray(page?.commits) ? page.commits.map(normalizeCommit) : []
      nextCursor = page?.nextCursor ?? page?.next_cursor ?? null
      selectedCommitId = commits[0]?.commitId ?? ''
    } catch (error) {
      commits = []
      selectedCommitId = ''
      nextCursor = null
      errorMessage = error?.message || 'Failed to load template history.'
    } finally {
      loadingHistory = false
    }
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) return
    loadingMore = true
    try {
      const page = await getTemplateHistory(PAGE_SIZE, nextCursor)
      const additional = Array.isArray(page?.commits) ? page.commits.map(normalizeCommit) : []
      commits = [...commits, ...additional]
      nextCursor = page?.nextCursor ?? page?.next_cursor ?? null
    } catch (error) {
      errorMessage = error?.message || 'Failed to load more history.'
    } finally {
      loadingMore = false
    }
  }

  async function loadDiff(commitId) {
    if (!commitId) {
      diff = {
        commitId: '',
        files: [],
        stats: { filesChanged: 0, insertions: 0, deletions: 0 },
      }
      selectedDiffPath = ''
      return
    }

    loadingDiff = true
    try {
      const result = normalizeDiff(await getTemplateDiff(commitId))
      diff = result
      selectedDiffPath = result.files[0]?.path ?? ''
    } catch (error) {
      diff = {
        commitId,
        files: [],
        stats: { filesChanged: 0, insertions: 0, deletions: 0 },
      }
      selectedDiffPath = ''
      errorMessage = error?.message || 'Failed to load template diff.'
    } finally {
      loadingDiff = false
    }
  }

  async function revertSelected() {
    if (!selectedTemplateId || !selectedCommitId || reverting) return
    reverting = true
    errorMessage = ''
    infoMessage = ''
    try {
      await revertTemplateVersion(selectedTemplateId, selectedCommitId)
      infoMessage = `Reverted ${selectedTemplateId} to ${selectedCommit?.shortId || selectedCommitId.slice(0, 8)}.`
      await Promise.all([loadStatus(), loadHistory()])
      onReverted({ templateId: selectedTemplateId, commitId: selectedCommitId })
    } catch (error) {
      errorMessage = error?.message || 'Failed to revert template.'
    } finally {
      reverting = false
    }
  }

  function scopeButtonTone(value) {
    if (scope === value) {
      return 'border-brand-600 bg-brand-600 text-white'
    }
    return neutralButton
  }

  $effect(() => {
    void Promise.all([loadStatus(), loadHistory()])
  })

  $effect(() => {
    if (scope === 'template' && !selectedTemplateId) {
      scope = 'global'
    }
  })

  $effect(() => {
    const visible = visibleCommits
    if (visible.length === 0) {
      selectedCommitId = ''
      return
    }
    if (!visible.some((commit) => commit.commitId === selectedCommitId)) {
      selectedCommitId = visible[0].commitId
    }
  })

  $effect(() => {
    void loadDiff(selectedCommitId)
  })
</script>

<section class="space-y-3 rounded-md border p-3 {frameTone}" data-testid="template-history-panel">
  <header class="flex items-center justify-between gap-3 border-b pb-2 {t.keyline}">
    <div>
      <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
        Template History
      </h3>
      <p class="text-[11px] {t.textMuted}">
        Commit log, diff, and revert controls for template files.
      </p>
    </div>
    <div class="flex items-center gap-1">
      <span
        class="rounded-full px-2 py-0.5 text-[10px] font-medium {storageStatus.dirty ? 'bg-warning-500/10 text-warning-500 border border-warning-500/40' : 'bg-success-500/10 text-success-500 border border-success-500/40'}"
        data-testid="template-history-dirty-indicator"
      >
        {loadingStatus ? 'Checking status…' : storageStatus.dirty ? 'Dirty' : 'Clean'}
      </span>
      <span class="rounded-full border px-2 py-0.5 text-[10px] {neutralButton}">
        {String(storageStatus.mode ?? 'unknown')}
      </span>
    </div>
  </header>

  <div class="flex flex-wrap items-center gap-1.5">
    <button
      class="rounded-md border px-2 py-1 text-[11px] {scopeButtonTone('global')}"
      onclick={() => {
        scope = 'global'
      }}
      data-testid="template-history-scope-global"
    >
      Global history
    </button>
    <button
      class="rounded-md border px-2 py-1 text-[11px] {scopeButtonTone('template')} disabled:cursor-not-allowed disabled:opacity-60"
      onclick={() => {
        scope = 'template'
      }}
      disabled={!selectedTemplateId}
      data-testid="template-history-scope-template"
    >
      Selected template
    </button>
    {#if selectedTemplateId}
      <span class="text-[11px] {t.textMuted}" data-testid="template-history-selected-template">
        {selectedTemplateId}
      </span>
    {/if}
  </div>

  {#if errorMessage}
    <p class="rounded-md border border-danger-500/40 bg-danger-500/10 px-2 py-1 text-xs text-danger-400" data-testid="template-history-error">
      {errorMessage}
    </p>
  {/if}

  {#if infoMessage}
    <p class="rounded-md border border-success-500/40 bg-success-500/10 px-2 py-1 text-xs text-success-500">
      {infoMessage}
    </p>
  {/if}

  <div class="grid grid-cols-1 gap-3 xl:grid-cols-[280px,1fr]">
    <section class="rounded-md border p-2 {cardTone}">
      <h4 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
        Commits ({visibleCommits.length})
      </h4>

      {#if loadingHistory}
        <p class="mt-2 text-xs {t.textMuted}">Loading history...</p>
      {:else if visibleCommits.length === 0}
        <p class="mt-2 text-xs {t.textMuted}" data-testid="template-history-empty">
          {scope === 'template'
            ? 'No commits affect the selected template yet.'
            : 'No template commits found.'}
        </p>
      {:else}
        <div class="mt-2 space-y-1">
          {#each visibleCommits as commit}
            <button
              class="w-full rounded-md border p-2 text-left transition-colors {selectedCommitId === commit.commitId ? (dark ? 'border-brand-500/40 bg-brand-500/10' : 'border-brand-300 bg-brand-50') : cardTone}"
              onclick={() => {
                selectedCommitId = commit.commitId
              }}
              data-testid={`template-history-commit-${commit.shortId}`}
            >
              <div class="flex items-center justify-between gap-2">
                <span class="font-mono text-[10px] {t.textMuted}">{commit.shortId}</span>
                <span class="text-[10px] {t.textMuted}">{formatTimestamp(commit.timestamp)}</span>
              </div>
              <p class="mt-1 text-[12px] font-medium {t.textPrimary} line-clamp-2">{commit.message}</p>
              <p class="mt-1 text-[10px] {t.textMuted}">{commit.author}</p>
            </button>
          {/each}

          {#if nextCursor}
            <button
              class="mt-1 w-full rounded-md border px-2 py-1 text-[11px] {neutralButton}"
              onclick={loadMore}
              disabled={loadingMore}
              data-testid="template-history-load-more"
            >
              {loadingMore ? 'Loading…' : 'Load more'}
            </button>
          {/if}
        </div>
      {/if}
    </section>

    <section class="rounded-md border p-2 {cardTone}" data-testid="template-history-detail-panel">
      <div class="flex items-start justify-between gap-2">
        <div>
          <h4 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
            Commit Details
          </h4>
          {#if selectedCommit}
            <p class="mt-1 text-[13px] font-medium {t.textPrimary}">
              {selectedCommit.message}
            </p>
            <p class="text-[11px] {t.textMuted}">
              {selectedCommit.shortId} · {selectedCommit.author} · {formatTimestamp(selectedCommit.timestamp)}
            </p>
          {/if}
        </div>
        <button
          class="rounded-md border px-2 py-1 text-[11px] {neutralButton} disabled:cursor-not-allowed disabled:opacity-60"
          onclick={revertSelected}
          disabled={!selectedTemplateId || !selectedCommitId || reverting}
          data-testid="template-history-revert-button"
        >
          {reverting ? 'Reverting…' : 'Revert selected'}
        </button>
      </div>

      {#if !selectedTemplateId}
        <p class="mt-2 text-xs {t.textMuted}" data-testid="template-history-template-hint">
          Select a role or preset in the catalog to enable template-specific revert.
        </p>
      {/if}

      {#if selectedCommit}
        <div class="mt-2">
          <p class="text-[10px] uppercase tracking-wide {t.textMuted}">Files changed</p>
          <div class="mt-1 flex flex-wrap gap-1">
            {#each selectedCommit.changedPaths as path}
              <span class="rounded-full border px-1.5 py-0.5 text-[10px] {neutralButton}">
                {path}
              </span>
            {/each}
          </div>
        </div>
      {/if}

      <div class="mt-3 rounded-md border p-2 {frameTone}" data-testid="template-history-diff-panel">
        <div class="flex items-center justify-between gap-2">
          <p class="text-[10px] uppercase tracking-wide {t.textMuted}">
            Diff
          </p>
          <p class="text-[10px] {t.textMuted}">
            {diff.stats.filesChanged} file(s) · +{diff.stats.insertions} / -{diff.stats.deletions}
          </p>
        </div>

        {#if loadingDiff}
          <p class="mt-2 text-xs {t.textMuted}" data-testid="template-history-diff-loading">Loading diff...</p>
        {:else if diff.files.length === 0}
          <p class="mt-2 text-xs {t.textMuted}" data-testid="template-history-diff-empty">
            Select a commit to inspect template diff.
          </p>
        {:else}
          <div class="mt-2 flex flex-wrap gap-1">
            {#each diff.files as file}
              <button
                class="rounded-full border px-2 py-0.5 text-[10px] {selectedDiffPath === file.path ? 'border-brand-600 bg-brand-600 text-white' : neutralButton}"
                onclick={() => {
                  selectedDiffPath = file.path
                }}
                data-testid={`template-history-diff-file-${file.path.replace(/[^a-zA-Z0-9_-]/g, '-')}`}
              >
                {file.path}
              </button>
            {/each}
          </div>

          {#if selectedDiffFile}
            <div class="mt-2 overflow-hidden rounded border {t.keyline}">
              {#each selectedDiffFile.hunks ?? [] as hunk}
                <div class="border-b {t.keyline}">
                  <div class="px-2 py-1 text-[10px] font-mono {t.textMuted} {dark ? 'bg-zinc-900/80' : 'bg-zinc-100'}">
                    @@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
                  </div>
                  {#each hunk.lines ?? [] as line}
                    <div class="grid grid-cols-[48px,48px,16px,1fr] border-t {t.keyline} text-[11px] font-mono {line.origin === '+' ? (dark ? 'bg-success-500/10 text-success-300' : 'bg-success-50 text-success-700') : line.origin === '-' ? (dark ? 'bg-danger-500/10 text-danger-300' : 'bg-danger-50 text-danger-700') : t.textBody}" data-testid="template-history-diff-line">
                      <span class="px-1 text-right {t.textMuted}">{line.old_lineno ?? ''}</span>
                      <span class="px-1 text-right {t.textMuted}">{line.new_lineno ?? ''}</span>
                      <span class="px-1 text-center">{line.origin ?? ' '}</span>
                      <span class="px-1 whitespace-pre-wrap break-words">{line.content ?? ''}</span>
                    </div>
                  {/each}
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    </section>
  </div>
</section>
