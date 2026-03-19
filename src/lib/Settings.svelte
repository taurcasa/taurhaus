<script>
  import {
    getSettings,
    updateSettings,
    getIndexStatus,
    rebuildIndex,
    getPlatform,
  } from './ipc.js'
  import { buildFrontendFallbackTerminalContract } from './ipc/system.js'
  import { lightThemes, darkThemes, DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME } from './shikiThemes.js'
  import { formatUserFacingError } from './format.js'
  import { themeTokens } from './themeTokens.js'

  let { dark = false, onClose = () => {}, onSettingsChanged = () => {}, codeThemeLight = DEFAULT_LIGHT_THEME, codeThemeDark = DEFAULT_DARK_THEME, onCodeThemeChanged = () => {} } = $props()

  // Shared theme tokens
  const t = $derived(themeTokens(dark))

  // Component-specific tokens (different from shared)
  const textTertiary  = $derived(dark ? 'text-zinc-500' : 'text-zinc-400')
  const cardBg        = $derived(dark ? 'bg-zinc-900' : 'bg-zinc-50')
  const inputBg       = $derived(dark ? 'bg-zinc-800 border-zinc-700 text-zinc-200' : 'bg-white border-zinc-300 text-zinc-900')
  const addonBg       = $derived(dark ? 'bg-zinc-700/50 text-zinc-400' : 'bg-zinc-100 text-zinc-500')

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

  function cloneCliCommands(source) {
    return {
      claude: {
        continue_cmd: source?.claude?.continue_cmd ?? '',
        fresh: source?.claude?.fresh ?? '',
        resume: source?.claude?.resume ?? '',
      },
      codex: {
        continue_cmd: source?.codex?.continue_cmd ?? '',
        fresh: source?.codex?.fresh ?? '',
        resume: source?.codex?.resume ?? '',
      },
      gemini: {
        continue_cmd: source?.gemini?.continue_cmd ?? '',
        fresh: source?.gemini?.fresh ?? '',
        resume: source?.gemini?.resume ?? '',
      },
    }
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
  })

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

  async function saveSettings() {
    if (!settings) return
    saving = true
    saveError = null
    try {
      settings = await updateSettings(settings)
      onSettingsChanged()
    } catch (e) {
      saveError = formatUserFacingError(e, 'Could not save settings. Try again.')
      console.error('Failed to save settings:', e)
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
      }
    }
    if (!settings.terminal.cli_commands) settings.terminal.cli_commands = cliDefaults
  }

  function getCliCmd(tool, mode) {
    return settings?.terminal?.cli_commands?.[tool]?.[mode] ?? getTerminalCliDefaults()[tool][mode]
  }

  function setCliCmd(tool, mode, value) {
    ensureCliCommands()
    settings.terminal.cli_commands[tool][mode] = value
    saveSettings()
  }

  function resetToolDefaults(tool) {
    ensureCliCommands()
    settings.terminal.cli_commands[tool] = { ...getTerminalCliDefaults()[tool] }
    saveSettings()
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
  <div class="max-w-[640px] mx-auto px-6 py-6">
    <!-- Header -->
    <div class="mb-6">
      <button
        class="text-[13px] {t.linkColor} transition-colors mb-3 flex items-center gap-1"
        onclick={onClose}
        data-testid="settings-back"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5"/></svg>
        Back to projects
      </button>
      <h1 class="text-[20px] font-semibold {t.textPrimary}">Settings</h1>
    </div>

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
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {t.labelColor} mb-3">General</h2>

          <!-- Scan directories -->
          <div class="mb-4">
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-[13px] {t.textSecondary}">Scan directories</span>
              {#if !editingScanDirs}
                <button
                  class="text-[12px] {t.linkColor} transition-colors"
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
                class="w-full h-24 px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 resize-none font-mono"
                bind:value={scanDirsText}
                placeholder="One directory per line"
              ></textarea>
              <div class="flex gap-2 mt-2">
                <button
                  class="px-3 py-1 text-[12px] rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors"
                  onclick={saveScanDirs}
                >Save</button>
                <button
                  class="px-3 py-1 text-[12px] rounded-md {textTertiary} hover:text-zinc-600 transition-colors"
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
                  class="text-[12px] {t.linkColor} transition-colors"
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
                class="w-full h-24 px-3 py-2 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 resize-none font-mono"
                bind:value={ignoreText}
                placeholder="One pattern per line"
              ></textarea>
              <div class="flex gap-2 mt-2">
                <button
                  class="px-3 py-1 text-[12px] rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors"
                  onclick={saveIgnore}
                >Save</button>
                <button
                  class="px-3 py-1 text-[12px] rounded-md {textTertiary} hover:text-zinc-600 transition-colors"
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
                    class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} focus:outline-none focus:ring-1 focus:ring-brand-500 text-center border-none"
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
                    class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} focus:outline-none focus:ring-1 focus:ring-brand-500 text-center border-none"
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
                    class="w-14 px-2 py-1 text-[13px] {dark ? 'bg-zinc-800 text-zinc-200' : 'bg-white text-zinc-900'} focus:outline-none focus:ring-1 focus:ring-brand-500 text-center border-none"
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
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {t.labelColor} mb-3">Display</h2>

          <p class="text-[13px] {t.textSecondary} mb-3">Syntax highlighting</p>
          <div class="space-y-3">
            <div class="flex items-center gap-3">
              <label for="code-theme-light" class="text-[13px] {t.textBody} w-20">Light</label>
              <select
                id="code-theme-light"
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500"
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
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500"
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
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {t.labelColor} mb-3">Terminal & Sessions</h2>

          <div class="space-y-3">
            <div class="flex items-center gap-3">
              <label for="terminal-emulator" class="text-[13px] {t.textSecondary} w-24">Emulator</label>
              <select
                id="terminal-emulator"
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500"
                value={getSelectedTerminalEmulator()}
                onchange={(e) => {
                  const defaultEmulator = getTerminalDefaultEmulator()
                  const cliDefaults = getTerminalCliDefaults()
                  if (!settings.terminal) {
                    settings.terminal = { emulator: defaultEmulator, custom_command: '', tmux_layout: 'new_window', cli_commands: cliDefaults }
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
                class="flex-1 px-2 py-1 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500"
                value={settings.terminal?.tmux_layout || 'new_window'}
                onchange={(e) => {
                  const defaultEmulator = getTerminalDefaultEmulator()
                  const cliDefaults = getTerminalCliDefaults()
                  if (!settings.terminal) {
                    settings.terminal = { emulator: defaultEmulator, custom_command: '', tmux_layout: 'new_window', cli_commands: cliDefaults }
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
                  class="w-full px-3 py-1.5 text-[13px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
                  value={settings.terminal?.custom_command || ''}
                  placeholder={getTerminalPlatform() === 'macos'
                    ? "e.g. /usr/local/bin/alacritty -e tmux attach -t {'{tmux_session}'}"
                    : "e.g. wezterm.exe cli spawn -- wsl.exe -d {'{distro}'} -- tmux attach -t {'{tmux_session}'}"}
                  onblur={(e) => {
                    const cliDefaults = getTerminalCliDefaults()
                    if (!settings.terminal) settings.terminal = { emulator: 'custom', custom_command: '', tmux_layout: 'new_window', cli_commands: cliDefaults };
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
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {t.labelColor} mb-3">CLI Tools</h2>
          <p class="text-[13px] {textTertiary} mb-4">Shell commands executed in tmux when launching sessions. The project directory is set automatically.</p>

          {#each [['claude', 'Claude Code'], ['codex', 'Codex'], ['gemini', 'Gemini CLI']] as [tool, label]}
            <div class="mb-4 last:mb-0">
              <div class="flex items-center justify-between mb-2">
                <h3 class="text-[12px] font-medium {t.textBody}">{label}</h3>
                <button
                  class="text-[11px] {t.linkColor} transition-colors"
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
                    class="flex-1 px-2 py-1 text-[12px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
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
                    class="flex-1 px-2 py-1 text-[12px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
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
                    class="flex-1 px-2 py-1 text-[12px] rounded-md border {inputBg} focus:outline-none focus:ring-1 focus:ring-brand-500 font-mono"
                    value={getCliCmd(tool, 'resume')}
                    placeholder={getTerminalCliDefaults()[tool].resume}
                    onblur={(e) => setCliCmd(tool, 'resume', e.target.value)}
                    data-testid="cli-{tool}-resume"
                  />
                </div>
              </div>
              {#if tool !== 'gemini'}
                <div class="mt-3 border-b {t.keyline}"></div>
              {/if}
            </div>
          {/each}
        </section>

        <!-- ═══ SEARCH ═══ -->
        <section class="{cardBg} rounded-lg border {t.keyline} p-4" data-testid="settings-index">
          <h2 class="text-[11px] font-semibold uppercase tracking-wider {t.labelColor} mb-3">Search</h2>

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
            class="px-3 py-1.5 text-[13px] rounded-md border {t.keyline} {t.textSecondary} transition-colors disabled:opacity-50 {dark ? 'hover:bg-zinc-800 hover:border-zinc-700' : 'hover:bg-zinc-100 hover:border-zinc-300'}"
            onclick={handleRebuildIndex}
            disabled={rebuilding}
            data-testid="rebuild-index-btn"
          >
            {rebuilding ? 'Rebuilding...' : 'Rebuild index'}
          </button>
          {#if rebuildError}
            <p class="mt-2 text-[12px] text-danger-500" data-testid="rebuild-error">{rebuildError}
              <button class="ml-1 underline" onclick={handleRebuildIndex}>Retry</button>
            </p>
          {/if}
        </section>

      </div>
    {/if}
  </div>
</div>
