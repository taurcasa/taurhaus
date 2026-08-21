<script>
  import ModelSelect from '../../lib/components/ModelSelect.svelte'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const cases = $derived(scenario?.cases ?? [])
  const catalog = $derived(scenario?.catalog ?? null)
  const labelTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
</script>

<div class="grid w-full max-w-3xl gap-4 p-6 sm:grid-cols-2">
  {#each cases as entry (entry.label)}
    <div class="space-y-1.5">
      <span class="block text-[10px] font-medium uppercase tracking-wide {labelTone}">
        {entry.label}
      </span>
      <ModelSelect
        tool={entry.tool}
        model={entry.model}
        reasoningEffort={entry.reasoningEffort}
        {catalog}
        {dark}
        compact={Boolean(entry.compact)}
        disabled={Boolean(entry.disabled)}
        testId={`model-select-${entry.label}`}
      />
    </div>
  {/each}
</div>
