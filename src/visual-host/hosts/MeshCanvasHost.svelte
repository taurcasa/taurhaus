<script>
  import MeshCanvas from '../../lib/components/MeshCanvas.svelte'

  let { scenario, theme = 'light' } = $props()

  const members = $derived(Array.isArray(scenario?.members) ? scenario.members : [])
  const lead = $derived(members.find((member) => member.role === 'lead') ?? null)
  const agents = $derived(members.filter((member) => member.role !== 'lead'))
  const canvasWidth = $derived(Number(scenario?.canvasSize?.width ?? 900))
  const canvasHeight = $derived(Number(scenario?.canvasSize?.height ?? 520))
  const dark = $derived(theme === 'dark')
  const mode = $derived(String(scenario?.mode || 'runtime'))
</script>

<div class="mx-auto max-w-full" style={`width: ${canvasWidth}px; height: ${canvasHeight}px;`}>
  <MeshCanvas {lead} {agents} {dark} {mode} />
</div>
