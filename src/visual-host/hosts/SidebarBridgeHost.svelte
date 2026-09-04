<script>
  import { onMount, tick } from 'svelte'
  import ShellTitlebar from '../../lib/components/shell/ShellTitlebar.svelte'
  import Sidebar from '../../lib/Sidebar.svelte'
  import { setScrollDrivenTrackingForTesting } from '../../lib/sidebarBridge.js'
  import { themeTokens } from '../../lib/themeTokens.js'

  let { scenario, theme = 'light' } = $props()

  const dark = $derived(theme === 'dark')
  const t = $derived(themeTokens(dark))
  const surfaceProps = $derived({
    settingsOpen: scenario?.props?.settingsOpen ?? false,
    accountsOpen: scenario?.props?.accountsOpen ?? false,
    projectsOpen: scenario?.props?.projectsOpen ?? false,
  })

  // The Sidebar captures the tracking mode when it initializes, so a
  // scenario that exercises the JS fallback must force it before the child
  // mounts — this script body runs first. The initial value is the right
  // one: hosts remount per scenario. Reset on unmount so the next fixture
  // detects for itself.
  // svelte-ignore state_referenced_locally
  if (scenario?.forceJsTracking) {
    setScrollDrivenTrackingForTesting(false)
  }

  onMount(() => {
    void (async () => {
      if (scenario?.scrollTo != null) {
        await tick()
        const list = document.querySelector('[data-testid="sidebar-project-scroll"]')
        if (list) list.scrollTop = scenario.scrollTo
      }
    })()
    return () => setScrollDrivenTrackingForTesting(null)
  })
</script>

<!-- The Shell body junction the bridge lives at: teal frame, real titlebar
     (manila tab and inverse scoop for reference), real Sidebar, the frame
     gutter, and the main panel's surface — the same DOM shape Shell.svelte
     builds, so the bridge driver finds the panel where it does in the app. -->
<div class="shell-frame h-screen flex flex-col font-sans antialiased">
  <ShellTitlebar {dark} activeTab="overview" {...surfaceProps} />
  <div class="flex-1 flex gap-1.5 p-1.5 pt-0 min-h-0">
    <Sidebar
      projects={scenario?.projects ?? []}
      selectedProject={scenario?.selectedProject ?? null}
      daemonStatus={scenario?.daemonStatus ?? null}
      {...surfaceProps}
      {dark}
      actions={{}}
    />
    <main class="shell-main-surface shell-main-panel flex-1 {t.textBody} rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden">
      <div class="flex-1 flex items-center justify-center">
        <p class="text-[13px] {t.textTertiary}">
          {scenario?.selectedProject && !surfaceProps.settingsOpen ? scenario.selectedProject.name : 'Select a project'}
        </p>
      </div>
    </main>
  </div>
</div>
