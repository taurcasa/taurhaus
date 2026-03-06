<script>
  import Sidebar from '../../lib/Sidebar.svelte'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const actions = $derived({
    onSelectProject: () => {},
    onAddProject: () => {},
    onToggleSettings: () => {},
    onRetry: () => {},
    onProjectRemoved: () => {},
  })
</script>

<div class="w-[320px] max-w-full">
  <Sidebar
    projects={scenario?.projects ?? []}
    selectedProject={scenario?.selectedProject ?? null}
    daemonStatus={scenario?.daemonStatus ?? null}
    settingsOpen={false}
    {dark}
    {actions}
  />
</div>
