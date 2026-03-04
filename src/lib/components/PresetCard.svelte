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
      ? 'bg-zinc-800/50 border-zinc-700/50 hover:border-brand-500/40'
      : 'bg-linear-to-b from-brand-50 to-[#e6f7f4] border-[#b2d8d0] hover:border-[#8ec5ba] shadow-[0_2px_8px_rgba(0,0,0,0.08)] hover:shadow-[0_4px_14px_rgba(0,0,0,0.12)]'
  )
  const titleTone = $derived(dark ? 'text-zinc-100' : 'text-brand-900')
  const mutedTone = $derived(dark ? 'text-zinc-400' : 'text-brand-700')
  const badgeTone = $derived(
    dark ? 'bg-zinc-700/60 text-zinc-200' : 'bg-brand-100 text-brand-700'
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
  class="relative w-full rounded-[12px] border p-2.5 text-left transition-colors {surfaceTone}"
>
  <div class="flex items-start gap-1.5">
    <button
      class="flex-1 text-left"
      type="button"
      onclick={() => onSelect()}
      aria-label={`Select preset ${name}`}
      data-testid={cardTestId}
    >
      <h4 class="pr-1 text-[13px] font-semibold leading-tight {titleTone}">
        {name || 'Untitled preset'}
      </h4>
    </button>
    {#if builtIn}
      <span
        class="shrink-0 rounded-full px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide {badgeTone}"
      >
        Built-in
      </span>
    {/if}
    <button
      class="ml-auto shrink-0 rounded p-1 transition-colors {inspectTone}"
      type="button"
      onclick={(event) => {
        event.stopPropagation()
        onInspect()
      }}
      aria-label={`Inspect preset ${name}`}
      data-testid={`preset-card-inspect-${name}`}
    >
      <svg class="h-3 w-3" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M2 8c1.4-2.3 3.3-3.5 6-3.5S12.6 5.7 14 8c-1.4 2.3-3.3 3.5-6 3.5S3.4 10.3 2 8Z"
          stroke="currentColor"
          stroke-width="1.2"
        />
        <circle cx="8" cy="8" r="1.6" fill="currentColor" />
      </svg>
    </button>
  </div>

  <button class="mt-2 w-full text-left" type="button" onclick={() => onSelect()}>
    <div class="flex items-center gap-2">
      <span class="rounded-full px-1.5 py-0.5 text-[10px] font-medium {badgeTone}">{agentLabel}</span>
      <span class="text-[10px] {mutedTone}">{normalizedLeadCount} lead</span>
    </div>

    {#if normalizedTools.length > 0}
      <div class="mt-2 flex items-center gap-1.5 text-zinc-500">
        {#each normalizedTools as tool}
          {@const icon = getToolIcon(tool)}
          <span
            class="inline-flex h-3 w-3 items-center justify-center"
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

    <p class="mt-2 line-clamp-2 text-[11px] leading-4 {mutedTone}">
      {description || 'No description available.'}
    </p>
  </button>
</article>
