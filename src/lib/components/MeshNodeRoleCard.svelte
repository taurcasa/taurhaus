<script>
  import { activitySignal } from '../activitySignal.js'

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
  const roleDescription = $derived.by(() =>
    String(node?.instructions ?? node?.description ?? '').trim()
  )
  const signal = $derived(activitySignal(node))
  const toolLabel = $derived.by(() => {
    const value = String(node?.tool ?? node?.cliTool ?? node?.cli_tool ?? '').trim().toLowerCase()
    if (value === 'claude') return 'Claude'
    if (value === 'codex') return 'Codex'
    if (value === 'gemini') return 'Gemini'
    return value || 'Unknown'
  })
  const model = $derived.by(() => String(node?.model ?? node?.modelName ?? node?.model_name ?? '').trim())
  const statusLabel = $derived(signal.label)
  const statusTone = $derived.by(() => {
    if (signal.level === 'working' || signal.level === 'active') {
      return dark
        ? 'border-emerald-400/35 bg-emerald-500/14 text-emerald-100'
        : 'border-emerald-300/80 bg-emerald-50 text-emerald-800'
    }
    if (signal.level === 'idle') {
      return dark
        ? 'border-amber-400/35 bg-amber-500/14 text-amber-100'
        : 'border-amber-300/80 bg-amber-50 text-amber-800'
    }
    if (signal.level === 'uncertain') {
      return dark
        ? 'border-sky-400/35 bg-sky-500/14 text-sky-100'
        : 'border-sky-300/80 bg-sky-50 text-sky-800'
    }
    return dark
      ? 'border-white/10 bg-white/[0.04] text-zinc-300'
      : 'border-zinc-200 bg-zinc-50 text-zinc-700'
  })
  const compactSummary = $derived.by(() => focusArea || contextSummary || behaviorSummary || roleDescription)
  const hasRoleContent = $derived.by(() => compactSummary.length > 0)
  const placeholderTitle = $derived('No role defined')
  const placeholderMessage = $derived(
    'Assign a role template to show a compact focus summary here.'
  )
  const cardTone = $derived(
    dark
      ? 'border-white/12 bg-zinc-950/96 text-zinc-100 shadow-[0_16px_34px_rgba(0,0,0,0.48)]'
      : 'border-brand-200/90 bg-white/98 text-zinc-900 shadow-[0_14px_30px_rgba(15,23,42,0.14)]'
  )
  const labelTone = $derived(dark ? 'text-zinc-400' : 'text-zinc-500')
  const secondaryTone = $derived(dark ? 'text-zinc-300' : 'text-zinc-700')
  const hintTone = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
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
    <div class="min-w-0 space-y-1.5">
      <div class="space-y-0.5">
        <p class="text-[13px] font-semibold truncate" data-testid="mesh-node-role-card-name" title={name}>{name}</p>
        {#if roleName}
          <p class="text-[12px] font-medium truncate {secondaryTone}" data-testid="mesh-node-role-card-role-name" title={roleName}>{roleName}</p>
        {/if}
      </div>

      <div class="flex flex-wrap items-center gap-1.5">
        <p
          class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium {dark ? 'border-white/10 bg-white/[0.04] text-zinc-200' : 'border-brand-200 bg-brand-50/65 text-zinc-700'}"
          data-testid="mesh-node-role-card-tool-model"
          title={model ? `${toolLabel} · ${model}` : toolLabel}
        >
          <span class="inline-block h-1.5 w-1.5 rounded-full bg-brand-500" aria-hidden="true"></span>
          {toolLabel}{model ? ` · ${model}` : ''}
        </p>
        <p
          class="inline-flex items-center rounded-full border px-2.5 py-1 text-[11px] font-medium {statusTone}"
          data-testid="mesh-node-role-card-status"
        >
          {statusLabel}
        </p>
      </div>
    </div>

    {#if hasRoleContent}
      <div class="space-y-0.5 min-w-0">
        <p class="text-[10px] font-semibold uppercase tracking-wide {labelTone}">Focus</p>
        <p
          class="text-[11px] leading-snug line-clamp-2 {secondaryTone}"
          data-testid="mesh-node-role-card-summary"
          title={compactSummary}
        >
          {compactSummary}
        </p>
      </div>
    {:else}
      <div
        class="space-y-1 rounded-xl border px-3 py-2.5 {dark ? 'border-white/10 bg-white/[0.04]' : 'border-brand-200/80 bg-brand-50/60'}"
        data-testid="mesh-node-role-card-placeholder"
      >
        <p class="text-[12px] font-medium {secondaryTone}" data-testid="mesh-node-role-card-placeholder-title">{placeholderTitle}</p>
        <p class="text-[11px] leading-relaxed {labelTone}" data-testid="mesh-node-role-card-placeholder-message">{placeholderMessage}</p>
      </div>
    {/if}

    <p
      class="text-[10px] font-medium uppercase tracking-[0.14em] {hintTone}"
      data-testid="mesh-node-role-card-hint"
    >
      Click for details
    </p>
  </div>
</aside>
