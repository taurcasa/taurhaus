<script>
  import SlideOver from './SlideOver.svelte'
  import TeamComposer from './TeamComposer.svelte'

  let {
    open = false,
    dark = false,
    projectPath = '',
    availableProjects = [],
    teamConfig = null,
    context = null,
    onClose = () => {},
    onSave = () => {},
    onReset = () => {},
  } = $props()

  function inferProjectName(path) {
    const segments = String(path || '')
      .replace(/\\/g, '/')
      .split('/')
      .filter(Boolean)
    return segments.at(-1) ?? 'project'
  }

  const projectName = $derived(inferProjectName(projectPath))
  const initialPreset = $derived.by(() => {
    if (teamConfig?.composition?.presetId || teamConfig?.composition?.leadRoleId) {
      return {
        ...teamConfig.composition,
      }
    }
    return null
  })

  function handleSave(payload) {
    onSave(payload)
  }
</script>

<SlideOver
  {open}
  title="Customize Team"
  width={460}
  {dark}
  onClose={onClose}
>
  {#snippet children()}
    <div data-testid="team-customizer-panel">
      <TeamComposer
        {dark}
        {projectPath}
        {projectName}
        availableTools={['claude', 'codex', 'gemini']}
        {initialPreset}
        onApply={handleSave}
        onSavePreset={() => {}}
        onClose={onClose}
      />

      {#if context?.selectedRole}
        <p class="mt-2 text-xs {dark ? 'text-zinc-400' : 'text-zinc-600'}" data-testid="team-customizer-selected-role">
          Selected role from catalog: {context.selectedRole.name || context.selectedRole.roleId}
        </p>
      {/if}

      <div class="mt-3 flex justify-end">
        <button
          class="rounded-md border px-2 py-1 text-xs transition-colors {dark ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800' : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'}"
          onclick={onReset}
          data-testid="team-customizer-reset"
        >
          Reset to Empty
        </button>
      </div>
    </div>
  {/snippet}
</SlideOver>
