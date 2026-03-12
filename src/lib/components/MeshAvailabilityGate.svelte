<script>
  import { checkMeshInstallStatus, coordinationPreflightCheck, installMesh } from '../ipc.js'
  import { defaultModelForTool } from '../meshDefaults.js'
  import { themeTokens } from '../themeTokens.js'

  let { dark = false, projectPath = '', children } = $props()

  const t = $derived(themeTokens(dark))

  let loading = $state(true)
  let meshStatus = $state(null)
  let blockingErrors = $state([])
  let agentWarnings = $state([])
  let errorMessage = $state('')
  let refreshToken = $state(0)
  let installingMesh = $state(false)
  let installMessage = $state('')
  let installError = $state('')

  function minimalPreflightRequest(path) {
    const projectId = String(path || '').trim() || 'mesh-preflight-project'
    return {
      teamName: 'mesh-preflight',
      teamDescription: null,
      leadMode: 'attach_existing',
      lead: {
        name: 'team-lead',
        cliTool: 'claude',
        model: 'opus',
        projectId,
        description: null,
      },
      agents: [
        {
          name: 'codex-check',
          cliTool: 'codex',
          model: defaultModelForTool('codex'),
          projectId,
          description: null,
        },
        {
          name: 'gemini-check',
          cliTool: 'gemini',
          model: 'gemini-3.1-pro',
          projectId,
          description: null,
        },
      ],
    }
  }

  function coerceBlockingErrors(report) {
    if (Array.isArray(report?.blockingErrors)) return report.blockingErrors
    if (Array.isArray(report?.blocking_errors)) return report.blocking_errors
    return []
  }

  function coerceAgentWarnings(report) {
    if (Array.isArray(report?.agentWarnings)) return report.agentWarnings
    if (Array.isArray(report?.agent_warnings)) return report.agent_warnings
    return []
  }

  function isMeshMissing(error) {
    return String(error || '').includes('Mesh CLI not found')
  }

  function meshNeedsUpdate(status) {
    return Boolean(status?.needsUpdate ?? status?.needs_update ?? false)
  }

  function meshCompatibilityIssues(status) {
    if (Array.isArray(status?.compatibilityIssues)) return status.compatibilityIssues
    if (Array.isArray(status?.compatibility_issues)) return status.compatibility_issues
    return []
  }

  function meshEnvironmentAvailable(status) {
    return Boolean(status?.environmentAvailable ?? status?.environment_available ?? false)
  }

  function meshBundledVersion(status) {
    return status?.bundledVersion ?? status?.bundled_version ?? 'unknown'
  }

  function meshVersionMismatchError(status) {
    if (!status?.installed || !meshNeedsUpdate(status)) return ''
    const installed = status.version || 'unknown'
    const bundled = meshBundledVersion(status)
    return `Mesh CLI ${installed} is installed, but taurhaus requires ${bundled}. Update Mesh to continue.`
  }

  function meshCompatibilityMessages(status) {
    const issues = meshCompatibilityIssues(status)
      .map((issue) => issue?.message || '')
      .filter(Boolean)
    if (issues.length > 0) return issues
    const fallback = meshVersionMismatchError(status)
    return fallback ? [fallback] : []
  }

  const showMeshInstallActions = $derived(
    (blockingErrors.some(isMeshMissing) || meshCompatibilityMessages(meshStatus).length > 0) &&
      meshEnvironmentAvailable(meshStatus)
  )

  async function installBundledMesh() {
    installingMesh = true
    installError = ''
    installMessage = ''
    try {
      const result = await installMesh()
      installMessage = result?.message || ''
      refreshToken += 1
    } catch (err) {
      installError = err?.message || 'Failed to install Mesh CLI.'
    } finally {
      installingMesh = false
    }
  }

  $effect(() => {
    const currentProjectPath = projectPath
    const currentRefreshToken = refreshToken
    let cancelled = false

    loading = true
    meshStatus = null
    blockingErrors = []
    agentWarnings = []
    errorMessage = ''
    void currentRefreshToken

    Promise.all([
      checkMeshInstallStatus(),
      coordinationPreflightCheck(minimalPreflightRequest(currentProjectPath)),
    ])
      .then(([status, report]) => {
        if (cancelled) return
        meshStatus = status
        const mergedErrors = coerceBlockingErrors(report)
        const mismatches = meshCompatibilityMessages(status)
        for (const mismatch of mismatches) {
          if (!mergedErrors.includes(mismatch)) {
            mergedErrors.push(mismatch)
          }
        }
        blockingErrors = mergedErrors
        agentWarnings = coerceAgentWarnings(report)
      })
      .catch((err) => {
        if (cancelled) return
        errorMessage = err?.message || 'Failed to check Mesh availability.'
      })
      .finally(() => {
        if (!cancelled) {
          loading = false
        }
      })

    return () => {
      cancelled = true
    }
  })
</script>

{#if loading}
  <p class="text-sm {t.textMuted}" data-testid="mesh-availability-loading">Checking Mesh prerequisites...</p>
{:else if errorMessage}
  <div
    class="border-l-2 border-danger-400 pl-3 py-1 text-xs text-danger-600/95"
    data-testid="mesh-availability-error"
  >
    {errorMessage}
  </div>
{:else if blockingErrors.length > 0}
  <section class="space-y-3" data-testid="mesh-availability-blocking">
    <header class="pb-3 border-b {t.keyline}">
      <h2 class="text-sm font-semibold {t.textPrimary}" data-testid="mesh-availability-title">Mesh Setup Required</h2>
      <p class="mt-1 text-xs {t.textMuted}">Resolve these prerequisites before initializing a team.</p>
    </header>

    <ul class="divide-y {t.keyline} border-y {t.keyline}">
      {#each blockingErrors as blockingError (blockingError)}
        <li class="py-2 text-xs text-danger-600" data-testid="mesh-availability-error">
          {blockingError}
        </li>
      {/each}
    </ul>

    {#if blockingErrors.some(isMeshMissing)}
      <p class="text-xs {t.textMuted}" data-testid="mesh-availability-mesh-help">
        Install Mesh CLI, then restart taurhaus. Verify with <code>mesh --help</code>.
      </p>
    {/if}

    {#if showMeshInstallActions}
      <div class="space-y-2" data-testid="mesh-availability-install-actions">
        <button
          type="button"
          class="rounded-md border px-3 py-1 text-xs font-medium transition-colors hover:bg-white/5 {t.keyline} {t.textPrimary}"
          onclick={installBundledMesh}
          disabled={installingMesh}
          data-testid="mesh-install-button"
        >
          {installingMesh ? 'Installing Mesh...' : 'Install Bundled Mesh'}
        </button>
        {#if installMessage}
          <p class="text-xs text-success-600" data-testid="mesh-install-success">{installMessage}</p>
        {/if}
        {#if installError}
          <p class="text-xs text-danger-600" data-testid="mesh-install-error">{installError}</p>
        {/if}
      </div>
    {/if}
  </section>
{:else}
  {@render children(agentWarnings)}
{/if}
