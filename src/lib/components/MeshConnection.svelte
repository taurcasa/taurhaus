<script>
  let {
    from = { x: 0, y: 0 },
    to = { x: 0, y: 0 },
    status = 'setup',
    dark = false,
    nodeHeight = 64,
    delay = 0,
    duration = 400,
    glowFilterId = '',
  } = $props()

  let pathElement = $state(null)
  let pathLength = $state(220)
  let initStage = $state('settled')

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

  const initDelay = $derived.by(() => {
    const value = Number(delay ?? 0)
    return Number.isFinite(value) && value > 0 ? value : 0
  })

  const initDuration = $derived.by(() => {
    const value = Number(duration ?? 400)
    return Number.isFinite(value) && value > 0 ? value : 400
  })

  const strokeColor = $derived.by(() => {
    if (normalizedStatus === 'active') return 'var(--mesh-connection-active)'
    if (normalizedStatus === 'idle' || normalizedStatus === 'offline') {
      return 'var(--mesh-connection-color-dim)'
    }
    if (!dark) return 'rgba(13, 148, 136, 0.55)'
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
      if (initStage === 'drawing') {
        styles.push(`stroke-dasharray: ${pathLength}`)
        styles.push(`stroke-dashoffset: ${pathLength}`)
        styles.push(`animation: mesh-draw ${initDuration}ms ease-out ${initDelay}ms forwards`)
      } else if (initStage === 'pulse') {
        styles.push('animation: mesh-established-pulse 260ms ease-out 1')
      }
    }

    if (normalizedStatus === 'active') {
      styles.push('animation: mesh-connection-breathe 2s ease-in-out infinite')
    }

    if (normalizedStatus === 'idle') {
      styles.push('opacity: 0.6')
    }

    if (normalizedStatus === 'offline') {
      styles.push('stroke-dasharray: 6,4')
      styles.push('opacity: 0.28')
    }

    if (dark && glowFilterId) {
      styles.push(`filter: url(#${glowFilterId})`)
    }

    return styles.join('; ')
  })

  $effect(() => {
    const currentPath = path
    const currentElement = pathElement
    if (!currentPath || !currentElement) return

    try {
      const measured = currentElement.getTotalLength()
      if (Number.isFinite(measured) && measured > 0) {
        pathLength = measured
      }
    } catch {
      pathLength = 220
    }
  })

  $effect(() => {
    if (normalizedStatus !== 'initializing') {
      initStage = 'settled'
      return
    }

    initStage = 'drawing'

    const pulseDelay = initDelay + initDuration
    const pulseTimer = setTimeout(() => {
      initStage = 'pulse'
    }, pulseDelay)
    const settleTimer = setTimeout(() => {
      initStage = 'settled'
    }, pulseDelay + 260)

    return () => {
      clearTimeout(pulseTimer)
      clearTimeout(settleTimer)
    }
  })
</script>

<path
  bind:this={pathElement}
  class={`mesh-connection mesh-connection-${normalizedStatus}`}
  data-testid="mesh-connection"
  data-init-stage={initStage}
  d={path}
  style={pathStyle}
></path>
