<script>
  import { themeTokens } from '../themeTokens.js'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import PresetCatalog from './PresetCatalog.svelte'
  import RoleCatalog from './RoleCatalog.svelte'
  import RoleEditor from './RoleEditor.svelte'
  import SlideOver from './SlideOver.svelte'
  import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'
  import TemplateHistoryPanel from './TemplateHistoryPanel.svelte'
  import { createTemplateBrowserController } from './templateBrowserController.svelte.js'

  let {
    open = false,
    dark = false,
    modelCatalog = null,
    onClose = () => {},
    onSelectPreset = () => {},
    onSelectRole = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const tabBase = 'px-3 py-2 text-[11px] font-bold uppercase tracking-wider transition-all duration-200 border-b-2'
  const tabActive = $derived(dark ? 'text-brand-400 border-brand-500 bg-brand-500/5' : 'text-brand-600 border-brand-500 bg-brand-50/50')
  const tabInactive = 'text-zinc-500 border-transparent hover:text-zinc-400 hover:bg-zinc-500/5'
  const inputTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )
  const cardTone = $derived(
    dark
      ? 'bg-white/[0.03] border-white/[0.06] hover:bg-white/[0.05] hover:border-brand-500/30'
      : 'bg-brand-50/50 border-brand-200/40 hover:bg-brand-50/80 hover:border-brand-500/30'
  )
  const actionSecondary = $derived(
    dark
      ? 'bg-white/[0.05] border-white/[0.08] text-zinc-300 hover:text-white hover:bg-white/[0.1] active:scale-95'
      : 'bg-zinc-100 border-zinc-200 text-zinc-700 hover:bg-zinc-200 active:scale-95'
  )
  const toneMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')

  const controller = createTemplateBrowserController({
    getOpen: () => open,
  })
</script>

