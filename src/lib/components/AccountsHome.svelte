<script>
  import { onMount } from 'svelte'
  import AccountRow from './AccountRow.svelte'
  import AddAccountFlow from './AddAccountFlow.svelte'
  import {
    accountState,
    refreshAccounts,
    refreshAccountRelationships,
    refreshResolvedBases,
    refreshUsage,
    rememberChoice,
    setGlobalDefault,
  } from '../accounts.svelte.js'
  import { revealDirectory } from '../ipc.js'
  import { tools } from '../toolRegistry.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    dark = false,
    projects = [],
    selectedProject = null,
    states = null,
    onClose = () => {},
    onOpenProject = () => {},
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

  let expandedIds = $state(new Set())
  let addTool = $state(null)
  let signInAccount = $state(null)

  $effect(() => {
    const requested = requestedAddTool
    if (!requested) return
    onRequestedAddConsumed()
    startAdd(requested)
  })

  function stateFor(tool) {
    return states?.[tool] ?? accountState(tool)
  }

  function unhealthy(account) {
    if (!account?.logged_in || account?.usage?.status === 'unauthorized') return true
    return (account?.usage?.windows ?? []).some(
      (window) => Number(window.used_percentage) >= 100 || window.severity === 'critical'
    )
  }

  $effect(() => {
    const next = new Set(expandedIds)
    let changed = false
    for (const descriptor of registry) {
      for (const account of stateFor(descriptor.id).accounts ?? []) {
        if (unhealthy(account) && !next.has(`${descriptor.id}:${account.id}`)) {
          next.add(`${descriptor.id}:${account.id}`)
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
      registry.flatMap((descriptor) => [
        refreshAccounts(descriptor.id, { force: true }),
        refreshAccountRelationships(descriptor.id, { force: true }),
        refreshResolvedBases(descriptor.id, { force: true }),
        refreshUsage(descriptor.id),
      ])
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

  function selectedAlias(state) {
    for (const base of state.resolvedBases ?? []) {
      if (!base?.selectorValue) continue
      const account = (state.accounts ?? []).find(
        (candidate) => (candidate.dir ?? candidate.config_dir) === base.selectorValue
      )
      const expansion = base.expansions?.[0]
      if (account && expansion) return { account, expansion }
    }
    return null
  }

  function projectMemory(project, tool) {
    return project?.accountMemory?.[tool] ?? project?.account_memory?.[tool] ?? null
  }

  async function convertAlias(tool, account) {
    const affected = projects.filter((project) => !projectMemory(project, tool))
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
      {@const alias = selectedAlias(state)}
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

        <div class="space-y-2">
          {#each state.accounts ?? [] as account (account.id)}
            <AccountRow
              tool={descriptor.id}
              {account}
              relationships={state.relationships?.[account.id] ?? {}}
              defaultAccountId={state.defaultAccountId}
              expanded={expandedIds.has(`${descriptor.id}:${account.id}`)}
              {dark}
              usageNote={descriptor.capabilities.usageNote}
              onToggle={() => toggle(descriptor.id, account.id)}
              onRemovePin={(project) => removePin(descriptor.id, project)}
              {onOpenProject}
              onSetDefault={() => setGlobalDefault(descriptor.id, account.id)}
              onSignIn={() => startSignIn(descriptor.id, account)}
              onReveal={() => revealDirectory(account.dir ?? account.config_dir)}
            />
          {/each}
        </div>

        {#if alias}
          <aside class="rounded-lg border border-amber-500/20 bg-amber-500/[0.05] px-3 py-2.5" data-testid="account-alias-{descriptor.id}">
            <div class="flex items-center justify-between gap-4">
              <p class="text-[10px] leading-relaxed {t.textSecondary}">
                Base command <strong>{alias.expansion.name}</strong> selects {alias.account.display_name ?? alias.account.label} for projects without a settled account.
              </p>
              <button class="shrink-0 text-[10px] font-semibold text-amber-500" onclick={() => convertAlias(descriptor.id, alias.account)}>Convert to pins</button>
            </div>
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
