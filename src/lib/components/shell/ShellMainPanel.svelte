<script>
  import FilesTab from '../../FilesTab.svelte'
  import GitTab from '../../GitTab.svelte'
  import OverviewTab from '../../OverviewTab.svelte'
  import Settings from '../../Settings.svelte'
  import AccountsHome from '../AccountsHome.svelte'
  import TaskBoard from '../../TaskBoard.svelte'
  import { describeProjectLoadBanner } from '../../errorCopy.js'
  import { themeTokens } from '../../themeTokens.js'
  import MeshTab from '../MeshTab.svelte'

  let {
    dark = false,
    codeTheme,
    codeThemeLight,
    codeThemeDark,
    settingsOpen = false,
    accountsOpen = false,
    accountStates = null,
    requestedAddTool = null,
    daemonStatus = null,
    daemonRecoveryEscalated = false,
    daemonUpdateAvailable = null,
    daemonUpdateDismissed = false,
    daemonUpdating = false,
    daemonRestarting = false,
    shellNotice = null,
    projectLoadIssues = [],
    selectedProject = null,
    projects = [],
    activeTab = 'overview',
    visitedTabs = new Set(['overview']),
    recentCommits = [],
    commitsLoading = false,
    latestSession = null,
    sessionHistory = [],
    sessionLoading = false,
    readmeContent = null,
    relationships = [],
    relationshipsLoading = false,
    gitNavTarget = null,
    filesNavTarget = null,
    taskNavTarget = null,
    fileChangePaths = null,
    filesPosition = $bindable(null),
    gitPosition = $bindable(null),
    taskPosition = $bindable(null),
    onCloseSettings = () => {},
    onCloseAccounts = () => {},
    onOpenProject = () => {},
    onRequestedAddConsumed = () => {},
    onSettingsChanged = () => {},
    onCodeThemeChanged = () => {},
    onViewAllCommits = () => {},
    onDismissRelationship = () => {},
    onMarkdownNavigate = () => {},
    onRetryProjectLoad = () => {},
    onHandleDaemonUpdate = () => {},
    onRestartDaemon = () => {},
    onDismissDaemonUpdate = () => {},
    onDismissProjectLoadIssues = () => {},
    onDismissShellNotice = () => {},
    onNavigateToFile = () => {},
    onMeshFocusPane = () => {},
    onClearTaskNavTarget = () => {},
    onClearGitNavTarget = () => {},
    onClearFilesNavTarget = () => {},
    onChangedPathsConsumed = () => {},
  } = $props()

  const t = $derived(themeTokens(dark))
  const projectLoadBannerMessage = $derived(describeProjectLoadBanner(projectLoadIssues))
</script>

