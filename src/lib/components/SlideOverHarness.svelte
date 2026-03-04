<script>
  import SlideOver from './SlideOver.svelte'

  let {
    open = true,
    title = 'Panel title',
    width = 420,
    onClose = () => {},
    dark = false,
  } = $props()
</script>

<div class="relative h-[400px]" data-testid="slideover-harness">
  <SlideOver {open} {title} {width} {onClose} {dark}>
    {#snippet children()}
      <p data-testid="slideover-child">Child content</p>
      <button data-testid="slideover-focus-1">Action one</button>
      <button data-testid="slideover-focus-2">Action two</button>
    {/snippet}
  </SlideOver>
</div>
