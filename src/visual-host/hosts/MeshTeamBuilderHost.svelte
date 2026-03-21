<script>
  import MeshTeamBuilder from '../../lib/components/MeshTeamBuilder.svelte'

  let { scenario, theme = 'light', dark = false } = $props()

  const resolvedDark = $derived(dark || theme === 'dark')
  const actions = $derived({
    onBuildCustom: () => {},
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
</script>

<div class="mx-auto w-full max-w-[900px] p-4">
  <MeshTeamBuilder
    dark={resolvedDark}
    mode={scenario?.mode ?? 'setup'}
    teamName={scenario?.teamName ?? ''}
    teamConfig={scenario?.teamConfig ?? { description: '', lead: null, agents: [] }}
    roleTemplates={scenario?.roleTemplates ?? []}
    presets={scenario?.presets ?? []}
    availableProjects={scenario?.availableProjects ?? []}
    onBuildCustom={actions.onBuildCustom}
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
</div>
