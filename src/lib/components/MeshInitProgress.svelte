<script>
  import { coordinationInitializeTeam, onCoordinationStepProgress } from '../ipc.js'

  let {
    dark = false,
    request = null,
    onsuccess = () => {},
    onback = () => {},
    onretry = () => {},
  } = $props()

  const stepsOrder = [
    'validate_configuration',
    'create_team',
    'create_panes',
    'launch_sessions',
    'join_mesh',
    'start_daemons',
    'send_onboarding',
  ]

  const keyline = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const textPrimary = $derived(dark ? 'text-zinc-100' : 'text-zinc-900')
  const textMuted = $derived(dark ? 'text-zinc-500' : 'text-zinc-500')
  const subtleButton = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:border-zinc-600 hover:text-zinc-200'
      : 'border-zinc-300 text-zinc-700 hover:border-zinc-400 hover:text-zinc-900'
  )

  let steps = $state([])
  let activeTeamName = $state('')
  let running = $state(false)
  let succeeded = $state(false)
  let failed = $state(false)
  let failedStep = $state('')
  let errorMessage = $state('')
  let lastRequest = $state(null)

  function resetSteps() {
    steps = stepsOrder.map((step) => ({
      step,
      status: 'pending',
      message: null,
    }))
  }

  function prettyStep(step) {
    return step
      .split('_')
      .map((part) => part[0]?.toUpperCase() + part.slice(1))
      .join(' ')
  }

  function stepIndex(step) {
    const preferredIndex = stepsOrder.indexOf(step)
    if (preferredIndex >= 0) return preferredIndex
    return stepsOrder.length + steps.findIndex((entry) => entry.step === step)
  }

  function setStep(stepName, status, message) {
    const idx = steps.findIndex((step) => step.step === stepName)
    if (idx === -1) {
      steps = [...steps, { step: stepName, status, message: message ?? null }]
    } else {
      const next = [...steps]
      next[idx] = {
        ...next[idx],
        status,
        message: message ?? next[idx].message ?? null,
      }
      steps = next
    }
    steps = [...steps].sort((a, b) => stepIndex(a.step) - stepIndex(b.step))
  }

  function applyReport(report) {
    for (const progress of report?.steps ?? []) {
      setStep(progress.step, progress.status, progress.message ?? null)
    }
    failedStep = report?.failedStep || report?.failed_step || ''
    if (failedStep) {
      failed = true
      succeeded = false
      errorMessage = report?.message || 'Initialization failed.'
    } else {
      failed = false
      succeeded = true
      errorMessage = ''
    }
  }

  async function runInitialization(nextRequest) {
    if (!nextRequest) return
    lastRequest = nextRequest
    activeTeamName = nextRequest.teamName ?? nextRequest.team_name ?? ''
    resetSteps()
    failedStep = ''
    errorMessage = ''
    running = true
    failed = false
    succeeded = false

    try {
      const report = await coordinationInitializeTeam(nextRequest)
      applyReport(report)
      if (!report?.failedStep && !report?.failed_step) {
        onsuccess({
          teamName: report?.teamName ?? report?.team_name ?? activeTeamName,
          report,
        })
      }
    } catch (err) {
      failed = true
      succeeded = false
      failedStep = failedStep || 'initialize_team'
      errorMessage = err?.message || `${err || 'Initialization failed.'}`
      setStep(failedStep, 'failed', errorMessage)
    } finally {
      running = false
    }
  }

  function statusGlyph(status) {
    if (status === 'running') return '●'
    if (status === 'succeeded') return '✓'
    if (status === 'failed') return '✗'
    return '○'
  }

  function statusClass(status) {
    if (status === 'running') return 'text-brand-500 animate-pulse'
    if (status === 'succeeded') return 'text-success-500'
    if (status === 'failed') return 'text-danger-500'
    return dark ? 'text-zinc-500' : 'text-zinc-400'
  }

  function handleProgressEvent(event) {
    const payload = event?.payload ?? event
    if (!payload || payload.operation !== 'initialize_team') return
    if (activeTeamName && payload.teamName && payload.teamName !== activeTeamName) return
    const progress = payload.progress
    if (!progress?.step) return
    setStep(progress.step, progress.status ?? 'pending', progress.message ?? null)
  }

  function handleRetry() {
    onretry()
    void runInitialization(lastRequest)
  }

  $effect(() => {
    if (!request) return
    void runInitialization(request)
  })

  $effect(() => {
    let cancelled = false
    let unlisten = null
    onCoordinationStepProgress((event) => {
      handleProgressEvent(event)
    })
      .then((dispose) => {
        if (cancelled) {
          if (typeof dispose === 'function') dispose()
          return
        }
        unlisten = dispose
      })
      .catch(() => {})
    return () => {
      cancelled = true
      if (typeof unlisten === 'function') unlisten()
    }
  })
</script>

<section class="space-y-3" data-testid="mesh-init-progress">
  <header class="pb-2 border-b {keyline}">
    <h2 class="text-base font-semibold {textPrimary}">Initializing Team{activeTeamName ? `: ${activeTeamName}` : ''}</h2>
    <p class="mt-1 text-xs {textMuted}">Per-step progress updates appear here in real time.</p>
  </header>

  <div class="divide-y {keyline} border-y {keyline}">
    {#each steps as entry}
      <div class="py-2" data-testid={`mesh-init-step-${entry.step}`}>
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center gap-2 min-w-0">
            <span class={`text-sm font-semibold ${statusClass(entry.status)}`} data-testid={`mesh-init-icon-${entry.step}`}>
              {statusGlyph(entry.status)}
            </span>
            <span class="text-sm truncate {textPrimary}">{prettyStep(entry.step)}</span>
          </div>
          <span class={`text-[11px] font-semibold uppercase tracking-[0.06em] ${statusClass(entry.status)}`} data-testid={`mesh-init-status-${entry.step}`}>
            {entry.status}
          </span>
        </div>
        {#if entry.message}
          <p class="mt-1 pl-5 text-xs {textMuted}">{entry.message}</p>
        {/if}
      </div>
    {/each}
  </div>

  {#if failed}
    <div class="border-l-2 border-danger-400 pl-3 py-1 text-xs text-danger-600" data-testid="mesh-init-failure">
      <p class="font-semibold">Initialization failed{failedStep ? ` at ${prettyStep(failedStep)}` : ''}.</p>
      {#if errorMessage}
        <p class="mt-1">{errorMessage}</p>
      {/if}
    </div>
  {/if}

  {#if succeeded}
    <div class="border-l-2 border-success-400 pl-3 py-1 text-xs text-success-600" data-testid="mesh-init-success">
      Team initialized!
    </div>
  {/if}

  <div class="flex justify-end gap-2">
    {#if failed}
      <button
        class="rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
        onclick={handleRetry}
        disabled={running}
        data-testid="mesh-init-retry-button"
      >
        Retry
      </button>
      <button
        class="rounded-md border px-3 py-1.5 text-xs {subtleButton} disabled:cursor-not-allowed disabled:opacity-50"
        onclick={onback}
        disabled={running}
        data-testid="mesh-init-back-button"
      >
        Back to Setup
      </button>
    {/if}

    {#if succeeded}
      <button
        class="rounded-md bg-brand-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-brand-700"
        onclick={() => onsuccess({ teamName: activeTeamName })}
        data-testid="mesh-init-runtime-button"
      >
        Switch to Runtime
      </button>
    {/if}
  </div>
</section>