<main class="shell-main-surface shell-main-panel flex-1 {t.textBody} rounded-b-lg rounded-tr-lg flex flex-col min-w-0 overflow-hidden">
  {#if (daemonStatus === 'busy' || daemonStatus === 'reconnecting' || daemonStatus === 'disconnected' || daemonStatus === 'failed') && !settingsOpen}
    <div
      class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-brand-500/10 border-b border-brand-500/20' : 'bg-brand-50 border-b border-brand-200'}"
      role="status"
      aria-live="polite"
      data-testid="daemon-connecting-banner"
    >
      <svg class="h-4 w-4 shrink-0 text-brand-500 animate-pulse" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
        <path fill-rule="evenodd" d="M10 18a8 8 0 1 0-5.657-2.343l1.414-1.414A6 6 0 1 1 10 16v2Zm1-11V4H9v5h5V7h-3Z" clip-rule="evenodd" />
      </svg>
      <span class="flex-1 text-[12px] {t.textSecondary}">
        {#if daemonStatus === 'busy'}
          The helper service is busy with another request. Live updates may be delayed for a moment.
        {:else if daemonStatus === 'failed'}
          The helper service stopped responding. Restart it to restore live updates.
        {:else if daemonRecoveryEscalated}
          Still trying to reconnect to the helper service. You can keep working, or restart it now.
        {:else}
          Reconnecting to the helper service. Live updates may be delayed for a moment.
        {/if}
      </span>
      {#if daemonStatus === 'disconnected' || daemonStatus === 'failed' || daemonRecoveryEscalated}
        <button
          class="text-[12px] font-medium text-brand-500 hover:text-brand-400 transition-colors disabled:opacity-50"
          onclick={onRestartDaemon}
          disabled={daemonRestarting}
          data-testid="daemon-restart-button"
        >{daemonRestarting ? 'Restarting...' : 'Restart helper'}</button>
      {/if}
    </div>
  {/if}

  {#if daemonUpdateAvailable && !daemonUpdateDismissed && !settingsOpen}
    <div class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-warning-500/10 border-b border-warning-500/20' : 'bg-warning-50 border-b border-warning-200'}" role="status" aria-live="polite" data-testid="daemon-update-banner">
      <svg class="w-4 h-4 text-warning-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z"/></svg>
      <span class="text-[12px] {t.textSecondary} flex-1">
        Helper service update available: v{daemonUpdateAvailable.version} → v{daemonUpdateAvailable.bundled_version}
      </span>
      <button
        class="text-[12px] font-medium text-brand-500 hover:text-brand-400 transition-colors disabled:opacity-50"
        onclick={onHandleDaemonUpdate}
        disabled={daemonUpdating}
        data-testid="daemon-update-button"
      >{daemonUpdating ? 'Updating...' : 'Update now'}</button>
      <button
        class="text-[12px] {t.textTertiary} hover:text-white/60 transition-colors"
        onclick={onDismissDaemonUpdate}
        data-testid="daemon-update-dismiss"
      >Dismiss</button>
    </div>
  {/if}

  {#if projectLoadIssues.length > 0 && !settingsOpen}
    <div class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-red-500/10 border-b border-red-500/20' : 'bg-red-50 border-b border-red-200'}" role="status" aria-live="polite" data-testid="project-load-degraded-banner">
      <svg class="w-4 h-4 text-red-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z"/></svg>
      <span class="text-[12px] {t.textSecondary} flex-1" data-testid="project-load-degraded-message">
        {projectLoadBannerMessage}
      </span>
      <button
        class="text-[12px] font-medium text-brand-500 hover:text-brand-400 transition-colors"
        onclick={onRetryProjectLoad}
        data-testid="project-load-retry"
      >Retry</button>
      <button
        class="text-[12px] {t.textTertiary} hover:text-white/60 transition-colors"
        onclick={onDismissProjectLoadIssues}
        data-testid="project-load-dismiss"
      >Dismiss</button>
    </div>
  {/if}

  {#if shellNotice && !settingsOpen}
    <div class="flex items-center gap-3 px-4 py-2 {dark ? 'bg-warning-500/10 border-b border-warning-500/20' : 'bg-warning-50 border-b border-warning-200'}" role="status" aria-live="polite" data-testid="shell-notice-banner">
      <svg class="w-4 h-4 text-warning-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m0 3.75h.007M4.93 19.5h14.14c1.54 0 2.502-1.667 1.732-3L13.732 4.25c-.77-1.333-2.694-1.333-3.464 0L3.198 16.5c-.77 1.333.192 3 1.732 3Z"/></svg>
      <span class="text-[12px] {t.textSecondary} flex-1" data-testid="shell-notice-message">{shellNotice}</span>
      <button
        class="text-[12px] {t.textTertiary} hover:text-white/60 transition-colors"
        onclick={onDismissShellNotice}
        data-testid="shell-notice-dismiss"
      >Dismiss</button>
    </div>
  {/if}

  {#if accountsOpen}
    <AccountsHome
      {dark}
      {projects}
      {selectedProject}
      states={accountStates}
      {requestedAddTool}
      {onRequestedAddConsumed}
      onClose={onCloseAccounts}
      {onOpenProject}
    />
  {:else if settingsOpen}
    <Settings
      {dark}
      onClose={onCloseSettings}
      onSettingsChanged={onSettingsChanged}
      {codeThemeLight}
      {codeThemeDark}
      onCodeThemeChanged={onCodeThemeChanged}
    />
  {:else if !selectedProject}
    <div class="flex-1 flex items-center justify-center">
      <p class="text-[13px] {t.textTertiary}">Select a project</p>
    </div>
  {:else}
    {#key selectedProject.id}
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden content-enter" data-testid="content-wrapper">
        <div
          id="shell-panel-overview"
          class="flex-1 flex flex-col min-h-0 overflow-hidden"
          role="tabpanel"
          aria-labelledby="shell-tab-overview"
          hidden={activeTab !== 'overview'}
          class:hidden={activeTab !== 'overview'}
        >
          <OverviewTab
            {dark}
            {codeTheme}
            data={{
              selectedProject,
              projects,
              recentCommits,
              commitsLoading,
              latestSession,
              sessionHistory,
              sessionLoading,
              readmeContent,
              relationships,
              relationshipsLoading,
            }}
            onViewAllCommits={onViewAllCommits}
            onDismissRelationship={onDismissRelationship}
            onMarkdownNavigate={onMarkdownNavigate}
          />
        </div>

        <div
          id="shell-panel-tasks"
          class="flex-1 flex min-h-0 overflow-hidden"
          role="tabpanel"
          aria-labelledby="shell-tab-tasks"
          hidden={activeTab !== 'tasks'}
          class:hidden={activeTab !== 'tasks'}
        >
          {#if visitedTabs.has('tasks')}
            <TaskBoard
              projectId={selectedProject.id}
              projectPath={selectedProject.path}
              isActive={activeTab === 'tasks'}
              {dark}
              {codeTheme}
              bind:position={taskPosition}
              navTarget={taskNavTarget}
              onClearNavTarget={onClearTaskNavTarget}
            />
          {/if}
        </div>

        <div
          id="shell-panel-mesh"
          class="flex-1 flex min-h-0 overflow-hidden"
          role="tabpanel"
          aria-labelledby="shell-tab-mesh"
          hidden={activeTab !== 'mesh'}
          class:hidden={activeTab !== 'mesh'}
        >
          {#if visitedTabs.has('mesh')}
            <MeshTab
              {dark}
              projectPath={selectedProject.path}
              projectId={selectedProject.id}
              availableProjects={projects}
              onFocusPane={onMeshFocusPane}
            />
          {/if}
        </div>

        <div
          id="shell-panel-git"
          class="flex-1 flex min-h-0 overflow-hidden"
          role="tabpanel"
          aria-labelledby="shell-tab-git"
          hidden={activeTab !== 'git'}
          class:hidden={activeTab !== 'git'}
        >
          {#if visitedTabs.has('git')}
            <GitTab
              projectPath={selectedProject.path}
              projectId={selectedProject.id}
              {dark}
              navTarget={gitNavTarget}
              bind:position={gitPosition}
              onNavigateToFile={onNavigateToFile}
              onClearNavTarget={onClearGitNavTarget}
            />
          {/if}
        </div>

        <div
          id="shell-panel-files"
          class="flex-1 flex min-h-0 overflow-hidden"
          role="tabpanel"
          aria-labelledby="shell-tab-files"
          hidden={activeTab !== 'files'}
          class:hidden={activeTab !== 'files'}
        >
          {#if visitedTabs.has('files')}
            <FilesTab
              {dark}
              {codeTheme}
              {selectedProject}
              isActive={activeTab === 'files'}
              navTarget={filesNavTarget}
              onClearNavTarget={onClearFilesNavTarget}
              bind:position={filesPosition}
              onMarkdownNavigate={onMarkdownNavigate}
              changedPaths={fileChangePaths}
              onChangedPathsConsumed={onChangedPathsConsumed}
            />
          {/if}
        </div>
      </div>
    {/key}
  {/if}
</main>
