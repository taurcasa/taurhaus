<script>
  import { coordinationInitializeTeam, onCoordinationStepProgress } from '../ipc.js'
  import { themeTokens } from '../themeTokens.js'

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
  const stepLabels = {
    validate_configuration: 'Validating configuration',
    create_team: 'Creating team',
    create_panes: 'Opening terminal panes',
    launch_sessions: 'Launching agent sessions',
    join_mesh: 'Connecting agents to mesh',
    start_daemons: 'Starting coordination daemons',
    send_onboarding: 'Sending agent instructions',
  }
  const stepDescriptions = {
    validate_configuration: 'Checking team name, agent tools, and project assignments',
    create_team: 'Writing team config to ~/.claude/teams/',
    create_panes: 'Creating tmux panes for each agent',
    launch_sessions: 'Starting CLI tools in each pane',
    join_mesh: 'Registering agents with mesh protocol',
    start_daemons: 'Launching file watchers for each agent inbox',
    send_onboarding: 'Delivering initial instructions to each agent',
  }

  const t = $derived(themeTokens(dark))
  const subtleButton = $derived(
    dark
      ? 'bg-zinc-800/60 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200'
      : 'bg-zinc-100 text-zinc-600 hover:bg-zinc-200 hover:text-zinc-900'
  )
  const primaryCta = 'h-8 inline-flex items-center rounded-md bg-brand-600 px-3 text-xs font-medium text-white hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50'

  let steps = $state([])
  let activeTeamName = $state('')
  let running = $state(false)
  let succeeded = $state(false)
  let failed = $state(false)
  let failedStep = $state('')
  let errorMessage = $state('')
  let lastRequest = $state(null)
  let elapsedSeconds = $state(0)
  let elapsedTimer = null
  const succeededSteps = $derived.by(() => steps.filter((entry) => entry.status === 'succeeded'))
  const failedEntry = $derived.by(() => steps.find((entry) => entry.status === 'failed') ?? null)

  function resetSteps() {
    steps = stepsOrder.map((step) => ({
      step,
      status: 'pending',
      message: null,
    }))
  }

  function prettyStep(step) {
    return (
      stepLabels[step] ??
      step
        .split('_')
        .map((part) => part[0]?.toUpperCase() + part.slice(1))
        .join(' ')
    )
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
    if (status === 'running') return 'text-brand-500 animate-[pulse_1.2s_ease-in-out_infinite]'
    if (status === 'succeeded') return 'text-success-500'
    if (status === 'failed') return 'text-danger-500'
    return dark ? 'text-zinc-500' : 'text-zinc-400'
  }

  function rowClass(status) {
    if (status === 'running') return dark ? 'bg-brand-500/10' : 'bg-brand-50'
    if (status === 'succeeded') return dark ? 'bg-success-500/10' : 'bg-success-50'
    if (status === 'failed') return dark ? 'bg-danger-500/10' : 'bg-danger-50'
    return ''
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
    if (!running) return
    elapsedSeconds = 0
    elapsedTimer = setInterval(() => {
      elapsedSeconds += 1
    }, 1000)
    return () => {
      if (elapsedTimer) clearInterval(elapsedTimer)
    }
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
  <header class="pb-3 border-b {t.keyline}">
    <div class="flex items-center">
      <h2 class="text-sm font-semibold {t.textPrimary}">Initializing{activeTeamName ? ` ${activeTeamName}` : ''}...</h2>
      {#if running}
        <span class="text-[11px] {t.textMuted} ml-2" data-testid="mesh-init-elapsed">Elapsed: {elapsedSeconds}s</span>
      {/if}
    </div>
  </header>

  <div class="space-y-0.5">
    {#each steps as entry}
      <div class="flex items-center gap-2 h-[28px] -mx-2 px-2 rounded transition-colors {rowClass(entry.status)}" data-testid={`mesh-init-step-${entry.step}`}>
        <span class={`text-xs ${statusClass(entry.status)}`} data-testid={`mesh-init-icon-${entry.step}`}>
          {statusGlyph(entry.status)}
        </span>
        <span class="text-[13px] truncate min-w-0 {t.textPrimary}">{prettyStep(entry.step)}</span>
        <span class={`ml-auto text-[10px] shrink-0 ${statusClass(entry.status)}`} data-testid={`mesh-init-status-${entry.step}`}>
          {entry.status}
        </span>
      </div>
      {#if entry.status === 'running' && stepDescriptions[entry.step]}
        <p class="ml-5 -mt-0.5 mb-1 text-[10px] {t.textMuted}" data-testid={`mesh-init-desc-${entry.step}`}>
          {stepDescriptions[entry.step]}
        </p>
      {/if}
      {#if entry.message}
        <p class="ml-5 -mt-0.5 mb-1 text-[11px] {t.textMuted}">{entry.message}</p>
      {/if}
    {/each}
  </div>

  {#if failed}
    <div class="border-l-2 border-danger-400 pl-3 py-1 text-xs text-danger-600/95" data-testid="mesh-init-failure">
      <p class="font-semibold">Initialization failed{failedStep ? ` at ${prettyStep(failedStep)}` : ''}.</p>
      {#if succeededSteps.length}
        <p class="mt-1 text-[11px]">
          Succeeded: {succeededSteps.map((entry) => prettyStep(entry.step)).join(', ')}
        </p>
      {/if}
      {#if failedEntry}
        <p class="mt-1 text-[11px]">Failed: {prettyStep(failedEntry.step)}</p>
      {/if}
      {#if errorMessage}
        <p class="mt-1">{errorMessage}</p>
      {/if}
      <details class="mt-2 text-[11px]" data-testid="mesh-init-failure-details">
        <summary class="cursor-pointer font-medium">What went wrong?</summary>
        <div class="mt-1 space-y-1">
          {#if failedEntry?.message}
            <p>{failedEntry.message}</p>
          {/if}
          {#if errorMessage}
            <p>{errorMessage}</p>
          {/if}
          {#if !failedEntry?.message && !errorMessage}
            <p>No additional failure details were provided by the backend.</p>
          {/if}
        </div>
      </details>
    </div>
  {/if}

  {#if succeeded}
    <div class="border-l-2 border-success-400 pl-3 py-1 text-xs text-success-600/95" data-testid="mesh-init-success">
      Team initialized!
    </div>
  {/if}

  <div class="flex justify-end gap-2">
    {#if failed}
      <button
        class={primaryCta}
        onclick={handleRetry}
        disabled={running}
        data-testid="mesh-init-retry-button"
      >
        Retry
      </button>
      <button
        class="rounded-md px-3 py-1.5 text-xs {subtleButton} disabled:cursor-not-allowed disabled:opacity-50"
        onclick={onback}
        disabled={running}
        data-testid="mesh-init-back-button"
      >
        Back
      </button>
    {/if}

    {#if succeeded}
      <button
        class={primaryCta}
        onclick={() => onsuccess({ teamName: activeTeamName })}
        data-testid="mesh-init-runtime-button"
      >
        Switch to Runtime
      </button>
    {/if}
  </div>
</section>
