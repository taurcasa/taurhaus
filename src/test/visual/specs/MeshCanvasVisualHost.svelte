<script>
  import '../../../app.css'
  import MeshCanvas from '../../../lib/components/MeshCanvas.svelte'

  let { scenario } = $props()

  const members = $derived(Array.isArray(scenario?.members) ? scenario.members : [])
  const lead = $derived(members.find((member) => member.role === 'lead') ?? null)
  const agents = $derived(members.filter((member) => member.role !== 'lead'))
  const canvasWidth = $derived(Number(scenario?.canvasSize?.width ?? 900))
  const canvasHeight = $derived(Number(scenario?.canvasSize?.height ?? 520))
  const dark = $derived(scenario?.theme === 'dark')
  const mode = $derived(String(scenario?.mode || 'runtime'))
</script>

<main class="mesh-canvas-visual-stage" data-testid="mesh-canvas-visual-stage">
  <div
    class="mesh-canvas-visual-frame"
    style={`width: ${canvasWidth}px; height: ${canvasHeight}px;`}
  >
    <MeshCanvas {lead} {agents} {dark} {mode} />
  </div>
</main>

<style>
  .mesh-canvas-visual-stage {
    min-height: 100vh;
    width: 100%;
    display: grid;
    place-items: center;
    padding: 32px;
    box-sizing: border-box;
  }

  .mesh-canvas-visual-frame {
    max-width: 100%;
  }
</style>
