<script>
  import Shell from './Shell.svelte'
  import SplashScreen from './lib/SplashScreen.svelte'
  import { isTauri } from './lib/ipc.js'

  let showSplash = $state(true)
  let shellReady = $state(false)
  let daemonRetryError = $state(null)
  let daemonRetryErrorTimer = null

  function errorMessage(error) {
    if (error && typeof error === 'object' && typeof error.message === 'string' && error.message.trim()) {
      return error.message
    }
    if (typeof error === 'string' && error.trim()) {
      return error
    }
    return 'Failed to restart daemon. Check logs for details.'
  }

  function showDaemonRetryError(error) {
    daemonRetryError = errorMessage(error)
    if (daemonRetryErrorTimer) {
      clearTimeout(daemonRetryErrorTimer)
    }
    daemonRetryErrorTimer = setTimeout(() => {
      daemonRetryError = null
      daemonRetryErrorTimer = null
    }, 8000)
  }

  $effect(() => {
    return () => {
      if (daemonRetryErrorTimer) {
        clearTimeout(daemonRetryErrorTimer)
      }
    }
  })

  function handleSplashComplete() {
    shellReady = true
    // Brief crossfade — splash fades out while Shell fades in
    setTimeout(() => { showSplash = false }, 300)
  }

  async function handleRetry() {
    // Re-trigger daemon startup via Tauri command
    if (isTauri()) {
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        await invoke('start_daemon')
      } catch (error) {
        console.error('[splash] daemon retry failed:', error)
        showDaemonRetryError(error)
      }
    }
  }

  function handleContinue() {
    // Skip splash, enter degraded mode
    shellReady = true
    showSplash = false
  }
</script>

{#if showSplash}
  <div
    class="absolute inset-0 z-50"
    style="opacity: {shellReady ? 0 : 1}; transition: opacity 300ms ease-out; pointer-events: {shellReady ? 'none' : 'auto'};"
  >
    <SplashScreen
      onComplete={handleSplashComplete}
      onRetry={handleRetry}
      onContinue={handleContinue}
    />
  </div>
{/if}

{#if daemonRetryError}
  <div class="pointer-events-none absolute top-4 left-1/2 z-[60] w-full max-w-xl -translate-x-1/2 px-4">
    <div class="pointer-events-auto flex items-center gap-3 rounded-lg border border-danger-500/30 bg-danger-500/90 px-4 py-2 text-[12px] text-white shadow-lg backdrop-blur-sm" data-testid="daemon-retry-error-banner">
      <span class="flex-1">
        Retry failed: {daemonRetryError}
      </span>
      <button
        class="text-white/80 transition-colors hover:text-white"
        onclick={() => { daemonRetryError = null }}
        aria-label="Dismiss retry error"
      >
        Dismiss
      </button>
    </div>
  </div>
{/if}

{#if shellReady}
  <div
    class="h-full"
    style="opacity: {showSplash ? 0 : 1}; transition: opacity 300ms ease-out;"
  >
    <Shell />
  </div>
{/if}
