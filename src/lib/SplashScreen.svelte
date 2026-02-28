<script>
  import { isTauri, getDaemonStatus } from './ipc.js'

  let { onComplete = () => {}, onRetry = () => {}, onContinue = () => {} } = $props()

  // Animation phase: 0=hidden, 1=foundation (feet), 2=walls, 3=crown (horns)
  // Each phase is driven by actual backend state, not timers.
  let phase = $state(0)
  let status = $state('')
  let error = $state(null)
  let completed = $state(false)

  let splashStart = Date.now()
  const MIN_DISPLAY_MS = 800

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

    // Enforce minimum display, then hold for the "built" beat
    const elapsed = Date.now() - splashStart
    const remaining = Math.max(0, MIN_DISPLAY_MS - elapsed)
    const holdDelay = remaining + 300

    setTimeout(() => {
      completed = true
      onComplete()
    }, holdDelay)
  }

  function showError(msg) {
    error = msg
    status = ''
  }

  // State-driven boot: query backend, then listen for events
  $effect(() => {
    let cleanupListener = null
    let timeoutId = null

    // Phase 1: we're querying the backend
    advancePhase(1, 'Checking daemon...')

    getDaemonStatus()
      .then((result) => {
        const s = result?.status

        if (s === 'connected') {
          // Fast path: daemon already connected from setup()
          // Reveal all phases in quick succession
          fastPath = true
          advancePhase(2, 'Connected')
          setTimeout(() => completeAfterHold(), 100)
          return
        }

        if (s === 'not_configured') {
          // No daemon needed (local-only mode) — complete quickly
          fastPath = true
          advancePhase(2, 'Loading...')
          setTimeout(() => completeAfterHold(), 100)
          return
        }

        // Slow path: daemon not connected, wait for health check
        advancePhase(2, 'Waiting for daemon...')

        if (isTauri()) {
          import('@tauri-apps/api/event').then(({ listen }) => {
            listen('daemon-status', (event) => {
              const evStatus = event.payload?.status
              if (evStatus === 'connected') {
                completeAfterHold()
              } else if (evStatus === 'failed') {
                showError('Could not start daemon')
              } else if (evStatus === 'reconnecting') {
                status = 'Reconnecting...'
              }
            }).then(unlisten => {
              cleanupListener = unlisten
            })
          })

          // Timeout: 15s without connection → error
          timeoutId = setTimeout(() => {
            if (!completed && !error) {
              showError('Could not start daemon — timed out')
            }
          }, 15000)
        }
      })
      .catch(() => {
        // IPC failed entirely — complete anyway (degraded mode)
        fastPath = true
        completeAfterHold()
      })

    return () => {
      cleanupListener?.()
      clearTimeout(timeoutId)
    }
  })

  function handleRetry() {
    error = null
    phase = 1
    status = 'Retrying...'
    splashStart = Date.now()
    onRetry()
  }

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
      style:opacity={phase === 3 && !error ? 1 : phase > 0 ? 0.85 : 0}
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
    {#if error}
      <p class="text-[13px] text-danger-400">{error}</p>
      <div class="mt-4 flex gap-3">
        <button
          class="px-4 py-1.5 text-[12px] font-medium text-white/90 bg-white/10 rounded hover:bg-white/15 transition-colors"
          onclick={handleRetry}
        >Retry</button>
        <button
          data-testid="continue-anyway-btn"
          class="px-4 py-1.5 text-[12px] font-medium text-white/40 hover:text-white/60 transition-colors"
          onclick={onContinue}
        >Continue anyway</button>
      </div>
    {:else if status}
      <p
        class="text-[12px] text-white/30"
        class:opacity-0={phase === 0}
        class:opacity-100={phase > 0}
        style="transition: opacity {reducedMotion ? '0ms' : '300ms'} ease-out;"
      >{status}</p>
    {/if}
  </div>
</div>
