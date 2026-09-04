<script>
  import {
    getSettings,
    updateSettings,
    getIndexStatus,
    rebuildIndex,
    getPlatform,
  } from './ipc.js'
  import { buildFrontendFallbackTerminalContract } from './ipc/system.js'
  import {
    accountState,
    forgetResolvedBases,
    opaqueBaseNotice,
    refreshAccounts,
    refreshResolvedBases,
  } from './accounts.svelte.js'
  import { lightThemes, darkThemes, DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME } from './shikiThemes.js'
  import { formatUserFacingError } from './format.js'
  import { baseCommandSelection } from './accountPresentation.js'
  import { themeTokens } from './themeTokens.js'
  import { tools, toolAccent } from './toolRegistry.js'
  import { getToolIcon } from './toolLogos.js'
  import { RAIL_ICONS } from './railIcons.js'
  import SurfaceDoorway from './components/SurfaceDoorway.svelte'

  let { dark = false, onClose = () => {}, onSettingsChanged = () => {}, codeThemeLight = DEFAULT_LIGHT_THEME, codeThemeDark = DEFAULT_DARK_THEME, onCodeThemeChanged = () => {}, onOpenAccounts = () => {} } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))
  const cliTools = $derived(tools())

  // Component-specific tokens (different from shared)
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const cardBg        = $derived(dark ? 'bg-zinc-900' : 'bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-white border-zinc-300 text-zinc-900')
  const addonBg       = $derived(dark ? 'bg-zinc-700/50 text-zinc-400' : 'bg-zinc-100 text-zinc-500')
  const buttonFocusRing = $derived(
    dark
      ? 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/40 focus-visible:ring-offset-1 focus-visible:ring-offset-zinc-900'
      : 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/35 focus-visible:ring-offset-1 focus-visible:ring-offset-white'
  )
  const fieldFocusRing = 'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand-500'

  // Registry accent -> the literal families declared in `app.css`. Tailwind
  // cannot see an interpolated class name, so each one is spelled out.
  const ACCENT_TONES = {
    emerald: { light: 'text-emerald-600', dark: 'text-emerald-300', control: 'accent-emerald-500' },
    sky: { light: 'text-sky-600', dark: 'text-sky-300', control: 'accent-sky-500' },
    'google-blue': { light: 'text-google-blue-600', dark: 'text-google-blue-300', control: 'accent-google-blue-500' },
    graphite: { light: 'text-graphite-600', dark: 'text-graphite-300', control: 'accent-graphite-500' },
  }

  /** The tool's own mark colour, dark-mode aware. */
  function toolMarkTone(toolId) {
    const tone = ACCENT_TONES[toolAccent(toolId)]
    if (!tone) return textTertiary
    return dark ? tone.dark : tone.light
  }

  /** The accent a tool's own form control carries. */
  function toolControlTone(toolId) {
    return ACCENT_TONES[toolAccent(toolId)]?.control ?? 'accent-brand-500'
  }

  // Settings state
  let settings = $state(null)
  let loading = $state(true)
  let saving = $state(false)
  let loadError = $state(null)
  let saveError = $state(null)

  // Index state
  let indexStatus = $state(null)
  let rebuilding = $state(false)
  let rebuildError = $state(null)

  // Edit modes
  let editingScanDirs = $state(false)
  let editingIgnore = $state(false)
  let scanDirsText = $state('')
  let ignoreText = $state('')

  /** One command triple per registered harness, so adding a tool is data. */
  function cloneCliCommands(source) {
    return Object.fromEntries(
      cliTools.map((descriptor) => [
        descriptor.id,
        {
          continue_cmd: source?.[descriptor.id]?.continue_cmd ?? '',
          fresh: source?.[descriptor.id]?.fresh ?? '',
          resume: source?.[descriptor.id]?.resume ?? '',
        },
      ])
    )
  }

  function createFallbackSettings(platform = 'linux') {
    const terminalContract = buildFrontendFallbackTerminalContract(platform)
    return {
      scan_directories: ['~/projects'],
      thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
      ignore_patterns: ['node_modules', '.git', 'target', 'dist'],
      code_theme: { light: DEFAULT_LIGHT_THEME, dark: DEFAULT_DARK_THEME },
      daemon: { port: 17233, path: '', auto_start: true },
      terminal: {
        emulator: terminalContract.default_emulator,
        custom_command: '',
        tmux_layout: 'new_window',
        cli_commands: cloneCliCommands(terminalContract.cli_command_defaults),
        default_account_ids: {},
        harness: { agy_hooks: true, grok_hooks: true },
      },
      terminal_contract: terminalContract,
      dark_mode: dark,
      project_dialog_last_path: '',
    }
  }

  function getTerminalContract() {
    return settings?.terminal_contract ?? buildFrontendFallbackTerminalContract('linux')
  }

  function getTerminalDefaultEmulator() {
    return getTerminalContract().default_emulator
  }

  function getSelectedTerminalEmulator() {
    const contract = getTerminalContract()
    const emulator = settings?.terminal?.emulator
    return contract.supported_emulators.includes(emulator)
      ? emulator
      : contract.default_emulator
  }

  function getTerminalCliDefaults() {
    return cloneCliCommands(getTerminalContract().cli_command_defaults)
  }

  function getTerminalPlatform() {
    return getTerminalContract().platform
  }

  function getEmulatorLabel(emulator) {
    switch (emulator) {
      case 'manual': return 'Use your existing terminal'
      case 'iterm2': return 'iTerm2'
      case 'ghostty': return 'Ghostty'
      case 'terminal_app': return 'Terminal.app'
      case 'windows_terminal': return 'Windows Terminal'
      case 'custom': return 'Custom command'
      default: return emulator
    }
  }

  // Load settings on mount
  $effect(() => {
    loadSettings()
    loadIndexStatus()
    // Detection alone: Settings names the account a launch command lands on,
    // and every usage reading it once painted belongs to the Accounts home.
    for (const tool of cliTools.filter((entry) => entry.capabilities.accountSelection)) {
      void refreshAccounts(tool.id)
    }
  })

  const accountTools = $derived(
    cliTools.filter(
      (tool) => tool.capabilities.accountSelection && accountState(tool.id).accounts.length >= 2
    )
  )

  $effect(() => {
    for (const tool of accountTools) void refreshResolvedBases(tool.id)
  })

  /** The account this user chose as the tool's global default, if any. */
  function persistedDefaultAccountId(tool) {
    return (
      settings?.terminal?.default_account_ids?.[tool] ??
      settings?.terminal?.defaultAccountIds?.[tool] ??
      null
    )
  }

  function accountLabel(account) {
    return String(account?.display_name ?? '').trim() || account?.label || account?.id || ''
  }

  /** The launch commands and selector values as the backend resolved them. */
  function launchBases(tool) {
    return accountState(tool.id).resolvedBases ?? []
  }

  /**
   * Which account a launch lands on, and why.
   *
   * Precedence is the backend's: the global default this user chose, then a
   * selector the launch command carries — through a shell alias included — and
   * only then the configured config directory. Detection marks that
   * directory's account `is_default`, which is a fact about the host rather
   * than a choice anybody made, so it cannot outrank the launch command.
   */
  function effectiveDefault(tool) {
    const state = accountState(tool.id)
    const configured = state.accounts.find(
      (account) => account.is_process_default || account.is_default
    )
    // A launch command taurhaus cannot see through decides the account itself,
    // whatever was chosen here — so the warning outranks every precedence rule
    // below, the chosen global default included.
    const selection = baseCommandSelection(launchBases(tool), state.accounts)
    if (selection.opaqueHead) {
      return { account: configured, origin: opaqueBaseNotice(selection.opaqueHead, tool.id) }
    }
    // Only a default that can run: the resolver accepts a saved default through
    // `usable()` alone and falls past a signed-out one to the launch command.
    const chosen = state.accounts.find(
      (account) => account.id === persistedDefaultAccountId(tool.id) && account.logged_in
    )
    if (chosen) return { account: chosen, origin: 'default' }
    if (selection.account && selection.usable) {
      return {
        account: selection.account,
        origin: selection.alias
          ? `from your launch command \"${selection.alias.name}\" (alias for ${selection.alias.body})`
          : `from your launch command \"${selection.command}\"`,
      }
    }
    if (selection.account && !selection.usable) {
      // The resolver rejects a signed-out selector account and falls through;
      // say so instead of presenting the dead selector as effective.
      return {
        account: configured,
        origin: `default config directory (your launch command names ${selection.account.display_name ?? selection.account.label}, which is signed out)`,
      }
    }
    return { account: configured, origin: 'default config directory' }
  }

  function focusCliCommands(tool) {
    document.querySelector(`[data-testid="cli-${tool}-fresh"]`)?.focus()
  }

  async function loadSettings() {
    loading = true
    loadError = null
    try {
      settings = await getSettings()
    } catch (e) {
      loadError = formatUserFacingError(e, 'Failed to load settings')
      // Provide defaults so the UI is still usable
      const platform = await getPlatform().catch(() => 'linux')
      settings = createFallbackSettings(platform)
    } finally {
      loading = false
    }
  }

  async function loadIndexStatus() {
    try {
      indexStatus = await getIndexStatus()
    } catch (e) {
      console.error('Failed to load index status:', e)
    }
  }

  /** Resolves to whether the write landed, for callers that must roll back. */
  async function saveSettings() {
    if (!settings) return false
    saving = true
    saveError = null
    try {
      settings = await updateSettings(settings)
      onSettingsChanged()
      return true
    } catch (e) {
      saveError = formatUserFacingError(e, 'Could not save settings. Try again.')
      console.error('Failed to save settings:', e)
      return false
    } finally {
      saving = false
    }
  }

  function handleCodeThemeChange(mode, value) {
    if (!settings.code_theme) settings.code_theme = { light: DEFAULT_LIGHT_THEME, dark: DEFAULT_DARK_THEME }
    settings.code_theme[mode] = value
    saveSettings().then(() => onCodeThemeChanged())
  }

  function handleThresholdBlur(field, value) {
    const num = parseInt(value, 10)
    if (isNaN(num) || num < 1) return
    settings.thresholds[field] = num
    saveSettings()
  }

  function startEditScanDirs() {
    scanDirsText = settings.scan_directories.join('\n')
    editingScanDirs = true
  }

  function saveScanDirs() {
    settings.scan_directories = scanDirsText
      .split('\n')
      .map(s => s.trim())
      .filter(s => s.length > 0)
    editingScanDirs = false
    saveSettings()
  }

  function startEditIgnore() {
    ignoreText = settings.ignore_patterns.join('\n')
    editingIgnore = true
  }

  function saveIgnore() {
    settings.ignore_patterns = ignoreText
      .split('\n')
      .map(s => s.trim())
      .filter(s => s.length > 0)
    editingIgnore = false
    saveSettings()
  }

  function ensureCliCommands() {
    const defaultEmulator = getTerminalDefaultEmulator()
    const cliDefaults = getTerminalCliDefaults()
    if (!settings.terminal) {
      settings.terminal = {
        emulator: defaultEmulator,
        custom_command: '',
        tmux_layout: 'new_window',
        cli_commands: cliDefaults,
        harness: { agy_hooks: true, grok_hooks: true },
      }
    }
    if (!settings.terminal.cli_commands) settings.terminal.cli_commands = cliDefaults
  }

  function getCliCmd(tool, mode) {
    return settings?.terminal?.cli_commands?.[tool]?.[mode] ?? getTerminalCliDefaults()[tool][mode]
  }

  /**
   * A saved launch command is a new question for the backend: what the pane
   * shell makes of it is resolved there, and the answer to the command it
   * replaced describes nothing any launch will run.
   */
  async function resolveSavedCommand(tool) {
    if (!cliTools.find((entry) => entry.id === tool)?.capabilities.accountSelection) return
    forgetResolvedBases(tool)
    await refreshResolvedBases(tool, { force: true })
  }

  async function setCliCmd(tool, mode, value) {
    ensureCliCommands()
    settings.terminal.cli_commands[tool][mode] = value
    if (await saveSettings()) await resolveSavedCommand(tool)
  }

  async function resetToolDefaults(tool) {
    ensureCliCommands()
    settings.terminal.cli_commands[tool] = { ...getTerminalCliDefaults()[tool] }
    if (await saveSettings()) await resolveSavedCommand(tool)
  }

  async function handleRebuildIndex() {
    rebuilding = true
    rebuildError = null
    try {
      await rebuildIndex()
      await loadIndexStatus()
    } catch (e) {
      rebuildError = formatUserFacingError(e, 'Failed to rebuild index')
    } finally {
      rebuilding = false
    }
  }

  // Keyboard: Escape closes settings
  $effect(() => {
    const handler = (e) => {
      if (e.key === 'Escape') {
        if (editingScanDirs) {
          editingScanDirs = false
        } else if (editingIgnore) {
          editingIgnore = false
        } else {
          onClose()
        }
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  })
</script>

<div class="flex-1 overflow-y-auto" data-testid="settings-view">
  <SurfaceDoorway
    {dark}
    title="Settings"
    icon={RAIL_ICONS.settings}
    backTestid="settings-back"
    onBack={onClose}
  />
  <div class="max-w-surface-reading mx-auto px-6 py-6">
    {#if loading}
      <div class="space-y-4">
        {#each Array(3) as _}
          <div class="{cardBg} rounded-lg p-4 animate-pulse">
            <div class="h-3 w-24 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'} mb-3"></div>
            <div class="h-8 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
          </div>
        {/each}
      </div>
    {:else if settings}
      <div class="space-y-4">
        {#if loadError}
          <div class="px-3 py-2 rounded-md text-[13px] {dark ? 'bg-warning-500/10 text-warning-400 border border-warning-500/20' : 'bg-warning-50 text-warning-700 border border-warning-200'}" data-testid="settings-load-error">
            Could not load saved settings — showing defaults. {loadError}
          </div>
        {/if}
        {#if saveError}
          <div class="px-3 py-2 rounded-md text-[13px] {dark ? 'bg-danger-500/10 text-danger-300 border border-danger-500/20' : 'bg-red-50 text-red-700 border border-red-200'}" data-testid="settings-save-error">
            Could not save settings. {saveError}
          </div>
        {/if}

        <!-- ═══ GENERAL ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-scanning">
          <h2 class="text-[12px] font-semibold {t.textPrimary} mb-3">General</h2>

          <!-- Scan directories -->
          <div class="mb-4">
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-[13px] {t.textSecondary}">Scan directories</span>
              {#if !editingScanDirs}
                <button
                  class="text-[12px] {t.linkColor} transition-colors {buttonFocusRing}"
                  onclick={startEditScanDirs}
                >Edit</button>
              {/if}
            </div>
            <div class="flex items-center gap-2 mb-2" data-testid="scan-directories-status">
              <span class="inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium {dark ? 'bg-success-500/10 text-success-300 border border-success-500/20' : 'bg-success-50 text-success-700 border border-success-200'}">
                Active
              </span>
              <p class="text-[12px] {textTertiary}">
                Background scanning uses this list, and search rebuilds follow the same directories.
              </p>
            </div>
            {#if editingScanDirs}
              <textarea
                class="w-full h-24 px-3 py-2 text-[13px] rounded-md border {inputBg} {fieldFocusRing} resize-none font-mono"
                bind:value={scanDirsText}
                placeholder="One directory per line"
                aria-label="Scan directories"
              ></textarea>
              <div class="flex gap-2 mt-2">
                <button
                  class="px-3 py-1 text-[12px] rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors {buttonFocusRing}"
                  onclick={saveScanDirs}
                >Save</button>
                <button
                  class="px-3 py-1 text-[12px] rounded-md {textTertiary} hover:text-zinc-600 transition-colors {buttonFocusRing}"
                  onclick={() => editingScanDirs = false}
                >Cancel</button>
              </div>
            {:else}
              {#if settings.scan_directories.length === 0}
                <p class="text-[13px] {textTertiary} italic">No directories configured</p>
              {:else}
                <div class="space-y-1">
                  {#each settings.scan_directories as dir}
                    <div class="text-[13px] {t.textBody} font-mono py-0.5">{dir}</div>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>

          <!-- Ignore patterns -->
          <div class="mb-4">
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-[13px] {t.textSecondary}">Ignore patterns</span>
              {#if !editingIgnore}
                <button
                  class="text-[12px] {t.linkColor} transition-colors {buttonFocusRing}"
                  onclick={startEditIgnore}
                >Edit</button>
              {/if}
            </div>
            <div class="flex items-center gap-2 mb-2" data-testid="ignore-patterns-status">
              <span class="inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium {dark ? 'bg-success-500/10 text-success-300 border border-success-500/20' : 'bg-success-50 text-success-700 border border-success-200'}">
                Active
              </span>
              <p class="text-[12px] {textTertiary}">
                Matching paths are skipped during scanning and search indexing.
              </p>
            </div>
            {#if editingIgnore}
              <textarea
                class="w-full h-24 px-3 py-2 text-[13px] rounded-md border {inputBg} {fieldFocusRing} resize-none font-mono"
                bind:value={ignoreText}
                placeholder="One pattern per line"
                aria-label="Ignore patterns"
              ></textarea>
              <div class="flex gap-2 mt-2">
                <button
                  class="px-3 py-1 text-[12px] rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors {buttonFocusRing}"
                  onclick={saveIgnore}
                >Save</button>
                <button
                  class="px-3 py-1 text-[12px] rounded-md {textTertiary} hover:text-zinc-600 transition-colors {buttonFocusRing}"
                  onclick={() => editingIgnore = false}
                >Cancel</button>
              </div>
            {:else}
              {#if settings.ignore_patterns.length === 0}
                <p class="text-[13px] {textTertiary} italic">No patterns configured</p>
              {:else}
                <div class="flex flex-wrap gap-1.5">
                  {#each settings.ignore_patterns as pattern}
                    <span class="text-[12px] px-2 py-0.5 rounded-md font-mono {dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-600'}">{pattern}</span>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>

          <!-- Activity thresholds -->
          <div class="pt-4 border-t {t.keyline}">
            <p class="text-[13px] {t.textSecondary} mb-3">Activity state thresholds (days since last activity)</p>
            <div class="space-y-3">
              <div class="flex items-center gap-3">
                <label for="threshold-active" class="text-[13px] {t.textBody} w-20">Active</label>
                <div class="flex items-stretch rounded-md border {dark ? 'border-zinc-700' : 'border-zinc-300'} overflow-hidden">
                  <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-r {dark ? 'border-zinc-700' : 'border-zinc-300'}">{"<"}</span>
                  <input
                    id="threshold-active"
                    type="number"
                    min="1"
                    value={settings.thresholds.active_days}
                    onblur={(e) => handleThresholdBlur('active_days', e.target.value)}
                    class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} {fieldFocusRing} text-center border-none"
                    data-testid="threshold-active"
                  />
                  <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-l {dark ? 'border-zinc-700' : 'border-zinc-300'}">days</span>
                </div>
              </div>
              <div class="flex items-center gap-3">
                <label for="threshold-recent" class="text-[13px] {t.textBody} w-20">Recent</label>
                <div class="flex items-stretch rounded-md border {dark ? 'border-zinc-700' : 'border-zinc-300'} overflow-hidden">
                  <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-r {dark ? 'border-zinc-700' : 'border-zinc-300'}">{"<"}</span>
                  <input
                    id="threshold-recent"
                    type="number"
                    min="1"
                    value={settings.thresholds.recent_days}
                    onblur={(e) => handleThresholdBlur('recent_days', e.target.value)}
                    class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} {fieldFocusRing} text-center border-none"
                    data-testid="threshold-recent"
                  />
                  <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-l {dark ? 'border-zinc-700' : 'border-zinc-300'}">days</span>
                </div>
              </div>
              <div class="flex items-center gap-3">
                <label for="threshold-stale" class="text-[13px] {t.textBody} w-20">Stale</label>
                <div class="flex items-stretch rounded-md border {dark ? 'border-zinc-700' : 'border-zinc-300'} overflow-hidden">
                  <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-r {dark ? 'border-zinc-700' : 'border-zinc-300'}">{"<"}</span>
                  <input
                    id="threshold-stale"
                    type="number"
                    min="1"
                    value={settings.thresholds.stale_days}
                    onblur={(e) => handleThresholdBlur('stale_days', e.target.value)}
                    class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} {fieldFocusRing} text-center border-none"
                    data-testid="threshold-stale"
                  />
                  <span class="px-2 py-1 text-[12px] {addonBg} flex items-center border-l {dark ? 'border-zinc-700' : 'border-zinc-300'}">days</span>
                </div>
              </div>
            </div>
          </div>

          {#if saving}
            <p class="text-[12px] {textTertiary} mt-2">Saving...</p>
          {/if}
        </section>

        <!-- ═══ DISPLAY ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-display">
          <h2 class="text-[12px] font-semibold {t.textPrimary} mb-3">Display</h2>

          <p class="text-[13px] {t.textSecondary} mb-3">Syntax highlighting</p>
          <div class="space-y-3">
            <div class="flex items-center gap-3">
              <label for="code-theme-light" class="text-[13px] {t.textBody} w-20">Light</label>
              <select
                id="code-theme-light"
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} {fieldFocusRing}"
                value={codeThemeLight}
                onchange={(e) => handleCodeThemeChange('light', e.target.value)}
                data-testid="code-theme-light"
              >
                {#each lightThemes as theme}
                  <option value={theme.id}>{theme.displayName}</option>
                {/each}
              </select>
            </div>
            <div class="flex items-center gap-3">
              <label for="code-theme-dark" class="text-[13px] {t.textBody} w-20">Dark</label>
              <select
                id="code-theme-dark"
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} {fieldFocusRing}"
                value={codeThemeDark}
                onchange={(e) => handleCodeThemeChange('dark', e.target.value)}
                data-testid="code-theme-dark"
              >
                {#each darkThemes as theme}
                  <option value={theme.id}>{theme.displayName}</option>
                {/each}
              </select>
            </div>
          </div>
        </section>

        <!-- ═══ TERMINAL & SESSIONS ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-terminal">
          <h2 class="text-[12px] font-semibold {t.textPrimary} mb-3">Terminal & sessions</h2>

          <div class="space-y-3">
            <div class="flex items-center gap-3">
              <label for="terminal-emulator" class="text-[13px] {t.textSecondary} w-24">Emulator</label>
              <select
                id="terminal-emulator"
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} {fieldFocusRing}"
                value={getSelectedTerminalEmulator()}
                onchange={(e) => {
                  const defaultEmulator = getTerminalDefaultEmulator()
                  const cliDefaults = getTerminalCliDefaults()
                  if (!settings.terminal) {
                    settings.terminal = { emulator: defaultEmulator, custom_command: '', tmux_layout: 'new_window', cli_commands: cliDefaults, harness: { agy_hooks: true, grok_hooks: true } }
                  }
                  settings.terminal.emulator = e.target.value
                  saveSettings()
                }}
                data-testid="terminal-emulator"
              >
                {#each getTerminalContract().supported_emulators as emulator}
                  <option value={emulator}>{getEmulatorLabel(emulator)}</option>
                {/each}
              </select>
            </div>

            {#if getTerminalPlatform() === 'linux'}
              <p class="text-[12px] {textTertiary}" data-testid="terminal-linux-note">
                taurhaus does not open or focus terminals on Linux. Keep working in your existing terminal alongside the app.
              </p>
            {/if}

            <div class="flex items-center gap-3">
              <label for="tmux-layout" class="text-[13px] {t.textSecondary} w-24">Pane layout</label>
              <select
                id="tmux-layout"
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} {fieldFocusRing}"
                value={settings.terminal?.tmux_layout || 'new_window'}
                onchange={(e) => {
                  const defaultEmulator = getTerminalDefaultEmulator()
                  const cliDefaults = getTerminalCliDefaults()
                  if (!settings.terminal) {
                    settings.terminal = { emulator: defaultEmulator, custom_command: '', tmux_layout: 'new_window', cli_commands: cliDefaults, harness: { agy_hooks: true, grok_hooks: true } }
                  }
                  settings.terminal.tmux_layout = e.target.value
                  saveSettings()
                }}
                data-testid="tmux-layout"
              >
                <option value="new_window">New window per session</option>
                <option value="split">Split panes (fill window, then new)</option>
                <option value="per_project">Per-project (same project shares window)</option>
              </select>
            </div>

            {#if settings.terminal?.emulator === 'custom'}
              <div>
                <label for="terminal-custom-cmd" class="text-[13px] {t.textSecondary} block mb-1.5">Custom command</label>
                <input
                  id="terminal-custom-cmd"
                  type="text"
                  class="w-full px-3 py-1.5 text-[13px] rounded-md border {inputBg} {fieldFocusRing} font-mono"
                  value={settings.terminal?.custom_command || ''}
                  placeholder={getTerminalPlatform() === 'macos'
                    ? "e.g. /usr/local/bin/alacritty -e tmux attach -t {'{tmux_session}'}"
                    : "e.g. wezterm.exe cli spawn -- wsl.exe -d {'{distro}'} -- tmux attach -t {'{tmux_session}'}"}
                  onblur={(e) => {
                    const cliDefaults = getTerminalCliDefaults()
                    if (!settings.terminal) settings.terminal = { emulator: 'custom', custom_command: '', tmux_layout: 'new_window', cli_commands: cliDefaults, harness: { agy_hooks: true, grok_hooks: true } };
                    settings.terminal.custom_command = e.target.value
                    saveSettings()
                  }}
                  data-testid="terminal-custom-cmd"
                />
                <p class="mt-1.5 text-[11px] {textTertiary}">
                  {#if getTerminalPlatform() === 'windows'}
                    Placeholders: <code class="font-mono">{'{distro}'}</code> (WSL distro name), <code class="font-mono">{'{tmux_session}'}</code> (tmux session name)
                  {:else}
                    Placeholder: <code class="font-mono">{'{tmux_session}'}</code> (tmux session name)
                  {/if}
                </p>
              </div>
            {/if}
          </div>
        </section>

        <!-- ═══ CLI TOOLS ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-cli-tools">
          <h2 class="text-[12px] font-semibold {t.textPrimary} mb-3">CLI tools</h2>
          <p class="text-[13px] {textTertiary} mb-2">Shell commands executed in tmux when launching sessions. The project directory is set automatically.</p>
          <p class="text-[12px] {textTertiary} mb-4">A resume command may carry <code class="font-mono">{'{session_id}'}</code>. taurhaus substitutes the resolved id already quoted, so leave the token bare — quoting it yourself produces a doubly quoted id the CLI cannot resume.</p>

          {#each cliTools as descriptor, toolIndex (descriptor.id)}
            {@const tool = descriptor.id}
            <div class="mb-4 last:mb-0">
              <div class="flex items-center justify-between mb-2">
                <h3 class="text-[12px] font-medium {t.textBody}">{descriptor.displayName}</h3>
                <button
                  class="text-[11px] {t.linkColor} transition-colors {buttonFocusRing}"
                  onclick={() => resetToolDefaults(tool)}
                  data-testid="cli-reset-{tool}"
                >Reset</button>
              </div>
              <div class="space-y-2">
                <div class="flex items-center gap-3">
                  <label for="cli-{tool}-continue" class="text-[12px] {t.textSecondary} w-20 shrink-0">Continue</label>
                  <input
                    id="cli-{tool}-continue"
                    type="text"
                    class="flex-1 px-2 py-1 text-[12px] rounded-md border {inputBg} {fieldFocusRing} font-mono"
                    value={getCliCmd(tool, 'continue_cmd')}
                    placeholder={getTerminalCliDefaults()[tool].continue_cmd}
                    onblur={(e) => setCliCmd(tool, 'continue_cmd', e.target.value)}
                    data-testid="cli-{tool}-continue"
                  />
                </div>
                <div class="flex items-center gap-3">
                  <label for="cli-{tool}-fresh" class="text-[12px] {t.textSecondary} w-20 shrink-0">New session</label>
                  <input
                    id="cli-{tool}-fresh"
                    type="text"
                    class="flex-1 px-2 py-1 text-[12px] rounded-md border {inputBg} {fieldFocusRing} font-mono"
                    value={getCliCmd(tool, 'fresh')}
                    placeholder={getTerminalCliDefaults()[tool].fresh}
                    onblur={(e) => setCliCmd(tool, 'fresh', e.target.value)}
                    data-testid="cli-{tool}-fresh"
                  />
                </div>
                <div class="flex items-center gap-3">
                  <label for="cli-{tool}-resume" class="text-[12px] {t.textSecondary} w-20 shrink-0">Resume</label>
                  <input
                    id="cli-{tool}-resume"
                    type="text"
                    class="flex-1 px-2 py-1 text-[12px] rounded-md border {inputBg} {fieldFocusRing} font-mono"
                    value={getCliCmd(tool, 'resume')}
                    placeholder={getTerminalCliDefaults()[tool].resume}
                    onblur={(e) => setCliCmd(tool, 'resume', e.target.value)}
                    data-testid="cli-{tool}-resume"
                  />
                </div>
              </div>
              {#if toolIndex < cliTools.length - 1}
                <div class="mt-3 border-b {t.keyline}"></div>
              {/if}
            </div>
          {/each}
        </section>

        <!-- ═══ ACCOUNTS ═══ -->
        {#if accountTools.length}
          <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-accounts">
            <div class="mb-2 flex items-center justify-between gap-3">
              <h2 class="text-[12px] font-semibold {t.textPrimary}">Accounts</h2>
              <button
                type="button"
                class="text-[11px] text-brand-500 hover:underline {buttonFocusRing}"
                onclick={onOpenAccounts}
                data-testid="settings-open-accounts"
              >Manage accounts →</button>
            </div>
            <p class="text-[12px] {t.textSecondary}">
              Defaults, pins and sign-in live in Accounts. Settings owns the launch commands, and says which account each one lands on.
            </p>
            <div class="mt-3 space-y-3">
              {#each accountTools as tool (tool.id)}
                {@const state = accountState(tool.id)}
                {@const effective = effectiveDefault(tool)}
                <div data-testid="settings-accounts-{tool.id}">
                  <div class="flex items-center justify-between">
                    <h3 class="flex items-center gap-2 text-[13px] font-semibold {t.textBody}">
                      <svg
                        class="h-[13px] w-[13px] shrink-0 {toolMarkTone(tool.id)}"
                        viewBox={getToolIcon(tool.id).viewBox}
                        fill="currentColor"
                        aria-hidden="true"
                        data-testid="tool-mark-{tool.id}"
                      >
                        <path d={getToolIcon(tool.id).path} />
                      </svg>
                      {tool.label}
                    </h3>
                    <button
                      type="button"
                      class="text-[11px] text-brand-500 hover:underline {buttonFocusRing}"
                      onclick={() => focusCliCommands(tool.id)}
                    >CLI commands</button>
                  </div>
                  <p class="mt-1 text-[11px] {textTertiary}" data-testid="effective-default-{tool.id}">
                    {#if state.resolvingBases}
                      Effective default: resolving…
                    {:else}
                      Effective default: {effective.account ? accountLabel(effective.account) : 'none'} — {effective.origin}
                    {/if}
                  </p>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- ═══ MESH ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-mesh">
          <h2 class="text-[12px] font-semibold {t.textPrimary} mb-3">Mesh</h2>
          <div class="flex items-start gap-3">
            <input
              id="agy-hooks-toggle"
              type="checkbox"
              class="mt-0.5 h-3.5 w-3.5 {toolControlTone('agy')} {fieldFocusRing}"
              checked={settings.terminal?.harness?.agy_hooks ?? true}
              onchange={(e) => {
                ensureCliCommands()
                if (!settings.terminal.harness) settings.terminal.harness = { agy_hooks: true, grok_hooks: true }
                settings.terminal.harness.agy_hooks = e.target.checked
                saveSettings()
              }}
              data-testid="agy-hooks-toggle"
            />
            <div class="min-w-0">
              <label for="agy-hooks-toggle" class="text-[13px] {t.textSecondary}">Antigravity activity hooks</label>
              <p class="mt-1 text-[11px] {textTertiary}">
                Native busy and idle signals from Antigravity. Needs agy 1.1.10 or newer, and the folder must be trusted once in the pane before hooks load.
              </p>
            </div>
          </div>
          <div class="mt-4 flex items-start gap-3 border-t {t.keyline} pt-4">
            <input
              id="grok-hooks-toggle"
              type="checkbox"
              class="mt-0.5 h-3.5 w-3.5 {toolControlTone('grok')} {fieldFocusRing}"
              checked={settings.terminal?.harness?.grok_hooks ?? true}
              onchange={(e) => {
                ensureCliCommands()
                if (!settings.terminal.harness) settings.terminal.harness = { agy_hooks: false, grok_hooks: true }
                settings.terminal.harness.grok_hooks = e.target.checked
                saveSettings()
              }}
              data-testid="grok-hooks-toggle"
            />
            <div class="min-w-0">
              <label for="grok-hooks-toggle" class="text-[13px] {t.textSecondary}">Grok compaction hooks</label>
              <p class="mt-1 text-[11px] {textTertiary}">
                Installed for managed Grok members in <code class="font-mono">~/.grok/hooks</code>, which needs no workspace trust. Turn off to leave Grok's hook directory untouched.
              </p>
            </div>
          </div>
        </section>

        <!-- ═══ SEARCH ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-index">
          <h2 class="text-[12px] font-semibold {t.textPrimary} mb-3">Search</h2>

          {#if indexStatus}
            <div class="flex items-center gap-4 mb-3">
              <div>
                <p class="text-[13px] {t.textBody}">
                  {indexStatus.doc_count} document{indexStatus.doc_count !== 1 ? 's' : ''} indexed
                </p>
              </div>
            </div>
          {/if}

          <button
            class="px-3 py-1.5 text-[13px] rounded-md border {t.keyline} {t.textSecondary} transition-colors disabled:opacity-50 {dark ? 'hover:bg-zinc-800 hover:border-zinc-700' : 'hover:bg-zinc-100 hover:border-zinc-300'} {buttonFocusRing}"
            onclick={handleRebuildIndex}
            disabled={rebuilding}
            data-testid="rebuild-index-btn"
          >
            {rebuilding ? 'Rebuilding...' : 'Rebuild index'}
          </button>
          {#if rebuildError}
            <p class="mt-2 text-[12px] text-danger-500" data-testid="rebuild-error">{rebuildError}
              <button class="ml-1 underline {buttonFocusRing}" onclick={handleRebuildIndex}>Retry</button>
            </p>
          {/if}
        </section>

      </div>
    {/if}
  </div>
</div>
