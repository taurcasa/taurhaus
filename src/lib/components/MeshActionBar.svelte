<script>
  let {
    canInitialize = false,
    teamName = '',
    dark = false,
    onInitialize = () => {},
    onOpenCustomizer = () => {},
    onReset = () => {},
  } = $props()

  const borderTone = $derived(dark ? 'border-zinc-700/70' : 'border-zinc-200')
  const nameTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const neutralGhost = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800'
      : 'border-zinc-300 text-zinc-700 hover:bg-zinc-100'
  )
</script>

<footer
  class="flex h-12 items-center justify-between gap-2 border-t px-3 {borderTone}"
  data-testid="mesh-action-bar"
>
  <span class="truncate text-xs {nameTone}" data-testid="mesh-action-team-name">
    {teamName.trim() || 'Untitled Team'}
  </span>

  <div class="flex items-center gap-1.5">
    <button
      class="h-8 rounded-md border px-2.5 text-xs transition-colors {neutralGhost}"
      type="button"
      onclick={() => onOpenCustomizer()}
      data-testid="mesh-action-customize"
    >
      Customize
    </button>
    <button
      class="h-8 rounded-md bg-brand-600 px-3 text-xs font-medium text-white transition-colors hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
      type="button"
      onclick={() => onInitialize()}
      disabled={!canInitialize}
      data-testid="mesh-action-initialize"
    >
      Initialize ▶
    </button>
    <button
      class="h-8 rounded-md border px-2.5 text-xs transition-colors {neutralGhost}"
      type="button"
      onclick={() => onReset()}
      data-testid="mesh-action-reset"
    >
      Reset
    </button>
  </div>
</footer>
