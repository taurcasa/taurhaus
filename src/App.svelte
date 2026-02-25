<script>
  import Shell from './Shell.svelte'
  import SplashScreen from './lib/SplashScreen.svelte'
  import { isTauri } from './lib/ipc.js'

  let showSplash = $state(true)
  let shellReady = $state(false)

  function handleSplashComplete() {
    shellReady = true
    // Brief crossfade — splash fades out while Shell fades in
    setTimeout(() => { showSplash = false }, 300)
  }

  function handleRetry() {
    // Re-trigger daemon startup via Tauri command
    if (isTauri()) {
      import('@tauri-apps/api/core').then(({ invoke }) => {
        invoke('start_daemon').catch(() => {})
      })
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

{#if shellReady}
  <div
    class="h-full"
    style="opacity: {showSplash ? 0 : 1}; transition: opacity 300ms ease-out;"
  >
    <Shell />
  </div>
{/if}
