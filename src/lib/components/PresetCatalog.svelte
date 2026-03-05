<script>
  import PresetCard from './PresetCard.svelte'
  import { isCustomPreset } from './templateBrowserUtils.js'

  let {
    dark = false,
    t,
    cardTone = '',
    actionSecondary = '',
    toneMuted = '',
    detailKind = '',
    detailLoading = false,
    selectedPreset = null,
    filteredTeamPresets = [],
    onSelectPreset = () => {},
    onResetDetail = () => {},
    onOpenCreatePresetEditor = () => {},
    onInspectPreset = () => {},
    onOpenPresetEditorForMutation = () => {},
    onRequestPresetDelete = () => {},
  } = $props()
</script>

{#if detailKind === 'preset'}
  <section class="rounded-xl border p-4 space-y-4 animate-in fade-in slide-in-from-left-2 duration-200 {cardTone}" data-testid="template-preset-detail">
    <button
      class="inline-flex items-center gap-1.5 h-8 px-2.5 rounded-lg text-[11px] font-bold uppercase tracking-wide {actionSecondary}"
      onclick={onResetDetail}
      data-testid="template-preset-back"
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
      Back
    </button>

    {#if detailLoading}
      <p class="text-xs text-center py-4 {toneMuted}">Loading preset details...</p>
    {:else if selectedPreset}
      <div class="space-y-2">
        <h3 class="text-base font-bold {t.textPrimary}">
          {selectedPreset.name}
        </h3>
        <p class="text-[10px] font-mono {toneMuted}">{selectedPreset.presetId}</p>
      </div>

      <div class="p-3 rounded-lg bg-black/5 dark:bg-white/5 border border-white/5">
        <p class="text-xs leading-relaxed {t.textSecondary}">
          {selectedPreset.description || 'No preset description provided.'}
        </p>
      </div>

      <div class="flex items-center gap-2 px-1">
        <span class="text-[10px] font-bold uppercase tracking-widest text-brand-500">Configuration:</span>
        <span class="text-[11px] font-medium {t.textSecondary}">
          {(selectedPreset.agentSlots ?? []).length} slot(s) configured.
        </span>
      </div>

      <button
        class="w-full h-10 rounded-lg bg-brand-600 px-4 py-1 text-xs font-bold text-white hover:bg-brand-500 shadow-lg shadow-brand-500/20 active:scale-95 transition-all"
        onclick={() => onSelectPreset(selectedPreset)}
        data-testid={`preset-select-${selectedPreset.presetId}`}
      >
        Use this Preset
      </button>
    {/if}
  </section>
{:else}
  <div class="flex items-center justify-between px-1">
    <p class="text-[10px] font-bold uppercase tracking-wider {t.textMuted}">Team Presets</p>
    <button
      class="h-8 px-3 rounded-lg text-[11px] font-bold text-white bg-brand-600 hover:bg-brand-500 active:scale-95 transition-all shadow-lg shadow-brand-500/10"
      onclick={onOpenCreatePresetEditor}
      data-testid="template-preset-create"
    >
      + Create
    </button>
  </div>

  {#if filteredTeamPresets.length === 0}
    <div class="flex flex-col items-center justify-center py-12 border-2 border-dashed rounded-xl {dark ? 'border-zinc-800' : 'border-zinc-200'}">
      <p class="text-xs {t.textMuted}">
        No team presets match the current filter.
      </p>
    </div>
  {:else}
    <div class="space-y-3" data-testid="template-preset-list">
      {#each filteredTeamPresets as preset, i}
        <article class="group space-y-3 rounded-xl border p-3 transition-all animate-in fade-in slide-in-from-bottom-1 duration-200 {cardTone}" style:transition-delay={`${i * 30}ms`}>
          <PresetCard
            dark={dark}
            name={preset.name}
            description={preset.description}
            leadCount={Math.max(1, Number(preset.roleCount ?? 1) - Number(preset.agentCount ?? 0))}
            agentCount={preset.agentCount}
            tools={preset.tools}
            builtIn={preset.builtIn}
            onSelect={() => {
              onSelectPreset(preset)
            }}
            onInspect={() => {
              onInspectPreset(preset)
            }}
            testId={`template-browser-preset-${preset.presetId}`}
          />

          <div class="flex flex-wrap justify-end gap-2 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
            <div class="flex gap-1.5">
              <button
                class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
                onclick={() => onSelectPreset(preset)}
                data-testid={`template-preset-use-${preset.presetId}`}
              >
                Use
              </button>
              <button
                class="h-8 px-3 rounded-lg text-[11px] font-bold {actionSecondary}"
                onclick={() => {
                  onInspectPreset(preset)
                }}
                data-testid={`template-preset-inspect-${preset.presetId}`}
              >
                Inspect
              </button>
            </div>

            {#if isCustomPreset(preset)}
              <div class="flex gap-1.5 ml-auto">
                <button
                  class="h-8 w-8 flex items-center justify-center rounded-lg {actionSecondary}"
                  onclick={() => {
                    onOpenPresetEditorForMutation(preset, 'edit')
                  }}
                  aria-label="Edit preset"
                  title="Edit preset"
                  data-testid={`template-preset-edit-${preset.presetId}`}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/><path d="m15 5 4 4"/></svg>
                </button>
                <button
                  class="h-8 w-8 flex items-center justify-center rounded-lg {actionSecondary}"
                  onclick={() => {
                    onOpenPresetEditorForMutation(preset, 'duplicate')
                  }}
                  aria-label="Duplicate preset"
                  title="Duplicate preset"
                  data-testid={`template-preset-duplicate-${preset.presetId}`}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>
                </button>
                <button
                  class="h-8 w-8 flex items-center justify-center rounded-lg border border-danger-500/20 text-danger-500 hover:bg-danger-500/10 active:scale-95 transition-all"
                  onclick={() => onRequestPresetDelete(preset)}
                  aria-label="Delete preset"
                  title="Delete preset"
                  data-testid={`template-preset-delete-${preset.presetId}`}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                </button>
              </div>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {/if}
{/if}
