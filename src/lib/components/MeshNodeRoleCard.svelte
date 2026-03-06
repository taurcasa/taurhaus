<script>
  let {
    node = {},
    dark = false,
    anchor = null,
  } = $props()

  const name = $derived.by(() => String(node?.name ?? '').trim() || 'unnamed-agent')
  const roleName = $derived.by(() => String(node?.roleName ?? node?.role_name ?? '').trim())
  const focusArea = $derived.by(() => String(node?.focusArea ?? node?.focus_area ?? '').trim())
  const contextSummary = $derived.by(() => String(node?.contextSummary ?? node?.context_summary ?? '').trim())
  const behaviorSummary = $derived.by(() => String(node?.behaviorSummary ?? node?.behavior_summary ?? '').trim())
  const toolLabel = $derived.by(() => {
    const value = String(node?.tool ?? node?.cliTool ?? node?.cli_tool ?? '').trim().toLowerCase()
    if (value === 'claude') return 'Claude'
    if (value === 'codex') return 'Codex'
    if (value === 'gemini') return 'Gemini'
    return value || 'Unknown'
  })
  const model = $derived.by(() => String(node?.model ?? node?.modelName ?? node?.model_name ?? '').trim())
  const cardTone = $derived(
    dark
      ? 'border-white/12 bg-zinc-950/96 text-zinc-100 shadow-[0_16px_34px_rgba(0,0,0,0.48)]'
      : 'border-brand-200/90 bg-white/98 text-zinc-900 shadow-[0_14px_30px_rgba(15,23,42,0.14)]'
  )
  const labelTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const secondaryTone = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const anchoredStyle = $derived.by(() => {
    if (!anchor || typeof anchor !== 'object') return ''
    const left = Number(anchor.left)
    const top = Number(anchor.top)
    const cardWidth = Number(anchor.cardWidth)
    if (!Number.isFinite(left) || !Number.isFinite(top)) return ''
    return [
      `left: ${left}px`,
      `top: ${top}px`,
      `width: ${Number.isFinite(cardWidth) ? Math.max(192, cardWidth) : 224}px`,
      'max-width: calc(100% - 16px)',
    ].join('; ')
  })
</script>

<aside
  class="absolute rounded-2xl border px-3.5 py-3 backdrop-blur-sm mesh-node-role-card animate-[mesh-detail-enter_160ms_cubic-bezier(0.22,1,0.36,1)] {cardTone}"
  style={anchoredStyle}
  data-testid="mesh-node-role-card"
  data-placement={anchor?.placement === 'top' ? 'top' : 'bottom'}
>
  <div class="space-y-2">
    <div class="space-y-0.5 min-w-0">
      <p class="text-[13px] font-semibold truncate" data-testid="mesh-node-role-card-name" title={name}>{name}</p>
      {#if roleName}
        <p class="text-[12px] font-medium {secondaryTone}" data-testid="mesh-node-role-card-role-name">{roleName}</p>
      {/if}
    </div>

    {#if focusArea}
      <div class="space-y-0.5">
        <p class="text-[10px] font-semibold uppercase tracking-wide {labelTone}">Focus</p>
        <p class="text-[12px] leading-snug {secondaryTone}" data-testid="mesh-node-role-card-focus">{focusArea}</p>
      </div>
    {/if}

    {#if contextSummary}
      <p class="text-[11px] leading-relaxed {secondaryTone}" data-testid="mesh-node-role-card-context">{contextSummary}</p>
    {/if}

    {#if behaviorSummary}
      <p class="text-[11px] leading-relaxed {secondaryTone}" data-testid="mesh-node-role-card-behavior">{behaviorSummary}</p>
    {/if}

    <p
      class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium {dark ? 'border-white/10 bg-white/[0.04] text-zinc-200' : 'border-brand-200 bg-brand-50/65 text-zinc-700'}"
      data-testid="mesh-node-role-card-tool-model"
      title={model ? `${toolLabel} · ${model}` : toolLabel}
    >
      <span class="inline-block h-1.5 w-1.5 rounded-full bg-brand-500" aria-hidden="true"></span>
      {toolLabel}{model ? ` · ${model}` : ''}
    </p>
  </div>
</aside>
