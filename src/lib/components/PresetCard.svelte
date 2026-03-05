<script>
  import { getToolIcon, getToolName } from '../toolLogos.js'

  let {
    name = '',
    description = '',
    leadCount = 1,
    agentCount = 0,
    tools = [],
    builtIn = false,
    onSelect = () => {},
    onInspect = () => {},
    dark = false,
    testId = '',
  } = $props()

  const surfaceTone = $derived(
    dark
      ? 'bg-white/[0.03] border-white/[0.06] hover:bg-white/[0.05] hover:border-brand-500/40'
      : 'bg-brand-50/50 border-brand-200/40 hover:bg-brand-50/80 hover:border-brand-500/40'
  )
  const titleTone = $derived(dark ? 'text-zinc-100' : 'text-brand-900')
  const mutedTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const badgeTone = $derived(
    dark ? 'bg-brand-500/10 border-brand-500/20 text-brand-400' : 'bg-brand-100 border-brand-200 text-brand-700'
  )
  const inspectTone = $derived(
    dark
      ? 'text-zinc-500 hover:text-zinc-200 hover:bg-zinc-700/50'
      : 'text-brand-700/60 hover:text-brand-900 hover:bg-brand-100/70'
  )

  const normalizedTools = $derived.by(() => {
    if (!Array.isArray(tools)) return []
    return [...new Set(tools.map((tool) => String(tool || '').toLowerCase()).filter(Boolean))]
  })
  const normalizedAgentCount = $derived(Math.max(0, Number(agentCount ?? 0)))
  const normalizedLeadCount = $derived(Math.max(0, Number(leadCount ?? 0)))
  const agentLabel = $derived(
    `${normalizedAgentCount} agent${normalizedAgentCount === 1 ? '' : 's'}`
  )
  const cardTestId = $derived(testId || `preset-card-${name}`)

</script>

<article
  class="relative group w-full rounded-xl border p-3 text-left transition-all duration-200 active:scale-[0.98] {surfaceTone}"
>
  <div class="flex items-start gap-2">
    <button
      class="flex-1 text-left outline-none"
      type="button"
      onclick={() => onSelect()}
      aria-label={`Select preset ${name}`}
      data-testid={cardTestId}
    >
      <h4 class="pr-1 text-[14px] font-bold leading-tight transition-colors group-hover:text-brand-500 {titleTone}">
        {name || 'Untitled preset'}
      </h4>
    </button>
    
    <div class="flex items-center gap-1.5 ml-auto">
      {#if builtIn}
        <span
          class="shrink-0 rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider {badgeTone}"
        >
          Built-in
        </span>
      {/if}
      
      <button
        class="shrink-0 h-7 w-7 flex items-center justify-center rounded-lg transition-all opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/30 {inspectTone}"
        type="button"
        onclick={(event) => {
          event.stopPropagation()
          onInspect()
        }}
        aria-label={`Inspect preset ${name}`}
        data-testid={`preset-card-inspect-${name}`}
      >
        <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/>
        </svg>
      </button>
    </div>
  </div>

  <button class="mt-3 w-full text-left outline-none" type="button" onclick={() => onSelect()}>
    <div class="flex items-center gap-2">
      <span class="rounded-lg border px-2 py-0.5 text-[10px] font-bold {badgeTone}">{agentLabel}</span>
      <span class="text-[10px] font-medium uppercase tracking-tight {mutedTone}">{normalizedLeadCount} lead</span>
    </div>

    {#if normalizedTools.length > 0}
      <div class="mt-3 flex items-center gap-2 text-zinc-500">
        {#each normalizedTools as tool}
          {@const icon = getToolIcon(tool)}
          <span
            class="inline-flex h-4 w-4 items-center justify-center rounded-md bg-black/5 dark:bg-white/5 border border-white/5"
            title={getToolName(tool)}
            data-testid={`preset-card-tool-${name}-${tool}`}
          >
            <svg class="h-3 w-3" viewBox={icon.viewBox} fill="currentColor" aria-hidden="true">
              <path d={icon.path}></path>
            </svg>
          </span>
        {/each}
      </div>
    {/if}

    <p class="mt-3 line-clamp-2 text-[11px] font-medium leading-relaxed {mutedTone}">
      {description || 'No description available.'}
    </p>
  </button>
</article>
