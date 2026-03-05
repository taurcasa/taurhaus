<script>
  let {
    dark = false,
    t,
    hoverRow = '',
    badgeBg = '',
    scanning = false,
    scanError = null,
    discovered = [],
    selectableProjects = [],
    selected = new Set(),
    selectedCount = 0,
    allSelected = false,
    registering = false,
    onToggleProject = () => {},
    onSelectAll = () => {},
    onDeselectAll = () => {},
    onRegister = () => {},
    onEnterManualMode = () => {},
    onEnterCreateMode = () => {},
    onRetryScan = () => {},
  } = $props()
</script>

{#if scanning}
  <div class="text-center py-4" data-testid="scanning-state">
    <div class="w-4 h-4 border-2 border-brand-500 border-t-transparent rounded-full animate-spin mx-auto mb-2"></div>
    <p class="text-[12px] {t.textTertiary}">Scanning ~/projects/...</p>
  </div>

{:else if scanError}
  <div class="text-center py-4" data-testid="scan-error">
    <p class="text-[13px] {t.textPrimary} mb-1">Scan failed</p>
    <p class="text-[11px] text-danger-500 mb-3">{scanError}</p>
    <div class="flex items-center justify-center gap-3">
      <button class="text-[12px] {t.linkColor} transition-colors" onclick={onRetryScan}>Try again</button>
      <span class="{t.textTertiary}">·</span>
      <button class="text-[12px] {t.linkColor} transition-colors" onclick={onEnterManualMode} data-testid="enter-manual-mode">Browse manually</button>
    </div>
  </div>

{:else if selectableProjects.length === 0 && discovered.length > 0}
  <div class="text-center py-4" data-testid="all-registered">
    <p class="text-[13px] {t.textSecondary}">All projects in ~/projects/ are already registered.</p>
    <button class="text-[12px] {t.linkColor} transition-colors mt-2" onclick={onEnterManualMode} data-testid="enter-manual-mode">Browse manually</button>
  </div>

{:else if discovered.length === 0}
  <div class="text-center py-4" data-testid="empty-scan">
    <p class="text-[13px] {t.textSecondary}">No new projects found in ~/projects/.</p>
    <button class="text-[12px] {t.linkColor} transition-colors mt-2" onclick={onEnterManualMode} data-testid="enter-manual-mode">Browse manually</button>
  </div>

{:else}
  <div class="flex items-center gap-3 mb-2">
    <p class="text-[12px] {t.textSecondary}">
      {selectableProjects.length} new project{selectableProjects.length !== 1 ? 's' : ''}
    </p>
    <span class="flex-1"></span>
    {#if selectableProjects.length > 1}
      <button class="text-[11px] {t.linkColor} transition-colors" onclick={allSelected ? onDeselectAll : onSelectAll}>
        {allSelected ? 'Deselect all' : 'Select all'}
      </button>
    {/if}
  </div>

  <div class="border {t.keyline} rounded-lg overflow-hidden max-h-[200px] overflow-y-auto" data-testid="discovered-list">
    {#each selectableProjects as project}
      {@const isSelected = selected.has(project.path)}
      <button
        class="w-full flex items-center gap-3 px-3 py-2 text-left border-b last:border-b-0 {t.keyline} {hoverRow} transition-colors"
        onclick={() => onToggleProject(project.path)}
      >
        <div class="w-4 h-4 rounded border flex items-center justify-center shrink-0 {isSelected ? 'bg-brand-600 border-brand-600' : t.checkBg}">
          {#if isSelected}
            <svg class="w-3 h-3 text-white" fill="none" viewBox="0 0 24 24" stroke-width="3" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/></svg>
          {/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-[13px] font-medium {t.textPrimary} truncate">{project.name}</div>
          <div class="text-[11px] {t.textTertiary} truncate font-mono">{project.path}</div>
        </div>
        {#if project.has_git}
          <span class="text-[10px] px-1.5 py-0.5 rounded {badgeBg}">git</span>
        {/if}
      </button>
    {/each}
  </div>
{/if}

{#if !scanning && !scanError}
  <div class="flex items-center justify-between mt-3">
    <div class="flex items-center gap-3">
      <button
        class="text-[12px] {t.linkColor} transition-colors"
        onclick={onEnterManualMode}
        data-testid="enter-manual-mode"
      >Enter path manually</button>
      <button
        class="text-[12px] {t.linkColor} transition-colors"
        onclick={onEnterCreateMode}
        data-testid="enter-create-mode"
      >Create new project</button>
    </div>
    {#if selectableProjects.length > 0}
      <button
        class="px-3 py-1.5 rounded-md bg-brand-600 text-white text-[12px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
        onclick={onRegister}
        disabled={selectedCount === 0 || registering}
        data-testid="register-button"
      >
        {registering ? 'Registering...' : `Register ${selectedCount}`}
      </button>
    {/if}
  </div>
{/if}
