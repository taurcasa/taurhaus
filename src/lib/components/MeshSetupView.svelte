<script>
  import { themeTokens } from '../themeTokens.js'
  import MeshActionBar from './MeshActionBar.svelte'
  import MeshAvailabilityGate from './MeshAvailabilityGate.svelte'
  import MeshCanvas from './MeshCanvas.svelte'
  import MeshEmptyState from './MeshEmptyState.svelte'
  import MeshInitProgress from './MeshInitProgress.svelte'
  import MeshNodeDetail from './MeshNodeDetail.svelte'
  import TeamCustomizerPanel from './TeamCustomizerPanel.svelte'
  import TemplateBrowserPanel from './TemplateBrowserPanel.svelte'

  let {
    mode = 'gate',
    dark = false,
    projectPath = '',
    teamConfig = null,
    selectedNode = null,
    selectedNodeId = null,
    teamName = '',
    canInitialize = false,
    initProgress = null,
    quickPresets = [],
    availableProjects = [],
    slideOver = null,
    slideOverContext = null,
    onGateReady = () => {},
    onSelectPreset = () => {},
    onBrowseTemplates = () => {},
    onStartCustom = () => {},
    onNodeClick = () => {},
    onOpenCustomizer = () => {},
    onRemoveSetupNode = () => {},
    onCloseNode = () => {},
    onInitialize = () => {},
    onReset = () => {},
    onInitializeBack = () => {},
    onInitializeSuccess = () => {},
    onCloseSlideOver = () => {},
    onSelectPresetFromBrowser = () => {},
    onSelectRoleFromBrowser = () => {},
    onTeamSave = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  let nodeDetailAnchor = $state(null)
  const detailNode = $derived.by(() => {
    if (!selectedNode || typeof selectedNode !== 'object') return null
    return {
      name: selectedNode.name,
      role: selectedNode.role,
      tool: selectedNode.tool ?? selectedNode.cliTool ?? selectedNode.cli_tool,
      model: selectedNode.model ?? selectedNode.modelName ?? selectedNode.model_name,
      status: selectedNode.status ?? selectedNode.sessionStatus ?? selectedNode.session_status,
      projectId: selectedNode.projectId ?? selectedNode.project_id,
      isCrossProject: selectedNode.isCrossProject ?? selectedNode.is_cross_project ?? false,
      projectLabel: selectedNode.projectLabel ?? selectedNode.project_label ?? '',
      description: selectedNode.description ?? '',
      roleName: selectedNode.roleName ?? selectedNode.role_name ?? '',
      focusArea: selectedNode.focusArea ?? selectedNode.focus_area ?? '',
      contextSummary: selectedNode.contextSummary ?? selectedNode.context_summary ?? '',
      behaviorSummary: selectedNode.behaviorSummary ?? selectedNode.behavior_summary ?? '',
      paneId: selectedNode.paneId ?? selectedNode.pane_id ?? '',
      sessionId: selectedNode.sessionId ?? selectedNode.session_id ?? '',
      sessionState: selectedNode.sessionState ?? selectedNode.session_status ?? '',
    }
  })

  function triggerGateReady(node) {
    void node
    onGateReady()
    return {}
  }

  function handleDetailAnchorChange(anchor) {
    nodeDetailAnchor = anchor
  }

  $effect(() => {
    if (selectedNode) return
    nodeDetailAnchor = null
  })
</script>

{#if mode === 'gate'}
  <div class="max-w-2xl mx-auto px-6 pt-4 pb-6 space-y-4" data-testid="mesh-mode-gate">
    <MeshAvailabilityGate {dark} {projectPath}>
      {#snippet children(_agentWarnings)}
        <p class="text-xs {t.textMuted}" data-testid="mesh-gate-ready" use:triggerGateReady>
          Checking project team state...
        </p>
      {/snippet}
    </MeshAvailabilityGate>
  </div>
{:else if mode === 'empty'}
  <div class="max-w-2xl mx-auto px-6 pt-4 pb-6 space-y-4">
    <div class="animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-empty">
      <MeshEmptyState
        {dark}
        presets={quickPresets}
        onSelectPreset={onSelectPreset}
        onBrowseTemplates={onBrowseTemplates}
        onStartCustom={onStartCustom}
      />
    </div>
  </div>
{:else if mode === 'setup'}
  <div class="px-4 pt-2 pb-4 space-y-3">
    <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-setup">
      <div class="relative" data-testid="mesh-setup-canvas-frame">
        <MeshCanvas
          lead={teamConfig?.lead ?? null}
          agents={teamConfig?.agents ?? []}
          mode="setup"
          {dark}
          onNodeClick={onNodeClick}
          onAddClick={onOpenCustomizer}
          onDetailAnchorChange={handleDetailAnchorChange}
          onDismissDetail={onCloseNode}
          {selectedNodeId}
        />

        {#if detailNode && nodeDetailAnchor}
          <div class="pointer-events-none absolute inset-0 z-20" data-testid="mesh-node-detail-host">
          <MeshNodeDetail
            node={detailNode}
            mode="setup"
            {dark}
            anchor={nodeDetailAnchor}
            actions={{
              onEdit: onOpenCustomizer,
              onRemove: onRemoveSetupNode,
              onClose: onCloseNode,
            }}
          />
          </div>
        {/if}
      </div>

      <MeshActionBar
        {canInitialize}
        {teamName}
        {dark}
        onInitialize={onInitialize}
        onOpenCustomizer={onOpenCustomizer}
        onReset={onReset}
      />
    </div>
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

{#if slideOver === 'templates'}
  <TemplateBrowserPanel
    open={true}
    {dark}
    onClose={onCloseSlideOver}
    onSelectPreset={onSelectPresetFromBrowser}
    onSelectRole={onSelectRoleFromBrowser}
  />
{/if}

{#if slideOver === 'customizer'}
  <TeamCustomizerPanel
    open={true}
    {dark}
    {projectPath}
    {availableProjects}
    {teamConfig}
    context={slideOverContext}
    onClose={onCloseSlideOver}
    onSave={onTeamSave}
    onReset={onReset}
  />
{/if}
