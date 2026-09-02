<script>
  import SlideOver from './SlideOver.svelte'
  import { accountState, refreshAccounts, setGlobalDefault } from '../accounts.svelte.js'
  import { launchAccountLogin, prepareAccountDirectory } from '../ipc.js'
  import { toolDescriptor } from '../toolRegistry.js'
  import { themeTokens } from '../themeTokens.js'

  let {
    open = false,
    tool,
    projectId = null,
    existingAccount = null,
    dark = false,
    onClose = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const descriptor = $derived(toolDescriptor(tool))
  const toolLabel = $derived(descriptor?.label ?? tool)
  const title = $derived(`${existingAccount ? 'Sign in to' : 'Add'} a ${toolLabel} account`)
  const inputTone = $derived(
    dark
      ? 'border-zinc-700 bg-zinc-900 text-zinc-100 focus:border-brand-500'
      : 'border-zinc-200 bg-white text-zinc-900 focus:border-brand-500'
  )
  const cardTone = $derived(
    dark ? 'border-white/[0.07] bg-zinc-900/50' : 'border-zinc-200 bg-zinc-50/70'
  )
  const primaryTone = $derived(
    dark
      ? 'bg-brand-500 text-white hover:bg-brand-400'
      : 'bg-brand-600 text-white hover:bg-brand-500'
  )

  let name = $state('')
  let configDir = $state(null)
  let waiting = $state(false)
  let detected = $state(null)
  let error = $state(null)
  const DETECTION_POLL_INITIAL_MS = 2_000
  const DETECTION_POLL_MAX_MS = 30_000
  const DETECTION_POLL_DEADLINE_MS = 5 * 60 * 1000

  $effect(() => {
    if (!open) return
    name = existingAccount?.display_name ?? existingAccount?.label ?? ''
    configDir = existingAccount?.dir ?? existingAccount?.config_dir ?? null
    waiting = false
    detected = null
    error = null
  })

  $effect(() => {
    if (!waiting) return
    const deadline = Date.now() + DETECTION_POLL_DEADLINE_MS
    let delay = DETECTION_POLL_INITIAL_MS
    let timer = null
    let cancelled = false
    const poll = async () => {
      await refreshAccounts(tool, { force: true })
      if (cancelled) return
      const found = accountState(tool).accounts.find((account) => {
        const dir = account?.dir ?? account?.config_dir
        return dir === configDir && account.logged_in
      })
      if (found) {
        detected = found
        waiting = false
        return
      }
      if (Date.now() >= deadline) {
        waiting = false
        return
      }
      timer = setTimeout(() => void poll(), Math.min(delay, deadline - Date.now()))
      delay = Math.min(delay * 2, DETECTION_POLL_MAX_MS)
    }
    void poll()
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  })

  async function openTerminal() {
    if (!projectId) {
      error = 'Select a project before opening the sign-in terminal.'
      return
    }
    error = null
    try {
      const directory = existingAccount
        ? configDir
        : await prepareAccountDirectory(tool, name)
      configDir = directory
      await launchAccountLogin(projectId, tool, directory)
      waiting = true
    } catch (failure) {
      error = failure?.message ?? String(failure)
      await refreshAccounts(tool, { force: true })
    }
  }

  async function makeDefault() {
    if (!detected?.id) return
    await setGlobalDefault(tool, detected.id)
  }
</script>

<SlideOver {open} {title} width={400} {dark} {onClose}>
  <div class="space-y-4" data-testid="add-account-flow">
    <section class="rounded-lg border p-3 {cardTone}">
      <div class="mb-2 flex items-center gap-2">
        <span class="flex h-5 w-5 items-center justify-center rounded-full bg-brand-500/15 text-[10px] font-semibold text-brand-500">1</span>
        <h3 class="text-[12px] font-semibold {t.textPrimary}">Account directory</h3>
      </div>
      {#if existingAccount}
        <p class="text-[12px] {t.textSecondary}">{configDir}</p>
      {:else}
        <label class="mb-1 block text-[11px] font-medium {t.textSecondary}" for="account-flow-name">Account name</label>
        <input
          id="account-flow-name"
          class="h-9 w-full rounded-md border px-2.5 text-[12px] outline-none {inputTone}"
          bind:value={name}
          placeholder="work"
        />
        <p class="mt-1.5 text-[10px] {t.textTertiary}">
          taurhaus creates a named sibling of {descriptor?.accountDirName ?? 'the default directory'}.
        </p>
      {/if}
    </section>

    <section class="rounded-lg border p-3 {cardTone}">
      <div class="mb-2 flex items-center gap-2">
        <span class="flex h-5 w-5 items-center justify-center rounded-full bg-brand-500/15 text-[10px] font-semibold text-brand-500">2</span>
        <h3 class="text-[12px] font-semibold {t.textPrimary}">Sign in with {toolLabel}</h3>
      </div>
      <p class="mb-3 text-[11px] leading-relaxed {t.textSecondary}">
        Credentials stay with {toolLabel}; taurhaus only opens its own sign-in command in the selected directory.
      </p>
      <button
        class="h-8 rounded-md px-3 text-[11px] font-semibold transition-colors disabled:opacity-50 {primaryTone}"
        onclick={openTerminal}
        disabled={waiting || (!existingAccount && !name.trim())}
      >Open sign-in terminal</button>
      {#if waiting}
        <p class="mt-3 text-[11px] text-amber-500" data-testid="account-login-waiting">
          Waiting for {toolLabel} to finish sign-in… You can close this panel; a signed-out row remains resumable.
        </p>
      {/if}
      {#if error}
        <p class="mt-3 text-[11px] text-rose-500" role="status">{error}</p>
      {/if}
    </section>

    {#if detected}
      <section class="rounded-lg border border-emerald-500/25 bg-emerald-500/5 p-3" data-testid="account-login-detected">
        <div class="mb-2 flex items-center gap-2">
          <span class="h-2 w-2 rounded-full bg-emerald-500"></span>
          <p class="text-[12px] font-semibold {t.textPrimary}">{detected.label} is signed in</p>
        </div>
        <div class="flex gap-2">
          <button class="text-[11px] font-medium text-brand-500" onclick={makeDefault}>Set as global default</button>
          <button class="text-[11px] font-medium text-brand-500" onclick={onClose}>Done</button>
        </div>
      </section>
    {/if}
  </div>
</SlideOver>
