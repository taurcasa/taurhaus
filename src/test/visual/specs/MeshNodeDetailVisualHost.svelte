<script>
  import '../../../app.css'
  import MeshNodeDetail from '../../../lib/components/MeshNodeDetail.svelte'

  let { scenario } = $props()

  const dark = $derived(scenario?.theme === 'dark')
  const mode = $derived(String(scenario?.mode || 'runtime'))
  const member = $derived(scenario?.member ?? {})
  const anchor = $derived({
    left: 72,
    top: 44,
    cardWidth: 336,
    placement: 'bottom',
  })
  const node = $derived.by(() => ({
    name: String(member?.name ?? '').trim(),
    role: member?.role === 'lead' ? 'lead' : 'agent',
    tool: String(member?.tool ?? member?.toolLabel ?? 'claude').toLowerCase(),
    model: String(member?.model ?? ''),
    status: String(member?.status ?? 'offline'),
    projectId: String(member?.projectId ?? ''),
    description: String(member?.description ?? ''),
    paneId: String(member?.paneId ?? ''),
    sessionId: String(member?.sessionId ?? ''),
    sessionState: String(
      member?.sessionState
      ?? member?.sessionTiming?.activeLabel
      ?? member?.sessionTiming?.startedLabel
      ?? ''
    ),
  }))
  const actions = $derived.by(() => {
    const shared = {
      onClose: () => {},
    }
    if (mode === 'setup') {
      return {
        ...shared,
        onEdit: () => {},
        onRemove: () => {},
      }
    }
    return {
      ...shared,
      onResume: () => {},
      onStop: () => {},
      onCapture: () => {},
      onFocusPane: scenario?.focusEnabled ? () => {} : null,
    }
  })
</script>

<main class="mesh-node-detail-visual-stage" data-testid="mesh-node-detail-visual-stage">
  <div class="mesh-node-detail-visual-frame" data-testid="mesh-node-detail-visual-frame">
    <MeshNodeDetail {node} {mode} {dark} {anchor} {actions} />
  </div>
</main>

<style>
  .mesh-node-detail-visual-stage {
    min-height: 100vh;
    width: 100%;
    display: grid;
    place-items: center;
    padding: 32px;
    box-sizing: border-box;
  }

  .mesh-node-detail-visual-frame {
    position: relative;
    width: 480px;
    height: 420px;
    max-width: 100%;
  }
</style>
