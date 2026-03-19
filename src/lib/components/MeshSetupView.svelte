<script>
  import { themeTokens } from '../themeTokens.js'
  import MeshCanvas from './MeshCanvas.svelte'
  import MeshInitProgress from './MeshInitProgress.svelte'
  import MeshTeamBuilder from './MeshTeamBuilder.svelte'

  let {
    mode = 'gate',
    dark = false,
    projectPath = '',
    teamConfig = null,
    teamName = '',
    initProgress = null,
    quickPresets = [],
    roleTemplates = [],
    availableProjects = [],
    onGateReady = () => {},
    onApplyPreset = () => {},
    onBrowseCatalog = () => {},
    onStartCustom = () => {},
    onTeamNameChange = () => {},
    onDescriptionChange = () => {},
    onAssignLeadRole = () => {},
    onClearLead = () => {},
    onAppendAgentRole = () => {},
    onUpdateLead = () => {},
    onUpdateAgent = () => {},
    onRemoveAgent = () => {},
    onReorderAgent = () => {},
    onMoveAgentToEnd = () => {},
    onInitialize = () => {},
    onReset = () => {},
    onSavePreset = () => {},
    onInitializeBack = () => {},
    onInitializeSuccess = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
</script>

{#if mode === 'gate'}
  <div class="max-w-2xl mx-auto px-6 pt-4 pb-6 space-y-4" data-testid="mesh-mode-gate">
    <p class="text-xs {t.textMuted}" data-testid="mesh-gate-ready">
      Checking project team state...
    </p>
  </div>
{:else if mode === 'empty'}
  <div class="px-4 pt-2 pb-4 space-y-3">
    <MeshTeamBuilder
      mode="empty"
      {dark}
      {teamName}
      {teamConfig}
      roleTemplates={roleTemplates}
      presets={quickPresets}
      {availableProjects}
      onBuildCustom={onStartCustom}
      onBrowseCatalog={onBrowseCatalog}
      onTeamNameChange={onTeamNameChange}
      onDescriptionChange={onDescriptionChange}
      onApplyPreset={onApplyPreset}
      onAssignLeadRole={onAssignLeadRole}
      onClearLead={onClearLead}
      onAppendAgentRole={onAppendAgentRole}
      onUpdateLead={onUpdateLead}
      onUpdateAgent={onUpdateAgent}
      onRemoveAgent={onRemoveAgent}
      onReorderAgent={onReorderAgent}
      onMoveAgentToEnd={onMoveAgentToEnd}
      onInitialize={onInitialize}
      onReset={onReset}
      onSavePreset={onSavePreset}
    />
  </div>
{:else if mode === 'setup'}
  <div class="px-4 pt-2 pb-4 space-y-3">
    <MeshTeamBuilder
      mode="setup"
      {dark}
      {teamName}
      {teamConfig}
      roleTemplates={roleTemplates}
      presets={quickPresets}
      {availableProjects}
      onBuildCustom={onStartCustom}
      onBrowseCatalog={onBrowseCatalog}
      onTeamNameChange={onTeamNameChange}
      onDescriptionChange={onDescriptionChange}
      onApplyPreset={onApplyPreset}
      onAssignLeadRole={onAssignLeadRole}
      onClearLead={onClearLead}
      onAppendAgentRole={onAppendAgentRole}
      onUpdateLead={onUpdateLead}
      onUpdateAgent={onUpdateAgent}
      onRemoveAgent={onRemoveAgent}
      onReorderAgent={onReorderAgent}
      onMoveAgentToEnd={onMoveAgentToEnd}
      onInitialize={onInitialize}
      onReset={onReset}
      onSavePreset={onSavePreset}
    />
  </div>
{:else if mode === 'initializing'}
  <div class="px-4 pt-2 pb-4 space-y-3">
    <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-initializing">
      <div data-testid="mesh-initializing-canvas-frame">
        <MeshCanvas
          lead={teamConfig?.lead ?? null}
          agents={teamConfig?.agents ?? []}
          mode="initializing"
          initSteps={null}
          {dark}
          onNodeClick={() => {}}
          onAddClick={() => {}}
        />
      </div>

      <MeshInitProgress
        {dark}
        request={initProgress}
        onSuccess={onInitializeSuccess}
        onBack={onInitializeBack}
      />
    </div>
  </div>
{/if}
