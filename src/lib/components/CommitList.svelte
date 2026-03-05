<script>
  let {
    dark = false,
    t,
    cardTone = '',
    neutralButton = '',
    loadingHistory = false,
    visibleCommits = [],
    selectedCommitId = '',
    nextCursor = null,
    loadingMore = false,
    emptyMessage = '',
    onSelectCommit = () => {},
    onLoadMore = () => {},
    formatTimestamp = () => 'Unknown time',
  } = $props()
</script>

<section class="rounded-md border p-2 {cardTone}">
  <h4 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">
    Commits ({visibleCommits.length})
  </h4>

  {#if loadingHistory}
    <p class="mt-2 text-xs {t.textMuted}">Loading history...</p>
  {:else if visibleCommits.length === 0}
    <p class="mt-2 text-xs {t.textMuted}" data-testid="template-history-empty">
      {emptyMessage}
    </p>
  {:else}
    <div class="mt-2 space-y-1">
      {#each visibleCommits as commit}
        <button
          class="w-full rounded-md border p-2 text-left transition-colors {selectedCommitId === commit.commitId ? (dark ? 'border-brand-500/40 bg-brand-500/10' : 'border-brand-300 bg-brand-50') : cardTone}"
          onclick={() => {
            onSelectCommit(commit.commitId)
          }}
          data-testid={`template-history-commit-${commit.shortId}`}
        >
          <div class="flex items-center justify-between gap-2">
            <span class="font-mono text-xs {t.textMuted}">{commit.shortId}</span>
            <span class="text-xs {t.textMuted}">{formatTimestamp(commit.timestamp)}</span>
          </div>
          <p class="mt-1 text-[12px] font-medium {t.textPrimary} line-clamp-2">{commit.message}</p>
          <p class="mt-1 text-[10px] {t.textMuted}">{commit.author}</p>
        </button>
      {/each}

      {#if nextCursor}
        <button
          class="mt-1 w-full rounded-md border px-2 py-1 text-[11px] {neutralButton}"
          onclick={onLoadMore}
          disabled={loadingMore}
          data-testid="template-history-load-more"
        >
          {loadingMore ? 'Loading…' : 'Load more'}
        </button>
      {/if}
    </div>
  {/if}
</section>
