<script>
  let {
    status = 'offline',
    size = 'sm',
    dark = false,
  } = $props()

  const normalizedStatus = $derived.by(() => {
    const value = String(status || '').toLowerCase()
    if (value === 'active') return 'active'
    if (value === 'idle') return 'idle'
    return 'offline'
  })

  const sizeClass = $derived(size === 'md' ? 'h-2 w-2' : 'h-1.5 w-1.5')
  const sizeStyle = $derived(size === 'md' ? 'width: 10px; height: 10px;' : 'width: 8px; height: 8px;')
  const toneClass = $derived.by(() => {
    if (normalizedStatus === 'active') return 'bg-success-400'
    if (normalizedStatus === 'idle') return 'bg-warning-400'
    return 'bg-zinc-500'
  })
  const isActive = $derived(normalizedStatus === 'active')
  const glowStyle = $derived(isActive
    ? `box-shadow: ${dark ? '0 0 6px rgba(74,222,128,0.4)' : '0 0 3px rgba(74,222,128,0.18)'};`
    : '')
</script>

<span
  class="inline-block rounded-full {sizeClass} {toneClass} {isActive ? 'status-badge-active activepulse' : ''}"
  style={`${sizeStyle} ${glowStyle}`}
  aria-label={normalizedStatus}
  data-testid={`status-badge-${normalizedStatus}`}
></span>

<style>
  .activepulse {
    animation: activepulse 2s ease-in-out infinite;
  }

  @keyframes activepulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.7;
    }
  }
</style>
