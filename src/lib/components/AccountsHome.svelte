<script>
  import { onMount } from 'svelte'
  import AccountRow from './AccountRow.svelte'
  import AccountPicker from './AccountPicker.svelte'
  import AddAccountFlow from './AddAccountFlow.svelte'
  import {
    accountState,
    opaqueBaseNotice,
    refreshAccounts,
    refreshAccountRelationships,
    refreshResolvedBases,
    refreshUsage,
    rememberChoice,
    setGlobalDefault,
  } from '../accounts.svelte.js'
  import { coordinationSwitchTeamAccount, revealDirectory } from '../ipc.js'
  import { baseCommandSelection } from '../accountPresentation.js'
  import { tools } from '../toolRegistry.js'
  import { themeTokens } from '../themeTokens.js'
  import { exhaustedUsage, liveUsageWindows, windowPressure } from '../usageWindows.js'

  let {
    dark = false,
    projects = [],
    selectedProject = null,
    states = null,
    onClose = () => {},
    onOpenProject = () => {},
    onOpenTeam = () => {},
    onSignIn = null,
    requestedAddTool = null,
    onRequestedAddConsumed = () => {},
  } = $props()

  const registry = $derived(tools())
  const t = $derived(themeTokens(dark))
  const shellTone = $derived(dark ? 'bg-zinc-950' : 'bg-zinc-50/70')
  const headerTone = $derived(dark ? 'border-zinc-800 bg-zinc-950/95' : 'border-zinc-200 bg-white/95')
  const bannerTone = $derived(
    dark
      ? 'border-amber-400/20 bg-amber-400/[0.06] text-amber-300'
      : 'border-amber-300 bg-amber-50 text-amber-800'
  )
  const refreshTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-300 hover:bg-zinc-800'
      : 'border-zinc-200 bg-white text-zinc-600 hover:bg-zinc-50'
  )
  const addTone = $derived(
    dark
      ? 'border-zinc-700 text-zinc-300 hover:bg-zinc-800'
      : 'border-zinc-200 text-zinc-600 hover:bg-white'
  )
  const listTone = $derived(dark ? 'border-white/[0.07]' : 'border-zinc-200')
  const switchPendingTone = $derived(
    dark
      ? 'border-brand-400/30 bg-zinc-950/95 text-zinc-200'
      : 'border-brand-200 bg-white text-zinc-700'
  )

  let expandedIds = $state(new Set())
  const autoExpandedIds = new Set()
  let addTool = $state(null)
  let signInAccount = $state(null)
  let switchContext = $state(null)
  let switchError = $state(null)
  let switching = $state(false)

  $effect(() => {
    const requested = requestedAddTool
    if (!requested) return
    onRequestedAddConsumed()
    startAdd(requested)
  })

  function stateFor(tool) {
    return states?.[tool] ?? accountState(tool)
  }

  // Auto-expansion follows the row's own health dot: a window the provider
  // called critical opens the row even before the percentage reaches its limit.
  function unhealthy(account) {
    if (!account?.logged_in || exhaustedUsage(account?.usage) !== null) return true
    return liveUsageWindows(account?.usage?.windows).some(
      (window) => windowPressure(window) === 'critical'
    )
  }

  $effect(() => {
    const next = new Set(expandedIds)
    let changed = false
    for (const descriptor of registry) {
      for (const account of stateFor(descriptor.id).accounts ?? []) {
        const key = `${descriptor.id}:${account.id}`
        if (!unhealthy(account)) {
          autoExpandedIds.delete(key)
        } else if (!autoExpandedIds.has(key)) {
          autoExpandedIds.add(key)
          next.add(key)
          changed = true
        }
      }
    }
    if (changed) expandedIds = next
  })

  onMount(() => {
    if (states) return
    void refreshAll()
  })

  const degraded = $derived(registry.some((descriptor) => stateFor(descriptor.id).degraded))
  const observedAt = $derived.by(() => {
    const observations = registry.flatMap((descriptor) =>
      (stateFor(descriptor.id).accounts ?? [])
        .map((account) => Date.parse(account?.usage?.observed_at))
        .filter(Number.isFinite)
    )
    return observations.length ? Math.max(...observations) : null
  })
  const freshness = $derived(
    observedAt == null
      ? 'No usage observations yet'
      : `Usage as of ${new Date(observedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
  )

  function toggle(tool, accountId) {
    const key = `${tool}:${accountId}`
    const next = new Set(expandedIds)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    expandedIds = next
  }

  async function refreshAll() {
    await Promise.all(
      registry.flatMap((descriptor) => {
        const refreshes = [
          refreshAccounts(descriptor.id, { force: true }),
          refreshAccountRelationships(descriptor.id, { force: true }),
          refreshResolvedBases(descriptor.id, { force: true }),
        ]
        if (descriptor.capabilities.usage) refreshes.push(refreshUsage(descriptor.id))
        return refreshes
      })
    )
  }

  async function removePin(tool, project) {
    await rememberChoice(project.id, tool, null)
    await refreshAccountRelationships(tool, { force: true })
  }

  function startSignIn(tool, account) {
    if (onSignIn) {
      onSignIn(tool, account)
      return
    }
    addTool = tool
    signInAccount = account
  }

  function startAdd(tool) {
    addTool = tool
    signInAccount = null
  }

  function closeAdd() {
    addTool = null
    signInAccount = null
  }

  function startTeamSwitch(tool, account, team) {
    switchError = null
    switchContext = { tool, account, team }
  }

  async function switchTeamAccount(accountId) {
    const context = switchContext
    if (!context || switching) return
    switchError = null
    switching = true
    try {
      await coordinationSwitchTeamAccount(context.team.name, context.tool, accountId)
      switchContext = null
      await refreshAccountRelationships(context.tool, { force: true })
    } catch (error) {
      switchError = error?.message ?? String(error)
    } finally {
      switching = false
    }
  }

  /**
   * What the base command does to this tool's accounts, in the same order
   * Settings reads it: an opaque head first — it outranks the chosen global
   * default — then a selector, whether an alias or the typed command carries it.
   *
   * Only a global default that can run silences the strip. The resolver falls
   * past a saved default it cannot use and lands on the selector, so a
   * signed-out default leaves the launch command deciding.
   */
  function baseSelection(state) {
    const selection = baseCommandSelection(state.resolvedBases, state.accounts ?? [])
    if (selection.opaqueHead) return selection
    const savedDefault = (state.accounts ?? []).find(
      (account) => account.id === state.defaultAccountId
    )
    if (savedDefault?.logged_in) return null
    return selection.account ? selection : null
  }

  function projectMemory(project, tool) {
    return project?.accountMemory?.[tool] ?? project?.account_memory?.[tool] ?? null
  }

  /**
   * Which projects this tool has already settled somewhere else.
   *
   * The `projects` prop is the list as it was last read; an account chosen from
   * Overview or the sidebar since then lives in the live account state instead.
   * Conversion reads both authorities — the relationship index, refreshed at
   * the click, and the choices this run made optimistically, where a cleared
   * pin is the absence of a row rather than one more row to preserve.
   */
  function settledProjectIds(state) {
    const settled = new Set()
    for (const relationships of Object.values(state.relationships ?? {})) {
      const rows = [
        ...(relationships.pinnedProjects ?? relationships.pinned_projects ?? []),
        ...(relationships.lastUsedProjects ?? relationships.last_used_projects ?? []),
      ]
      for (const project of rows) settled.add(project.id)
    }
    for (const [projectId, accountId] of Object.entries(state.projectChoices ?? {})) {
      if (accountId) settled.add(projectId)
      else settled.delete(projectId)
    }
    return settled
  }

  async function convertAlias(tool, account) {
    await refreshAccountRelationships(tool, { force: true })
    const settled = settledProjectIds(stateFor(tool))
    const affected = projects.filter(
      (project) => project?.id && !settled.has(project.id) && !projectMemory(project, tool)
    )
    await Promise.all(affected.map((project) => rememberChoice(project.id, tool, account.id)))
    await refreshAccountRelationships(tool, { force: true })
  }
</script>

<main class="h-full overflow-y-auto {shellTone}" data-testid="accounts-home">
  <header class="sticky top-0 z-10 flex min-h-16 items-center justify-between border-b px-6 py-3 backdrop-blur {headerTone}">
    <div>
      <div class="flex items-center gap-2">
        <button class="text-[12px] {t.textTertiary}" onclick={onClose} aria-label="Close accounts">←</button>
        <h1 class="text-[16px] font-semibold tracking-tight {t.textPrimary}">Accounts</h1>
      </div>
      <p class="mt-0.5 pl-6 text-[10px] {t.textTertiary}" data-testid="accounts-freshness">{freshness}</p>
    </div>
    <button
      class="h-8 rounded-md border px-3 text-[11px] font-medium transition-colors {refreshTone}"
      onclick={refreshAll}
      aria-label="Refresh account usage"
    >↻ Refresh</button>
  </header>

  <div class="mx-auto max-w-5xl space-y-7 px-6 py-6">
    {#if degraded}
      <div class="rounded-lg border px-3 py-2 text-[11px] {bannerTone}" data-testid="accounts-degraded-banner">
        Detection degraded — showing last-known accounts. Refresh or resume sign-in from the affected row.
      </div>
    {/if}

    {#each registry as descriptor (descriptor.id)}
      {@const state = stateFor(descriptor.id)}
      {@const selection = baseSelection(state)}
      <section class="space-y-2.5" data-testid="accounts-tool-{descriptor.id}" data-tool={descriptor.id}>
        <div class="flex items-end justify-between gap-4">
          <div>
            <h2 class="text-[12px] font-semibold {t.textPrimary}">{descriptor.label}</h2>
            <p class="mt-0.5 text-[10px] {t.textTertiary}">
              {(state.accounts ?? []).length} detected account{(state.accounts ?? []).length === 1 ? '' : 's'}
            </p>
          </div>
          {#if descriptor.capabilities.accountSelection && descriptor.accountLoginCommand}
            <button
              class="h-7 rounded-md border px-2.5 text-[10px] font-medium transition-colors {addTone}"
              onclick={() => startAdd(descriptor.id)}
              data-testid="add-account-{descriptor.id}"
            >+ Add account</button>
          {/if}
        </div>

        <div class="overflow-hidden rounded-lg border {listTone}">
          {#each state.accounts ?? [] as account (account.id)}
            <AccountRow
              tool={descriptor.id}
              {account}
              relationships={state.relationships?.[account.id] ?? {}}
              defaultAccountId={state.defaultAccountId}
              expanded={expandedIds.has(`${descriptor.id}:${account.id}`)}
              {dark}
              usageNote={descriptor.capabilities.usageNote}
              canSignIn={Boolean(descriptor.capabilities.accountSelection && descriptor.accountLoginCommand)}
              onToggle={() => toggle(descriptor.id, account.id)}
              onRemovePin={(project) => removePin(descriptor.id, project)}
              {onOpenProject}
              {onOpenTeam}
              onSwitchTeamAccount={descriptor.capabilities.accountSelection
                ? (team) => startTeamSwitch(descriptor.id, account, team)
                : null}
              onSetDefault={() => setGlobalDefault(descriptor.id, account.id)}
              onSignIn={() => startSignIn(descriptor.id, account)}
              onReveal={() => revealDirectory(account.dir ?? account.config_dir)}
            />
          {/each}
        </div>

        {#if selection?.opaqueHead}
          <aside class="rounded-lg border border-amber-500/20 bg-amber-500/[0.05] px-3 py-2.5" data-testid="account-base-opaque-{descriptor.id}">
            <p class="text-[10px] leading-relaxed {t.textSecondary}">
              {opaqueBaseNotice(selection.opaqueHead, descriptor.id)}
            </p>
          </aside>
        {:else if selection && selection.usable}
          <aside class="rounded-lg border border-amber-500/20 bg-amber-500/[0.05] px-3 py-2.5" data-testid="account-alias-{descriptor.id}">
            <div class="flex items-center justify-between gap-4">
              <p class="text-[10px] leading-relaxed {t.textSecondary}">
                Base command <strong>{selection.alias?.name ?? selection.command}</strong> selects {selection.account.display_name ?? selection.account.label} for projects without a settled account.
              </p>
              <button class="shrink-0 text-[10px] font-semibold text-amber-500" onclick={() => convertAlias(descriptor.id, selection.account)}>Convert to pins</button>
            </div>
          </aside>
        {:else if selection}
          <aside class="rounded-lg border border-rose-500/20 bg-rose-500/[0.05] px-3 py-2.5" data-testid="account-alias-signed-out-{descriptor.id}">
            <p class="text-[10px] leading-relaxed {t.textSecondary}">
              Base command <strong>{selection.alias?.name ?? selection.command}</strong> names {selection.account.display_name ?? selection.account.label}, which is signed out — launches fall through to another account. Sign it in below or update the command; pinning is offered once it can run.
            </p>
          </aside>
        {/if}
      </section>
    {/each}
  </div>
</main>

{#if addTool}
  <AddAccountFlow
    open={true}
    tool={addTool}
    projectId={selectedProject?.id ?? projects[0]?.id ?? null}
    existingAccount={signInAccount}
    {dark}
    onClose={closeAdd}
  />
{/if}

{#if switchContext}
  {@const switchState = stateFor(switchContext.tool)}
  <div class="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4" data-testid="team-account-switcher">
    <div>
      {#if switching}
        <div
          class="w-[22rem] max-w-full rounded-xl border p-4 text-sm shadow-2xl {switchPendingTone}"
          role="status"
          data-testid="team-account-switch-pending"
        >
          Switching {switchContext.team.name}… The team will restart when the account is ready.
        </div>
      {:else}
        <AccountPicker
          tool={switchContext.tool}
          accounts={switchState.accounts ?? []}
          projectName={switchContext.team.name}
          defaultAccountId={switchState.defaultAccountId}
          degraded={switchState.degraded}
          preselectedAccountId={switchContext.account.id}
          {dark}
          showRemember={false}
          onConfirm={switchTeamAccount}
          onCancel={() => { switchContext = null }}
          onAddAccount={(tool) => { switchContext = null; startAdd(tool) }}
          onManageAccounts={() => { switchContext = null }}
        />
      {/if}
      {#if switchError}
        <p class="mt-2 rounded-md bg-rose-950 px-3 py-2 text-[11px] text-rose-200" role="status">{switchError}</p>
      {/if}
    </div>
  </div>
{/if}
