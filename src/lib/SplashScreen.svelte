<script>
  import { isTauri, getDaemonStatus } from './ipc.js'

  let { onComplete = () => {} } = $props()

  // Animation phase: 0=hidden, 1=foundation (feet), 2=walls, 3=crown (horns)
  // Each phase is driven by actual backend state, not timers.
  let phase = $state(0)
  let status = $state('')
  let completed = $state(false)
  let daemonStatus = $state(null)

  let splashStart = Date.now()
  const MIN_DISPLAY_MS_FAST = 180
  const MIN_DISPLAY_MS_SLOW = 420
  const READY_BEAT_MS_FAST = 80
  const READY_BEAT_MS_SLOW = 180

  // Clip-path inset from top — reveals image bottom-to-top
  const clips = {
    0: 'inset(100% 0 0 0)',
    1: 'inset(68% 0 0 0)',
    2: 'inset(32% 0 0 0)',
    3: 'inset(0% 0 0 0)',
  }

  const clipPath = $derived(clips[phase] ?? clips[0])

  // Transition speed adapts: fast when daemon is already ready, slow when waiting
  let fastPath = $state(false)
  const transitionMs = $derived(fastPath ? 250 : 500)

  function advancePhase(newPhase, newStatus) {
    if (newPhase <= phase) return
    phase = newPhase
    status = newStatus
  }

  function completeAfterHold() {
    if (completed) return
    advancePhase(3, 'Ready')

    // Warm path keeps the beat short; cold path still gets a visible reveal.
    const elapsed = Date.now() - splashStart
    const minDisplay = fastPath ? MIN_DISPLAY_MS_FAST : MIN_DISPLAY_MS_SLOW
    const readyBeat = fastPath ? READY_BEAT_MS_FAST : READY_BEAT_MS_SLOW
    const remaining = Math.max(0, minDisplay - elapsed)
    const holdDelay = remaining + readyBeat

    setTimeout(() => {
      completed = true
      onComplete({ daemonStatus })
    }, holdDelay)
  }

  // State-driven boot: sample daemon status, then finish splash.
  $effect(() => {
    let cancelled = false

    // Phase 1: we're querying the backend
    advancePhase(1, 'Checking daemon...')

    getDaemonStatus()
      .then((result) => {
        if (cancelled) return
        const s = result?.status
        daemonStatus = s ?? 'disconnected'

        if (s === 'connected') {
          fastPath = true
          advancePhase(2, 'Connected')
          setTimeout(() => {
            if (!cancelled) completeAfterHold()
          }, 80)
          return
        }

        if (s === 'not_configured') {
          fastPath = true
          advancePhase(2, 'Loading...')
          setTimeout(() => {
            if (!cancelled) completeAfterHold()
          }, 80)
          return
        }

        advancePhase(2, 'Loading shell...')
        setTimeout(() => {
          if (!cancelled) completeAfterHold()
        }, 140)
      })
      .catch(() => {
        if (cancelled) return
        daemonStatus = isTauri() ? 'disconnected' : 'not_configured'
        fastPath = true
        advancePhase(2, 'Loading...')
        completeAfterHold()
      })

    return () => {
      cancelled = true
    }
  })

  const reducedMotion = typeof window !== 'undefined'
    ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
    : false
</script>

<div
  class="h-full bg-brand-950 flex flex-col items-center justify-center font-sans antialiased select-none"
  data-tauri-drag-region
  role="status"
  aria-live="polite"
  aria-label="Application loading"
>
  <!-- Logo with clip-path reveal -->
  <div class="relative" data-tauri-drag-region>
    <img
      src="/logo-200.png"
      alt="taurhaus logo"
      width="100"
      height="100"
      class="block"
      style:clip-path={clipPath}
      style:opacity={phase === 3 ? 1 : phase > 0 ? 0.85 : 0}
      style:transform={completed ? 'scale(1.02)' : 'scale(1)'}
      style:transition="clip-path {reducedMotion ? '0ms' : transitionMs + 'ms'} ease-out, opacity {reducedMotion ? '0ms' : '300ms'} ease-out, transform {reducedMotion ? '0ms' : '200ms'} ease-out"
    />
  </div>

  <!-- Wordmark -->
  <p
    class="mt-5 text-[18px] font-semibold text-white/90 tracking-[-0.02em]"
    class:opacity-0={phase === 0}
    class:opacity-100={phase > 0}
    style="transition: opacity {reducedMotion ? '0ms' : '400ms'} ease-out;"
    data-tauri-drag-region
  >
    taurhaus
  </p>

  <!-- Status text / Error state -->
  <div class="mt-3 h-8 flex flex-col items-center justify-start" data-tauri-drag-region>
    {#if status}
      <p
        class="text-[12px] text-white/30"
        class:opacity-0={phase === 0}
        class:opacity-100={phase > 0}
        style="transition: opacity {reducedMotion ? '0ms' : '300ms'} ease-out;"
      >{status}</p>
    {/if}
  </div>
</div>
