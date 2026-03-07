<script>
  import { themeTokens } from '../themeTokens.js'
  import { inferTeamName } from './meshTabUtils.js'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import MeshRuntimeView from './MeshRuntimeView.svelte'
  import MeshSetupView from './MeshSetupView.svelte'
  import { createMeshTabController } from './meshTabController.svelte.js'

  let {
    dark = false,
    projectPath = '',
    availableProjects = [],
    onAddAgent: onAddAgentProp = () => {},
    onDisband: onDisbandProp = () => {},
    onRemoveAgent: onRemoveAgentProp = () => {},
    onFocusPane: onFocusPaneProp = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))

  const controller = createMeshTabController({
    getProjectPath: () => projectPath,
    getAvailableProjects: () => availableProjects,
    onAddAgent: (report) => onAddAgentProp(report),
    onDisband: (result) => onDisbandProp(result),
    onRemoveAgent: (result) => onRemoveAgentProp(result),
    onFocusPane: (paneId) => onFocusPaneProp(paneId),
  })

  const mode = $derived(controller.mode)
  const resolvedTeamName = $derived(controller.teamName || inferTeamName(projectPath))

  function confirmDialogTitle() {
    const context = controller.confirmContext
    if (!context) return ''
    if (context.kind === 'disband') return 'Disband team?'
    if (context.kind === 'remove') return `Remove ${context.memberName}?`
    return 'Confirm Action'
  }

  function confirmDialogMessage() {
    const context = controller.confirmContext
    if (!context) return ''
    if (context.kind === 'disband') {
      return `This will stop all active sessions and remove team '${resolvedTeamName}'.`
    }
    return `This removes '${context.memberName}' from team '${resolvedTeamName}'.`
  }

  function confirmDialogLabel() {
    const context = controller.confirmContext
    if (!context) return 'Confirm'
    return context.kind === 'disband' ? 'Disband Team' : 'Remove Member'
  }
</script>

