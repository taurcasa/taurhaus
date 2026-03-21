<script>
  import { onMount, tick } from 'svelte'

  import MeshTeamBuilder from '../../lib/components/MeshTeamBuilder.svelte'

  const PINNED_ROLE_IDS_STORAGE_KEY = 'taurhaus.mesh.pinnedRoleIds'
  const CATALOG_DENSITY_STORAGE_KEY = 'taurhaus.mesh.roleCatalogDensity'

  let { scenario, theme = 'light', dark = false } = $props()

  const resolvedDark = $derived(dark || theme === 'dark')
  let ready = $state(false)
  const actions = $derived({
    onBrowseCatalog: () => {},
    onTeamNameChange: () => {},
    onDescriptionChange: () => {},
    onApplyPreset: () => {},
    onAssignLeadRole: () => {},
    onClearLead: () => {},
    onAppendAgentRole: () => {},
    onUpdateLead: () => {},
    onUpdateAgent: () => {},
    onRemoveAgent: () => {},
    onReorderAgent: () => {},
    onMoveAgentToEnd: () => {},
    onInitialize: () => {},
    onReset: () => {},
    onSavePreset: () => {},
  })

  onMount(async () => {
    try {
      window.localStorage.removeItem(PINNED_ROLE_IDS_STORAGE_KEY)
      window.localStorage.removeItem(CATALOG_DENSITY_STORAGE_KEY)
      if (Array.isArray(scenario?.pinnedRoleIds) && scenario.pinnedRoleIds.length > 0) {
        window.localStorage.setItem(
          PINNED_ROLE_IDS_STORAGE_KEY,
          JSON.stringify(scenario.pinnedRoleIds)
        )
      }
    } catch {
      // Ignore localStorage failures in visual host mode.
    }

    ready = true
    await tick()

    if (scenario?.expandCatalogAfterMount) {
      const catalog = document.querySelector('[data-testid="mesh-builder-catalog"]')
      if (catalog?.getAttribute('data-collapsed') === 'true') {
        document.querySelector('[data-testid="mesh-builder-catalog-toggle"]')?.click()
        await tick()
      }
    }
  })
</script>

<div class="w-full p-4">
  {#if ready}
    <MeshTeamBuilder
      dark={resolvedDark}
      mode={scenario?.mode ?? 'setup'}
      teamName={scenario?.teamName ?? ''}
      teamConfig={scenario?.teamConfig ?? { description: '', lead: null, agents: [] }}
      roleTemplates={scenario?.roleTemplates ?? []}
      presets={scenario?.presets ?? []}
      availableProjects={scenario?.availableProjects ?? []}
      onBrowseCatalog={actions.onBrowseCatalog}
      onTeamNameChange={actions.onTeamNameChange}
      onDescriptionChange={actions.onDescriptionChange}
      onApplyPreset={actions.onApplyPreset}
      onAssignLeadRole={actions.onAssignLeadRole}
      onClearLead={actions.onClearLead}
      onAppendAgentRole={actions.onAppendAgentRole}
      onUpdateLead={actions.onUpdateLead}
      onUpdateAgent={actions.onUpdateAgent}
      onRemoveAgent={actions.onRemoveAgent}
      onReorderAgent={actions.onReorderAgent}
      onMoveAgentToEnd={actions.onMoveAgentToEnd}
      onInitialize={actions.onInitialize}
      onReset={actions.onReset}
      onSavePreset={actions.onSavePreset}
    />
  {/if}
</div>
