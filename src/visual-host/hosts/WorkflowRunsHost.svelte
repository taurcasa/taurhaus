<script>
  import WorkflowRunsPanel from '../../lib/components/WorkflowRunsPanel.svelte'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const projectId = $derived(String(scenario?.projectId ?? ''))
  const sessions = $derived(Array.isArray(scenario?.sessions) ? scenario.sessions : [])
</script>

<div class="mx-auto w-full max-w-3xl px-7">
  <WorkflowRunsPanel {projectId} {sessions} {dark} />
</div>
