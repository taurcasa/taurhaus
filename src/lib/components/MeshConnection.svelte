<script>
  let {
    from = { x: 0, y: 0 },
    to = { x: 0, y: 0 },
    status = 'setup',
    dark = false,
    nodeHeight = 64,
  } = $props()

  const normalizedStatus = $derived.by(() => {
    const value = String(status || '').toLowerCase()
    if (value === 'initializing') return 'initializing'
    if (value === 'active') return 'active'
    if (value === 'idle') return 'idle'
    if (value === 'offline') return 'offline'
    return 'setup'
  })

  const path = $derived.by(() => {
    const fromX = Number(from?.x ?? 0)
    const fromY = Number(from?.y ?? 0)
    const toX = Number(to?.x ?? 0)
    const toY = Number(to?.y ?? 0)
    const halfHeight = Number(nodeHeight) / 2
    const midY = (fromY + toY) / 2

    return `M ${fromX},${fromY + halfHeight} C ${fromX},${midY} ${toX},${midY} ${toX},${toY - halfHeight}`
  })

  const strokeColor = $derived.by(() => {
    if (normalizedStatus === 'active') return 'var(--mesh-connection-active)'
    if (normalizedStatus === 'idle' || normalizedStatus === 'offline') {
      return 'var(--mesh-connection-color-dim)'
    }
    if (!dark) return 'rgba(13, 148, 136, 0.35)'
    return 'var(--mesh-connection-color)'
  })

  const pathStyle = $derived.by(() => {
    const styles = [
      `stroke: ${strokeColor}`,
      'stroke-width: var(--mesh-connection-width)',
      'fill: none',
      'transition: stroke 150ms ease-out, opacity 150ms ease-out',
    ]

    if (normalizedStatus === 'setup') {
      styles.push('stroke-dasharray: 6,4')
    }

    if (normalizedStatus === 'initializing') {
      styles.push('stroke-dasharray: 220')
      styles.push('stroke-dashoffset: 220')
      styles.push('animation: mesh-draw 900ms ease-out forwards')
    }

    if (normalizedStatus === 'active') {
      styles.push('animation: mesh-connection-breathe 2s ease-in-out infinite')
    }

    if (normalizedStatus === 'offline') {
      styles.push('stroke-dasharray: 6,4')
      styles.push('opacity: 0.45')
    }

    return styles.join('; ')
  })
</script>

<path
  class={`mesh-connection mesh-connection-${normalizedStatus}`}
  data-testid="mesh-connection"
  d={path}
  style={pathStyle}
></path>
