<script>
  import MeshNodeDetail from '../../lib/components/MeshNodeDetail.svelte'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const actions = $derived({
    onEdit: () => {},
    onRemove: () => {},
    onResume: () => {},
    onStop: () => {},
    onCapture: () => {},
    onFocusPane: () => {},
    onClose: () => {},
  })
</script>

<div class="relative min-h-[420px] w-full">
  <MeshNodeDetail
    node={scenario?.node ?? {}}
    mode={scenario?.mode ?? 'setup'}
    anchor={scenario?.anchor ?? null}
    {dark}
    {actions}
  />
</div>
