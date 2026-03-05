<script>
  import DirectoryBrowser from './DirectoryBrowser.svelte'

  let {
    dark = false,
    t,
    inputBg = '',
    createProjectName = '',
    createParentDir = '',
    createError = null,
    creating = false,
    canCreate = false,
    onCreateNameInput = () => {},
    onCreateParentInput = () => {},
    onCreateEnter = () => {},
    onCreateParentSelect = () => {},
    onCreateProject = () => {},
    onBackToScan = () => {},
  } = $props()
</script>

<div>
  <label for="create-project-name" class="text-[13px] {t.textSecondary} mb-1.5 block">Project name</label>
  <input
    id="create-project-name"
    type="text"
    placeholder="my-new-project"
    value={createProjectName}
    class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
    oninput={(event) => onCreateNameInput(event.currentTarget.value)}
    onkeydown={(event) => event.key === 'Enter' && onCreateEnter()}
    data-testid="create-name-input"
  />

  <label for="create-parent-dir" class="text-[13px] {t.textSecondary} mb-1.5 mt-3 block">Parent directory</label>
  <input
    id="create-parent-dir"
    type="text"
    placeholder="~/projects"
    value={createParentDir}
    class="w-full px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
    oninput={(event) => onCreateParentInput(event.currentTarget.value)}
    onkeydown={(event) => event.key === 'Enter' && onCreateEnter()}
    data-testid="create-parent-input"
  />

  <div class="mt-3">
    <DirectoryBrowser
      {dark}
      selectedPath={createParentDir}
      onSelect={onCreateParentSelect}
      maxHeight="180px"
    />
  </div>

  {#if createError}
    <p class="text-[12px] text-danger-500 mt-2" data-testid="create-error">{createError}</p>
  {/if}

  <div class="flex items-center justify-between mt-3">
    <button
      class="text-[12px] {t.linkColor} transition-colors"
      onclick={onBackToScan}
    >Back to scan</button>
    <button
      class="px-3 py-1.5 rounded-md bg-brand-600 text-white text-[12px] font-medium hover:bg-brand-700 transition-colors disabled:opacity-50"
      onclick={onCreateProject}
      disabled={!canCreate || creating}
      data-testid="create-project-button"
    >{creating ? 'Creating...' : 'Create project'}</button>
  </div>
</div>
