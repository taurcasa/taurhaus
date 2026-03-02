<script>
  import { coordinationPreflightCheck } from '../ipc.js'
  import { themeTokens } from '../themeTokens.js'

  let { dark = false, projectPath = '', children } = $props()

  const t = $derived(themeTokens(dark))

  let loading = $state(true)
  let blockingErrors = $state([])
  let agentWarnings = $state([])
  let errorMessage = $state('')

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
          model: 'gpt-5.3-codex',
          projectId,
          description: null,
        },
        {
          name: 'gemini-check',
          cliTool: 'gemini',
          model: 'gemini-2.5-pro',
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

  $effect(() => {
    const currentProjectPath = projectPath
    let cancelled = false

    loading = true
    blockingErrors = []
    agentWarnings = []
    errorMessage = ''

    coordinationPreflightCheck(minimalPreflightRequest(currentProjectPath))
      .then((report) => {
        if (cancelled) return
        blockingErrors = coerceBlockingErrors(report)
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
  </section>
{:else}
  {@render children(agentWarnings)}
{/if}
