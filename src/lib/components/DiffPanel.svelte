<script>
  let {
    dark = false,
    t,
    frameTone = '',
    neutralButton = '',
    loadingDiff = false,
    diff = { files: [], stats: { filesChanged: 0, insertions: 0, deletions: 0 } },
    selectedDiffPath = '',
    selectedDiffFile = null,
    onSelectDiffPath = () => {},
  } = $props()
</script>

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
            onSelectDiffPath(file.path)
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
