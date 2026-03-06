<script>
  import HoverCard from '../../lib/HoverCard.svelte'

  let { scenario, theme = 'light' } = $props()
  let anchorEl = $state(null)

  const dark = $derived(theme === 'dark')
</script>

<div class="relative min-h-[420px] w-full">
  <button
    bind:this={anchorEl}
    type="button"
    class="absolute left-16 top-20 rounded-full border border-brand-400/30 bg-brand-500/15 px-4 py-2 text-sm font-medium text-brand-800 dark:text-brand-100"
  >
    Hover anchor
  </button>

  <HoverCard
    project={scenario?.project ?? null}
    sessions={scenario?.sessions ?? []}
    {anchorEl}
    {dark}
    visible={true}
  />
</div>
