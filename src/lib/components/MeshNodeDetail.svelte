<script>
  import { themeTokens } from '../themeTokens.js'
  import StatusBadge from './StatusBadge.svelte'

  let {
    node = {},
    mode = 'setup',
    dark = false,
    actions = {},
  } = $props()

  const name = $derived.by(() => String(node?.name ?? ''))
  const role = $derived.by(() => String(node?.role ?? 'agent'))
  const tool = $derived.by(() => String(node?.tool ?? 'claude'))
  const model = $derived.by(() => String(node?.model ?? ''))
  const status = $derived.by(() => String(node?.status ?? 'offline'))
  const projectId = $derived.by(() => String(node?.projectId ?? ''))
  const description = $derived.by(() => String(node?.description ?? ''))

  const t = $derived(themeTokens(dark))
  const normalizedRole = $derived(role === 'lead' ? 'lead' : 'agent')
  const normalizedMode = $derived(mode === 'runtime' ? 'runtime' : 'setup')
  const normalizedStatus = $derived.by(() => {
    const value = String(status || '').toLowerCase()
    if (value === 'active') return 'active'
    if (value === 'idle') return 'idle'
    return 'offline'
  })

  const toolLabel = $derived.by(() => {
    const value = String(tool || '').toLowerCase()
    if (value === 'claude') return 'Claude'
    if (value === 'codex') return 'Codex'
    if (value === 'gemini') return 'Gemini'
    return tool || 'Unknown'
  })

  const statusLabel = $derived.by(() => {
    if (normalizedStatus === 'active') return 'Active'
    if (normalizedStatus === 'idle') return 'Idle'
    return 'Offline'
  })

  const ghostButtonTone = $derived(
    dark
      ? 'text-zinc-300 hover:text-zinc-100 hover:bg-zinc-800/70'
      : 'text-brand-700 hover:text-brand-900 hover:bg-brand-100/70'
  )
  const dangerGhostTone = $derived(
    dark
      ? 'text-danger-300 hover:bg-danger-500/10'
      : 'text-danger-600 hover:bg-danger-50'
  )
  const closeTone = $derived(
    dark
      ? 'text-zinc-500 hover:text-zinc-200 hover:bg-zinc-800/70'
      : 'text-brand-700/70 hover:text-brand-900 hover:bg-brand-100/70'
  )
  const detailTitleTone = $derived(dark ? t.textPrimary : 'text-brand-900')
  const detailMutedTone = $derived(dark ? t.textMuted : 'text-brand-700')
  const detailBodyTone = $derived(dark ? t.textSecondary : 'text-brand-700')
  const detailKeyline = $derived(dark ? t.keyline : 'border-brand-200')
  const detailStyle = $derived.by(() => {
    if (dark) {
      return 'background: linear-gradient(180deg, var(--mesh-node-gradient-from), var(--mesh-node-gradient-to)); border: 1px solid var(--mesh-node-border); box-shadow: var(--mesh-node-shadow);'
    }
    return 'background: linear-gradient(180deg, #f0fdfa, #e6f7f4); border: 1px solid #b2d8d0; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);'
  })
</script>

<aside
  class="absolute left-1/2 top-full mt-3 -translate-x-1/2 z-20 w-[240px] rounded-xl p-3 mesh-node-detail animate-[mesh-detail-enter_160ms_ease-out]"
  style={detailStyle}
  data-testid="mesh-node-detail"
>
  <header class="flex items-start gap-2">
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-1.5 min-w-0">
        <h3 class="text-[14px] font-semibold truncate {detailTitleTone}" data-testid="mesh-node-detail-name">{name}</h3>
        <StatusBadge status={normalizedStatus} size="md" {dark} />
        <span class="text-[10px] {detailMutedTone}" data-testid="mesh-node-detail-status">{statusLabel}</span>
      </div>
      <p class="mt-1 text-[12px] truncate {detailMutedTone}" data-testid="mesh-node-detail-tool-model">
        {toolLabel} {model ? `· ${model}` : ''}
      </p>
    </div>
    <button
      class="h-6 w-6 shrink-0 rounded-md text-xs transition-colors {closeTone}"
      onclick={() => actions?.onClose?.()}
      aria-label="Close node detail"
      data-testid="mesh-node-detail-close"
    >
      ✕
    </button>
  </header>

  <p class="mt-1 text-[12px] truncate {detailMutedTone}" data-testid="mesh-node-detail-project">{projectId || 'n/a'}</p>

  {#if description && description.trim().length > 0}
    <p class="mt-2 text-[12px] leading-relaxed {detailBodyTone} line-clamp-3" data-testid="mesh-node-detail-description">
      {description}
    </p>
  {/if}

  <div class="mt-3 pt-2 border-t {detailKeyline}">
    <div class="flex items-center gap-1.5">
      {#if normalizedMode === 'setup'}
        <button
          class="text-xs px-2 py-1 rounded transition-colors {ghostButtonTone}"
          onclick={() => actions?.onEdit?.()}
          data-testid="mesh-node-detail-edit"
        >
          Edit
        </button>
        {#if normalizedRole !== 'lead'}
          <button
            class="text-xs px-2 py-1 rounded transition-colors {dangerGhostTone}"
            onclick={() => actions?.onRemove?.()}
            data-testid="mesh-node-detail-remove"
          >
            Remove
          </button>
        {/if}
      {:else}
        <button
          class="text-xs px-2 py-1 rounded transition-colors {ghostButtonTone}"
          onclick={() => actions?.onResume?.()}
          data-testid="mesh-node-detail-resume"
        >
          Resume
        </button>
        <button
          class="text-xs px-2 py-1 rounded transition-colors {dangerGhostTone}"
          onclick={() => actions?.onStop?.()}
          data-testid="mesh-node-detail-stop"
        >
          Stop
        </button>
        <button
          class="text-xs px-2 py-1 rounded transition-colors {ghostButtonTone}"
          onclick={() => actions?.onCapture?.()}
          data-testid="mesh-node-detail-capture"
        >
          Capture
        </button>
        <button
          class="text-xs px-2 py-1 rounded transition-colors {ghostButtonTone}"
          onclick={() => actions?.onFocusPane?.()}
          data-testid="mesh-node-detail-focus"
        >
          Focus ▶
        </button>
      {/if}
    </div>
  </div>
</aside>
