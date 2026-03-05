<script>
  import { themeTokens } from '../themeTokens.js'
  import CommitList from './CommitList.svelte'
  import DiffPanel from './DiffPanel.svelte'
  import { createTemplateHistoryController } from './templateHistoryController.svelte.js'

  let {
    dark = false,
    selectedTemplateId = '',
    selectedTemplateKind = '',
    onReverted = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const frameTone = $derived(dark ? 'border-zinc-700/70 bg-zinc-900/60' : 'border-zinc-200 bg-zinc-50/80')
  const cardTone = $derived(dark ? 'border-zinc-700/60 bg-zinc-900/40' : 'border-zinc-200 bg-white')
  const neutralButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800/80'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )

  const controller = createTemplateHistoryController({
    getSelectedTemplateId: () => selectedTemplateId,
    getSelectedTemplateKind: () => selectedTemplateKind,
    getOnReverted: () => onReverted,
  })
</script>

<section class="space-y-3 rounded-md border p-3 {frameTone}" data-testid="template-history-panel">
  <header class="flex items-center justify-between gap-3 border-b pb-2 {t.keyline}">
    <div>
      <h3 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Template History</h3>
      <p class="text-[11px] {t.textMuted}">Commit log, diff, and revert controls for template files.</p>
    </div>
    <div class="flex items-center gap-1">
      <span class="rounded-full px-2 py-0.5 text-[10px] font-medium {controller.storageStatus.dirty ? 'bg-warning-500/10 text-warning-500 border border-warning-500/40' : 'bg-success-500/10 text-success-500 border border-success-500/40'}" data-testid="template-history-dirty-indicator">
        {controller.loadingStatus ? 'Checking status…' : controller.storageStatus.dirty ? 'Dirty' : 'Clean'}
      </span>
      <span class="rounded-full border px-2 py-0.5 text-[10px] {neutralButton}">{String(controller.storageStatus.mode ?? 'unknown')}</span>
    </div>
  </header>

  <div class="flex flex-wrap items-center gap-1.5">
    <button class="rounded-md border px-2 py-1 text-[11px] {controller.scopeButtonTone(neutralButton, 'global')}" onclick={() => controller.setScope('global')} data-testid="template-history-scope-global">Global history</button>
    <button class="rounded-md border px-2 py-1 text-[11px] {controller.scopeButtonTone(neutralButton, 'template')} disabled:cursor-not-allowed disabled:opacity-60" onclick={() => controller.setScope('template')} disabled={!selectedTemplateId} data-testid="template-history-scope-template">Selected template</button>
    {#if selectedTemplateId}
      <span class="text-[11px] {t.textMuted}" data-testid="template-history-selected-template">{selectedTemplateId}</span>
    {/if}
  </div>

  {#if controller.errorMessage}
    <p class="rounded-md border border-danger-500/40 bg-danger-500/10 px-2 py-1 text-xs text-danger-400" data-testid="template-history-error">{controller.errorMessage}</p>
  {/if}
  {#if controller.infoMessage}
    <p class="rounded-md border border-success-500/40 bg-success-500/10 px-2 py-1 text-xs text-success-500">{controller.infoMessage}</p>
  {/if}

  <div class="grid grid-cols-1 gap-3 xl:grid-cols-[280px,1fr]">
    <CommitList
      {dark}
      {t}
      {cardTone}
      {neutralButton}
      loadingHistory={controller.loadingHistory}
      visibleCommits={controller.visibleCommits}
      selectedCommitId={controller.selectedCommitId}
      nextCursor={controller.nextCursor}
      loadingMore={controller.loadingMore}
      emptyMessage={controller.commitsEmptyMessage}
      onSelectCommit={controller.setSelectedCommit}
      onLoadMore={controller.loadMore}
      formatTimestamp={controller.formatTimestamp}
    />

    <section class="rounded-md border p-2 {cardTone}" data-testid="template-history-detail-panel">
      <div class="flex items-start justify-between gap-2">
        <div>
          <h4 class="text-xs font-semibold uppercase tracking-wide {t.textSecondary}">Commit Details</h4>
          {#if controller.selectedCommit}
            <p class="mt-1 text-[13px] font-medium {t.textPrimary}">{controller.selectedCommit.message}</p>
            <p class="text-[11px] {t.textMuted}">{controller.selectedCommit.shortId} · {controller.selectedCommit.author} · {controller.formatTimestamp(controller.selectedCommit.timestamp)}</p>
          {/if}
        </div>
        <button class="rounded-md border px-2 py-1 text-[11px] {neutralButton} disabled:cursor-not-allowed disabled:opacity-60" onclick={controller.revertSelected} disabled={!selectedTemplateId || !controller.selectedCommitId || controller.reverting} data-testid="template-history-revert-button">
          {controller.reverting ? 'Reverting…' : 'Revert selected'}
        </button>
      </div>

      {#if !selectedTemplateId}
        <p class="mt-2 text-xs {t.textMuted}" data-testid="template-history-template-hint">Select a role or preset in the catalog to enable template-specific revert.</p>
      {/if}

      {#if controller.selectedCommit}
        <div class="mt-2">
          <p class="text-[10px] uppercase tracking-wide {t.textMuted}">Files changed</p>
          <div class="mt-1 flex flex-wrap gap-1">
            {#each controller.selectedCommit.changedPaths as path}
              <span class="rounded-full border px-1.5 py-0.5 text-[10px] {neutralButton}">{path}</span>
            {/each}
          </div>
        </div>
      {/if}

      <DiffPanel
        {dark}
        {t}
        {frameTone}
        {neutralButton}
        loadingDiff={controller.loadingDiff}
        diff={controller.diff}
        selectedDiffPath={controller.selectedDiffPath}
        selectedDiffFile={controller.selectedDiffFile}
        onSelectDiffPath={controller.setSelectedDiffPath}
      />
    </section>
  </div>
</section>
