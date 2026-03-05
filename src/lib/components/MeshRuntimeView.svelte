<script>
  import { defaultModelForTool, modelsForTool } from '../meshDefaults.js'
  import { normalizeProjectOption } from '../projectOptions.js'
  import { themeTokens } from '../themeTokens.js'
  import MeshCanvas from './MeshCanvas.svelte'
  import MeshNodeDetail from './MeshNodeDetail.svelte'
  import MeshRuntimeBar from './MeshRuntimeBar.svelte'
  import SlideOver from './SlideOver.svelte'

  let {
    dark = false,
    teamName = '',
    teamConfig = null,
    selectedNode = null,
    selectedNodeId = null,
    availableProjects = [],
    addAgentOpen = false,
    addAgentDraft = null,
    canSubmitAddAgent = false,
    roleTemplates = [],
    loadingRoles = false,
    captureRoleDraft = null,
    canSaveCapturedRole = false,
    onNodeClick = () => {},
    onOpenAddAgent = () => {},
    onRequestDisband = () => {},
    onCloseNode = () => {},
    onResumeSelected = () => {},
    onStopSelected = () => {},
    onFocusSelectedPane = () => {},
    onCaptureRole = () => {},
    onCloseAddAgent = () => {},
    onAddAgentRoleChange = () => {},
    onToggleAddAgentLock = () => {},
    onUpdateAddAgentField = () => {},
    onSubmitAddAgent = () => {},
    onCloseCaptureRole = () => {},
    onCaptureRoleName = () => {},
    onCaptureRoleId = () => {},
    onToggleCaptureRoleFlag = () => {},
    onSubmitCaptureRole = () => {},
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
      description: selectedNode.description ?? '',
      paneId: selectedNode.paneId ?? selectedNode.pane_id ?? '',
      sessionId: selectedNode.sessionId ?? selectedNode.session_id ?? '',
      sessionState: selectedNode.sessionState ?? selectedNode.session_status ?? '',
    }
  })
  const canFocusSelectedPane = $derived.by(
    () => Boolean(selectedNode?.paneId ?? selectedNode?.pane_id)
  )
  const fieldTone = $derived(
    dark
      ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20'
      : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'
  )
  const selectScheme = $derived(dark ? '[color-scheme:dark]' : '[color-scheme:light]')

  const projectOptions = $derived.by(() =>
    (availableProjects ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
  )

  function handleDetailAnchorChange(anchor) {
    nodeDetailAnchor = anchor
  }

  $effect(() => {
    if (selectedNode) return
    nodeDetailAnchor = null
  })
</script>

<div class="px-4 pt-2 pb-4 space-y-3">
  <div class="space-y-3 animate-[meshfade_180ms_ease-out]" data-testid="mesh-mode-runtime">
    <MeshRuntimeBar
      {teamName}
      agents={teamConfig?.agents ?? []}
      {dark}
      onAddAgent={onOpenAddAgent}
      onDisband={onRequestDisband}
    />

    <div class="relative" data-testid="mesh-runtime-canvas-frame">
      <MeshCanvas
        lead={teamConfig?.lead ?? null}
        agents={teamConfig?.agents ?? []}
        mode="runtime"
        {dark}
        {selectedNodeId}
        onNodeClick={onNodeClick}
        onAddClick={() => {}}
        onDetailAnchorChange={handleDetailAnchorChange}
        onDismissDetail={onCloseNode}
      />

      {#if detailNode && nodeDetailAnchor}
        <div class="pointer-events-none absolute inset-0 z-20" data-testid="mesh-node-detail-host">
          <MeshNodeDetail
            node={detailNode}
            mode="runtime"
            {dark}
            anchor={nodeDetailAnchor}
            actions={{
              onResume: onResumeSelected,
              onStop: onStopSelected,
              onFocusPane: canFocusSelectedPane ? onFocusSelectedPane : null,
              onCapture: onCaptureRole,
              onClose: onCloseNode,
            }}
          />
        </div>
      {/if}
    </div>
  </div>
</div>

<SlideOver
  open={addAgentOpen}
  title="Add Agent"
  width={420}
  {dark}
  onClose={onCloseAddAgent}
>
  {#snippet children()}
    <section class="space-y-5 animate-in fade-in slide-in-from-bottom-1 duration-200" data-testid="mesh-add-agent-form">
      <p class="text-sm {t.textMuted} px-1">Hot-add one member to <span class="font-medium {t.textSecondary}">{teamName}</span>.</p>

      <div class="space-y-2 p-3 rounded-xl border transition-all {dark ? 'bg-brand-500/[0.03] border-brand-500/20 border-l-2 border-l-brand-500' : 'bg-brand-50/50 border-brand-200 border-l-2 border-l-brand-500'}">
        <label for="mesh-add-agent-role-select-input" class="block text-[10px] font-bold uppercase tracking-wide text-brand-500">Pick from Role (Optional)</label>
        <div class="relative">
          <select
            id="mesh-add-agent-role-select-input"
            class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme}"
            value={addAgentDraft?.roleId ?? ''}
            onchange={(event) => onAddAgentRoleChange(event.currentTarget.value)}
            disabled={addAgentDraft?.submitting || loadingRoles}
            data-testid="mesh-add-agent-role-select"
          >
            {#if loadingRoles}
              <option value="">Loading roles...</option>
            {:else}
              <option value="">Manual configuration</option>
              {#each roleTemplates as role}
                <option value={role.roleId}>{role.name}</option>
              {/each}
            {/if}
          </select>
          <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-brand-500/60">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
          </div>
        </div>
      </div>

      <div class="space-y-4">
        <div class="space-y-1.5">
          <label for="mesh-add-agent-name-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Agent Name</label>
          <input
            id="mesh-add-agent-name-input-field"
            class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {fieldTone}"
            placeholder="e.g. backend-dev"
            value={addAgentDraft?.name ?? ''}
            oninput={(event) => onUpdateAddAgentField('name', event.currentTarget.value)}
            disabled={addAgentDraft?.submitting}
            data-testid="mesh-add-agent-name-input"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <div class="flex items-center justify-between px-1">
              <label for="mesh-add-agent-tool-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted}">Tool</label>
              {#if addAgentDraft?.roleId}
                <button
                  type="button"
                  class="h-5 w-5 flex items-center justify-center rounded-md transition-all {addAgentDraft.isLocked ? 'text-brand-500 bg-brand-500/10' : 'text-zinc-400 hover:bg-black/5 dark:hover:bg-white/5'}"
                  onclick={onToggleAddAgentLock}
                  title={addAgentDraft.isLocked ? 'Unlock to edit' : 'Lock fields'}
                  data-testid="mesh-add-agent-unlock-toggle"
                >
                  {#if addAgentDraft.isLocked}
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                  {:else}
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></svg>
                  {/if}
                </button>
              {/if}
            </div>
            <div class="relative">
              <select
                id="mesh-add-agent-tool-select-input"
                class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
                value={addAgentDraft?.tool ?? 'codex'}
                onchange={(event) => onUpdateAddAgentField('tool', event.currentTarget.value)}
                disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
                data-testid="mesh-add-agent-tool-select"
              >
                <option value="claude">Claude</option>
                <option value="codex">Codex</option>
                <option value="gemini">Gemini</option>
              </select>
              <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
              </div>
            </div>
          </div>

          <div class="space-y-1.5">
            <label for="mesh-add-agent-model-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Model</label>
            <div class="relative">
              <select
                id="mesh-add-agent-model-select-input"
                class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
                value={addAgentDraft?.model ?? defaultModelForTool(addAgentDraft?.tool ?? 'codex')}
                onchange={(event) => onUpdateAddAgentField('model', event.currentTarget.value)}
                disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
                data-testid="mesh-add-agent-model-select"
              >
                {#each modelsForTool(addAgentDraft?.tool ?? 'codex') as model}
                  <option value={model}>{model}</option>
                {/each}
              </select>
              <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
              </div>
            </div>
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="mesh-add-agent-project-select-input" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Project Binding</label>
          <div class="relative">
            <select
              id="mesh-add-agent-project-select-input"
              class="h-10 w-full rounded-lg border px-3 pr-8 text-sm transition-all outline-none appearance-none {fieldTone} {selectScheme}"
              value={addAgentDraft?.projectId ?? ''}
              onchange={(event) => onUpdateAddAgentField('projectId', event.currentTarget.value)}
              disabled={addAgentDraft?.submitting}
              data-testid="mesh-add-agent-project-select"
            >
              <option value="">Select project</option>
              {#each projectOptions as project}
                <option value={project.id}>{project.label}</option>
              {/each}
            </select>
            <div class="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-9"/></svg>
            </div>
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="mesh-add-agent-description-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Description</label>
          <textarea
            id="mesh-add-agent-description-input-field"
            class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none {fieldTone} {addAgentDraft?.isLocked ? 'opacity-50 cursor-not-allowed' : ''}"
            rows="3"
            placeholder="Specific goals for this agent..."
            value={addAgentDraft?.description ?? ''}
            oninput={(event) => onUpdateAddAgentField('description', event.currentTarget.value)}
            disabled={addAgentDraft?.submitting || addAgentDraft?.isLocked}
            data-testid="mesh-add-agent-description-input"
          ></textarea>
        </div>
      </div>

      {#if addAgentDraft?.error}
        <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-danger-500 text-center" data-testid="mesh-add-agent-error">{addAgentDraft.error}</p>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-3 pt-4 border-t {t.keyline}">
        <button
          class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
          type="button"
          onclick={onCloseAddAgent}
          disabled={addAgentDraft?.submitting}
          data-testid="mesh-add-agent-cancel"
        >
          Cancel
        </button>
        <button
          class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
          type="button"
          onclick={onSubmitAddAgent}
          disabled={!canSubmitAddAgent}
          data-testid="mesh-add-agent-submit"
        >
          {addAgentDraft?.submitting ? 'Adding...' : 'Add Agent'}
        </button>
      </div>
    </section>
  {/snippet}
</SlideOver>

<SlideOver
  open={Boolean(captureRoleDraft)}
  title="Capture as Role"
  width={360}
  {dark}
  onClose={onCloseCaptureRole}
>
  {#snippet children()}
    <section class="space-y-4 animate-in fade-in slide-in-from-bottom-1 duration-200" data-testid="mesh-capture-role-form">
      <p class="text-sm {t.textMuted} px-1">
        Save the selected runtime member as a reusable catalog role.
      </p>

      <div class="space-y-4">
        <div class="space-y-1.5">
          <label for="mesh-capture-role-name-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">New Role Name</label>
          <input
            id="mesh-capture-role-name-input-field"
            class="h-10 w-full rounded-lg border px-3 text-base transition-all outline-none {dark ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20' : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'}"
            value={captureRoleDraft?.name ?? ''}
            oninput={(event) => onCaptureRoleName(event.currentTarget.value)}
            disabled={captureRoleDraft?.submitting}
            data-testid="mesh-capture-role-name-input"
          />
        </div>

        <div class="space-y-1.5">
          <label for="mesh-capture-role-id-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Role ID</label>
          <input
            id="mesh-capture-role-id-input-field"
            class="h-9 w-full rounded-lg border px-3 text-sm transition-all outline-none {dark ? 'bg-zinc-950/50 border-white/[0.08] text-zinc-100 placeholder-zinc-600 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20' : 'bg-white border-brand-200/60 text-zinc-900 placeholder-zinc-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/10'}"
            value={captureRoleDraft?.roleId ?? ''}
            oninput={(event) => onCaptureRoleId(event.currentTarget.value)}
            disabled={captureRoleDraft?.submitting}
            data-testid="mesh-capture-role-id-input"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <label for="mesh-capture-role-tool-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Tool</label>
            <input
              id="mesh-capture-role-tool-input-field"
              class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
              value={captureRoleDraft?.tool ?? ''}
              readonly
              disabled
              data-testid="mesh-capture-role-tool-input"
            />
          </div>
          <div class="space-y-1.5">
            <label for="mesh-capture-role-model-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Model</label>
            <input
              id="mesh-capture-role-model-input-field"
              class="h-10 w-full rounded-lg border px-3 text-sm transition-all outline-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
              value={captureRoleDraft?.model ?? ''}
              readonly
              disabled
              data-testid="mesh-capture-role-model-input"
            />
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="mesh-capture-role-description-input-field" class="block text-[10px] font-bold uppercase tracking-wide {t.textMuted} px-1">Current Description</label>
          <textarea
            id="mesh-capture-role-description-input-field"
            class="w-full rounded-lg border px-3 py-2 text-sm transition-all outline-none resize-none bg-black/5 dark:bg-white/5 border-transparent opacity-60"
            rows="3"
            value={captureRoleDraft?.description ?? ''}
            readonly
            disabled
            data-testid="mesh-capture-role-description-input"
          ></textarea>
        </div>
      </div>

      <div class="space-y-2 p-3 rounded-xl border {t.keyline} {dark ? 'bg-white/[0.02]' : 'bg-brand-50/30'}">
        <label class="group flex items-center gap-3 cursor-pointer">
          <div class="relative flex items-center justify-center">
            <input
              type="checkbox"
              checked={Boolean(captureRoleDraft?.includeInstructions)}
              onchange={() => onToggleCaptureRoleFlag('includeInstructions')}
              disabled={captureRoleDraft?.submitting}
              class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
              data-testid="mesh-capture-role-include-instructions"
            />
            <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          </div>
          <span class="text-xs font-medium {t.textSecondary} group-hover:text-brand-500 transition-colors">Include current instructions</span>
        </label>

        <label class="group flex items-center gap-3 cursor-pointer">
          <div class="relative flex items-center justify-center">
            <input
              type="checkbox"
              checked={Boolean(captureRoleDraft?.includeBehavioralContract)}
              onchange={() => onToggleCaptureRoleFlag('includeBehavioralContract')}
              disabled={captureRoleDraft?.submitting}
              class="peer appearance-none w-4 h-4 rounded border transition-all cursor-pointer {dark ? 'bg-zinc-900 border-white/[0.1] checked:bg-brand-500 checked:border-brand-500' : 'bg-white border-brand-300 checked:bg-brand-500 checked:border-brand-500'} focus:ring-2 focus:ring-brand-500/20"
              data-testid="mesh-capture-role-include-behavioral-contract"
            />
            <svg class="absolute w-2.5 h-2.5 text-white pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          </div>
          <span class="text-xs font-medium {t.textSecondary} group-hover:text-brand-500 transition-colors">Include behavioral contract</span>
        </label>
      </div>

      {#if captureRoleDraft?.error}
        <div class="p-2 rounded-lg bg-danger-500/10 border border-danger-500/20 animate-in fade-in zoom-in-95 duration-200">
          <p class="text-[11px] font-medium text-danger-500 text-center" data-testid="mesh-capture-role-error">{captureRoleDraft.error}</p>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-3 pt-4 border-t {t.keyline}">
        <button
          class="h-10 px-4 rounded-lg text-xs font-bold transition-all active:scale-95 {dark ? 'text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.05]' : 'text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100'}"
          type="button"
          onclick={onCloseCaptureRole}
          disabled={captureRoleDraft?.submitting}
          data-testid="mesh-capture-role-cancel"
        >
          Cancel
        </button>
        <button
          class="h-10 px-6 rounded-lg bg-brand-600 text-white text-xs font-bold hover:bg-brand-500 active:scale-95 shadow-lg shadow-brand-500/20 disabled:opacity-50 disabled:pointer-events-none transition-all"
          type="button"
          onclick={onSubmitCaptureRole}
          disabled={!canSaveCapturedRole}
          data-testid="mesh-capture-role-save"
        >
          {captureRoleDraft?.submitting ? 'Saving...' : 'Save to Catalog'}
        </button>
      </div>
    </section>
  {/snippet}
</SlideOver>
