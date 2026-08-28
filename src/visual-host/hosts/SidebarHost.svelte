<script>
  import { onMount, tick } from 'svelte'
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

  onMount(async () => {
    if (!scenario?.openContextMenu) return
    await tick()
    document.querySelector('[data-testid="project-item"]')?.dispatchEvent(
      new MouseEvent('contextmenu', { bubbles: true, clientX: 280, clientY: 180 })
    )
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