<SlideOver {open} title="Templates" width={420} {dark} onClose={onClose}>
  {#snippet children()}
    <section class="space-y-4 animate-in fade-in duration-200" data-testid="template-browser-panel">
      <header class="space-y-4">
        <div class="flex items-center gap-1 border-b {t.keyline} -mx-4 px-4 bg-black/5 dark:bg-white/5">
          <button class="{tabBase} {controller.activeTab === 'roles' ? tabActive : tabInactive}" onclick={() => controller.setTab('roles')} data-testid="catalog-tab-roles">Roles</button>
          <button class="{tabBase} {controller.activeTab === 'presets' ? tabActive : tabInactive}" onclick={() => controller.setTab('presets')} data-testid="catalog-tab-presets">Presets</button>
          <button class="{tabBase} {controller.activeTab === 'history' ? tabActive : tabInactive}" onclick={() => controller.setTab('history')} data-testid="catalog-tab-history">History</button>
        </div>

        {#if controller.activeTab !== 'history'}
          <div class="px-1">
            <label class="space-y-1.5 block">
              <span class="text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Filter</span>
              <div class="relative">
                <input
                  class="h-10 w-full rounded-lg border px-3 pr-10 text-sm transition-all outline-none {inputTone}"
                  placeholder={controller.activeTab === 'roles' ? 'Search roles by name, id, or model' : 'Search presets by name, id, or description'}
                  oninput={(event) => {
                    controller.setSearchQuery(event.currentTarget.value)
                  }}
                  data-testid="template-browser-search-input"
                />
                <div class="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
                </div>
              </div>
            </label>
          </div>
        {/if}
      </header>

      {#if controller.errorMessage}
        <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-danger-500 text-center">{controller.errorMessage}</p>
        </div>
      {/if}

      {#if controller.exportNotice}
        <div
          class="rounded-lg border border-brand-500/20 bg-brand-500/10 px-3 py-2 animate-in fade-in slide-in-from-bottom-2 duration-200"
          data-testid="template-browser-notice"
        >
          <p class="text-[11px] font-medium text-brand-600 dark:text-brand-300 text-center">
            {controller.exportNotice}
          </p>
        </div>
      {/if}

      {#if controller.loading}
        <div class="flex flex-col items-center justify-center py-12 space-y-3 opacity-50">
          <div class="w-6 h-6 border-2 border-brand-500 border-t-transparent rounded-full animate-spin"></div>
          <p class="text-[11px] font-bold uppercase tracking-widest text-brand-500" data-testid="template-browser-loading">Loading templates...</p>
        </div>
      {:else if controller.activeTab === 'roles'}
        <RoleCatalog
          {dark}
          {t}
          {cardTone}
          {actionSecondary}
          {toneMuted}
          detailKind={controller.detailKind}
          detailLoading={controller.detailLoading}
          selectedRole={controller.selectedRole}
          filteredRoleTemplates={controller.filteredRoleTemplates}
          hasCustomRoles={controller.hasCustomRoles}
          {onSelectRole}
          onResetDetail={controller.resetDetail}
          onOpenCreateRoleEditor={controller.openCreateRoleEditor}
          onImportRole={controller.importRole}
          onInspectRole={(role) => {
            void controller.inspectRole(role)
          }}
          onExportRole={(role, format) => {
            void controller.handleRoleExport(role, format)
          }}
          onOpenEditRoleEditor={(role) => {
            void controller.openEditRoleEditor(role)
          }}
          onRequestRoleDelete={controller.requestRoleDelete}
          exportingRoleId={controller.exportingRoleId}
        />
      {:else if controller.activeTab === 'presets'}
        <PresetCatalog
          {dark}
          {t}
          {cardTone}
          {actionSecondary}
          {toneMuted}
          detailKind={controller.detailKind}
          detailLoading={controller.detailLoading}
          selectedPreset={controller.selectedPreset}
          filteredTeamPresets={controller.filteredTeamPresets}
          {onSelectPreset}
          onResetDetail={controller.resetDetail}
          onOpenCreatePresetEditor={controller.openCreatePresetEditor}
          onInspectPreset={(preset) => {
            void controller.inspectPreset(preset)
          }}
          onOpenPresetEditorForMutation={(preset, mode) => {
            void controller.openPresetEditorForMutation(preset, mode)
          }}
          onRequestPresetDelete={controller.requestPresetDelete}
        />
      {:else}
        <div class="animate-in fade-in slide-in-from-right-2 duration-200">
          <TemplateHistoryPanel dark={dark} selectedTemplateId={controller.historyTemplateId} selectedTemplateKind={controller.historyTemplateKind} />
        </div>
      {/if}
    </section>
  {/snippet}
</SlideOver>

<RoleEditor
  open={controller.roleEditorOpen}
  {dark}
  {modelCatalog}
  role={controller.roleEditorRole}
  onSave={controller.handleRoleSave}
  onCancel={controller.resetRoleEditor}
  onDelete={(roleId) => {
    const role = controller.roleTemplates.find((entry) => entry.roleId === roleId)
    if (role) controller.requestRoleDelete(role)
    controller.resetRoleEditor()
  }}
/>

<TeamCustomizerPanel
  open={controller.presetEditorOpen}
  {dark}
  {modelCatalog}
  teamConfig={controller.presetEditorTeamConfig}
  onClose={controller.closePresetEditor}
  onSave={controller.savePresetFromCustomizer}
  onReset={controller.closePresetEditor}
/>

{#if controller.importConflict}
  <ConfirmDialog
    {dark}
    open={true}
    title="Role already exists?"
    message={`Role '${controller.importConflict.importedRole.name || controller.importConflict.existingRole.name || controller.importConflict.importedRole.roleId}' already exists. Replace it or skip this import?`}
    confirmLabel="Replace"
    secondaryLabel="Skip"
    variant="default"
    onConfirm={controller.replaceImportedRole}
    onSecondary={controller.skipImportConflict}
    onCancel={controller.clearImportConflict}
  />
{/if}

{#if controller.deleteRoleId}
  <ConfirmDialog
    {dark}
    open={true}
    title="Delete role template?"
    message={`Delete ${controller.deleteRoleName || controller.deleteRoleId}? This cannot be undone.`}
    confirmLabel="Delete"
    variant="danger"
    onConfirm={controller.confirmRoleDelete}
    onCancel={controller.cancelRoleDelete}
  />
{/if}

{#if controller.deletePresetId}
  <ConfirmDialog
    {dark}
    open={true}
    title="Delete team preset?"
    message={`Delete ${controller.deletePresetName || controller.deletePresetId}? This cannot be undone.`}
    confirmLabel="Delete"
    variant="danger"
    onConfirm={controller.confirmPresetDelete}
    onCancel={controller.cancelPresetDelete}
  />
{/if}