<section class="flex-1 min-h-0 overflow-y-auto {t.mainBg}" data-testid="mesh-tab">
  {#if controller.errorMessage}
    <div class="relative overflow-hidden border-l-2 border-danger-400 pl-3 pr-2 py-1 text-xs text-danger-600/95 flex items-center justify-between gap-2" data-testid="mesh-error">
      <span class="min-w-0">{controller.errorMessage}</span>
      <button
        class="text-xs opacity-60 hover:opacity-100 ml-2"
        aria-label="Dismiss"
        onclick={controller.dismissError}
        data-testid="mesh-dismiss-error-message"
      >
        ✕
      </button>
      <div class="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-danger-400/50 animate-[shrink_8s_linear_forwards]" style="width: 100%"></div>
    </div>
  {/if}

  {#if controller.runtimeMessage}
    <div class="relative overflow-hidden border-l-2 border-success-400 pl-3 pr-2 py-1 text-xs text-success-600/95 flex items-center justify-between gap-2" data-testid="mesh-runtime-message">
      <span class="min-w-0">{controller.runtimeMessage}</span>
      <button
        class="text-xs opacity-60 hover:opacity-100 ml-2"
        aria-label="Dismiss"
        onclick={controller.dismissRuntimeMessage}
        data-testid="mesh-dismiss-runtime-message"
      >
        ✕
      </button>
      <div class="pointer-events-none absolute bottom-0 left-0 h-0.5 bg-success-400/50 animate-[shrink_5s_linear_forwards]" style="width: 100%"></div>
    </div>
  {/if}

  {#if controller.availabilityMessage}
    <div
      class="border-l-2 border-warning-400/80 pl-3 pr-2 py-1 text-xs text-warning-700/95 bg-warning-50/50 dark:bg-warning-500/8 dark:text-warning-200/95"
      data-testid="mesh-availability-inline"
    >
      {controller.availabilityMessage}
    </div>
  {/if}

  {#if mode === 'runtime'}
    <MeshRuntimeView
      {dark}
      teamName={resolvedTeamName}
      teamConfig={controller.teamConfig}
      selectedNode={controller.selectedNode}
      selectedNodeId={controller.selectedNodeId}
      teamRuntimeState={controller.teamRuntimeState}
      isResumingTeam={controller.isResumingTeam}
      resumeProgress={controller.teamResumeProgress}
      {availableProjects}
      addAgentOpen={controller.slideOver === 'addAgent'}
      addAgentDraft={controller.addAgentDraft}
      canSubmitAddAgent={controller.canSubmitAddAgent}
      roleTemplates={controller.roleTemplates}
      loadingRoles={controller.loadingRoles}
      captureRoleDraft={controller.captureRoleDraft}
      canSaveCapturedRole={controller.canSaveCapturedRole}
      onNodeClick={controller.toggleNode}
      onOpenAddAgent={controller.openAddAgentPanel}
      onRequestDisband={controller.requestDisband}
      onResumeTeam={controller.resumeTeam}
      onCloseNode={controller.clearSelectedNode}
      onResumeSelected={controller.resumeSelected}
      onStopSelected={controller.stopSelected}
      onFocusSelectedPane={controller.focusSelectedPane}
      onCaptureRole={controller.openCaptureRoleDialog}
      onCloseAddAgent={controller.closeSlideOver}
      onAddAgentRoleChange={controller.handleRoleChange}
      onToggleAddAgentLock={controller.toggleAddAgentLock}
      onUpdateAddAgentField={controller.updateAddAgentField}
      onSubmitAddAgent={controller.submitAddAgent}
      onCloseCaptureRole={controller.closeCaptureRoleDialog}
      onCaptureRoleName={controller.updateCaptureRoleName}
      onCaptureRoleId={controller.updateCaptureRoleId}
      onToggleCaptureRoleFlag={controller.toggleCaptureRoleFlag}
      onSubmitCaptureRole={controller.submitCaptureRole}
    />
  {:else}
    <MeshSetupView
      mode={mode}
      {dark}
      {projectPath}
      teamConfig={controller.teamConfig}
      selectedNode={controller.selectedNode}
      selectedNodeId={controller.selectedNodeId}
      teamName={resolvedTeamName}
      canInitialize={controller.canInitialize}
      initProgress={controller.initProgress}
      quickPresets={controller.quickPresets}
      {availableProjects}
      slideOver={controller.slideOver}
      slideOverContext={controller.slideOverContext}
      onGateReady={controller.ensureGateReady}
      onSelectPreset={controller.handlePresetSelect}
      onBrowseTemplates={controller.openTemplates}
      onStartCustom={controller.handleStartCustom}
      onNodeClick={controller.toggleNode}
      onOpenCustomizer={controller.openCustomizer}
      onRemoveSetupNode={controller.removeSelectedSetupNode}
      onCloseNode={controller.clearSelectedNode}
      onInitialize={controller.handleInitialize}
      onReset={controller.handleReset}
      onInitializeBack={controller.setInitializingBack}
      onInitializeSuccess={controller.handleInitializeSuccess}
      onCloseSlideOver={controller.closeSlideOver}
      onSelectPresetFromBrowser={controller.handlePresetSelect}
      onSelectRoleFromBrowser={controller.openRoleFromBrowser}
      onTeamSave={controller.handleTeamSave}
    />
  {/if}

  {#if controller.confirmContext}
    <ConfirmDialog
      {dark}
      open={true}
      title={confirmDialogTitle()}
      message={confirmDialogMessage()}
      confirmLabel={confirmDialogLabel()}
      variant="danger"
      onConfirm={controller.handleConfirmAction}
      onCancel={controller.cancelConfirm}
    />
  {/if}
</section>

<style>
  @keyframes shrink {
    from {
      width: 100%;
    }
    to {
      width: 0%;
    }
  }

  @keyframes meshfade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>
