<script>
  import MarkdownRenderer from './MarkdownRenderer.svelte'
  import { getProjectContext } from './context/ProjectContext.js'
  import { getSessionContext } from './context/SessionContext.js'
  import { themeTokens } from './themeTokens.js'
  import { getToolName, TOOL_ICONS } from './toolLogos.js'
  import AccountChip from './components/AccountChip.svelte'
  import WorkflowRunsPanel from './components/WorkflowRunsPanel.svelte'
  import { getSessionsForProject } from './sessionStore.svelte.js'
  import {
    accountState,
    effectiveAccount,
    loggedInAccounts,
    refreshAccounts,
    refreshUsage,
    rememberChoice,
  } from './accounts.svelte.js'
  import { toolDescriptor, tools as registryTools } from './toolRegistry.js'

  let {
    dark = false,
    codeTheme = 'github-light',
    data = null,
    actions = {},
    onViewAllCommits,
    onDismissRelationship,
    onMarkdownNavigate,
  } = $props()

  const projectContext = getProjectContext()
  const sessionContext = getSessionContext()

  const selectedProject = $derived.by(() => data?.selectedProject ?? projectContext?.selectedProject ?? {})
  const projects = $derived.by(() => data?.projects ?? projectContext?.projects ?? [])
  const recentCommits = $derived.by(() => data?.recentCommits ?? [])
  const commitsLoading = $derived.by(() => Boolean(data?.commitsLoading))
  const latestSession = $derived.by(() => data?.latestSession ?? null)
  const sessionHistory = $derived.by(() => data?.sessionHistory ?? [])
  const sessionLoading = $derived.by(() => Boolean(data?.sessionLoading))
  const readmeContent = $derived.by(() => data?.readmeContent ?? null)
  const relationships = $derived.by(() => data?.relationships ?? [])
  const relationshipsLoading = $derived.by(() => Boolean(data?.relationshipsLoading))
  const projectSessions = $derived.by(() => getSessionsForProject(selectedProject?.path ?? ''))

  const t = $derived(themeTokens(dark))

  const accountTools = $derived(
    registryTools().filter(
      (tool) => tool.capabilities.accountSelection && accountState(tool.id).accounts.length >= 2
    )
  )

  function handleAccountSelect(tool, accountId) {
    if (!selectedProject?.id) return
    void rememberChoice(selectedProject.id, tool, accountId)
  }

  $effect(() => {
    for (const tool of registryTools().filter((entry) => entry.capabilities.accountSelection)) {
      void refreshAccounts(tool.id)
    }
  })

  const statusColor    = $derived(dark ? 'text-success-400' : 'text-success-600')
  const dangerColor    = $derived(dark ? 'text-danger-400/70 hover:text-danger-400' : 'text-danger-600/60 hover:text-danger-600')
  const hashColor      = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')
  const timeColor      = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')
  const dashBorder     = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const sessionTint    = $derived(dark ? 'bg-brand-500/[0.03]' : 'bg-brand-50/40')
  const sessionBorder  = $derived(dark ? 'border-brand-400' : 'border-brand-500')
  const tagBg          = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-600')
  const actionBtnBase  = $derived(dark
    ? 'bg-zinc-800/60 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 border-zinc-700/50'
    : 'bg-zinc-50 hover:bg-zinc-100 text-zinc-500 hover:text-zinc-700 border-zinc-200')

  let showAllCommits = $state(false)
  const commitsRevealKey = $derived.by(() => {
    if (commitsLoading) return null
    return `${selectedProject?.id ?? 'none'}:${recentCommits.length}:${showAllCommits ? 'all' : 'top'}`
  })
  const sessionsRevealKey = $derived.by(() => {
    if (sessionLoading) return null
    const latestMarker = latestSession?.date ?? latestSession?.summary ?? 'none'
    return `${selectedProject?.id ?? 'none'}:${latestMarker}:${sessionHistory.length}`
  })
  const relationshipsRevealKey = $derived.by(() => {
    if (relationshipsLoading) return null
    return `${selectedProject?.id ?? 'none'}:${relationships.length}`
  })

  const readmeForOverview = $derived.by(() => {
    if (!readmeContent?.content) return ''
    return readmeContent.content.replace(/^#\s+[^\n]*\n?/, '')
  })

  function formatSessionDate(dateStr) {
    if (!dateStr) return ''
    const d = new Date(dateStr)
    const now = new Date()
    const diffMs = now - d
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))
    if (diffDays === 0) return 'Today'
    if (diffDays === 1) return 'Yesterday'
    if (diffDays < 7) return `${diffDays} days ago`
    return d.toLocaleDateString()
  }

  function getRelatedProjectName(rel) {
    const otherId = rel.source_project_id === selectedProject?.id
      ? rel.target_project_id
      : rel.source_project_id
    const p = projects.find(p => p.id === otherId)
    return p?.name || otherId
  }

  function getRelationshipDirection(rel) {
    return rel.source_project_id === selectedProject?.id ? 'outgoing' : 'incoming'
  }

  const DETECTION_SOURCE_LABELS = {
    cargo_toml: 'via Cargo.toml',
    package_json: 'via package.json',
    claude_md: 'via CLAUDE.md',
    session_mention: 'via session',
    gitmodules: 'via .gitmodules',
    manual: 'manual',
  }

  const RELATIONSHIP_TYPE_LABELS = {
    depends_on: 'depends on',
    references: 'references',
    mentioned_in_session: 'mentioned in',
    includes: 'includes',
    workspace_sibling: 'sibling of',
  }

  function handleViewAllCommits() {
    showAllCommits = true
    onViewAllCommits?.()
  }

  function handleNavigateToCommit(hash) {
    actions?.onNavigateToCommit?.(hash)
    projectContext?.navigateToCommit?.(hash)
  }

  function handleSelectProject(project) {
    actions?.onSelectProject?.(project)
    projectContext?.selectProject?.(project)
  }

  /**
   * A quick action launches on whatever the project already decided; holding a
   * modifier asks instead. Shift is the documented one, and Ctrl/Cmd is the
   * reflex a lot of people reach for first, so both open the chooser.
   */
  function handleLaunchSession(tool, event) {
    const choose = event?.shiftKey || event?.ctrlKey || event?.metaKey ? 'always' : 'auto'
    actions?.onLaunchSession?.(tool, { choose })
    sessionContext?.launchSession?.(tool, { choose })
  }

  /** Only a host with a second signed-in subscription has a choice to offer. */
  function canChooseAccount(tool) {
    return Boolean(
      toolDescriptor(tool)?.capabilities.accountSelection && loggedInAccounts(tool).length >= 2
    )
  }

  function launchTitle(tool) {
    const name = getToolName(tool)
    return canChooseAccount(tool)
      ? `Launch ${name} (Shift+click to choose the account)`
      : name
  }

  function handleOpenTerminal() {
    actions?.onOpenTerminal?.()
    sessionContext?.openTerminal?.()
  }

  const TOOLS = ['claude', 'codex', 'agy', 'grok']
</script>

<!-- Project header -->
<div class="px-7 pt-5 pb-4 shrink-0">
  <div class="flex items-center gap-3">
    <h1 class="text-[18px] font-semibold {t.textPrimary} tracking-[-0.02em]">{selectedProject.name}</h1>
    <span class="text-[11px] font-mono {t.textTertiary} self-baseline">{selectedProject.branch || ''}</span>
    {#if selectedProject.isDirty}
      <span class="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0" title="Uncommitted changes"></span>
    {/if}
    {#if selectedProject.activityState}
      <span class="text-[11px] {statusColor} font-medium capitalize self-baseline">{selectedProject.activityState}</span>
    {/if}
    {#each accountTools as tool (tool.id)}
      {@const state = accountState(tool.id)}
      {@const effective = effectiveAccount(selectedProject, tool.id)}
      <AccountChip
        tool={tool.id}
        accounts={state.accounts}
        selectedAccountId={effective.account?.id ?? null}
        defaultAccountId={state.defaultAccountId}
        degraded={state.degraded}
        origin={effective.origin}
        {dark}
        onSelect={(accountId) => handleAccountSelect(tool.id, accountId)}
        onRequestUsage={() => void refreshUsage(tool.id)}
      />
    {/each}
    <!-- Quick actions — compact icon buttons -->
    <div class="ml-auto flex items-center gap-1 shrink-0" data-testid="quick-actions">
      {#each TOOLS as tool}
        {@const icon = TOOL_ICONS[tool]}
        <button
          class="w-7 h-7 flex items-center justify-center rounded-md transition-colors {actionBtnBase}"
          onclick={(event) => handleLaunchSession(tool, event)}
          title={launchTitle(tool)}
          data-testid="action-launch-{tool}"
        >
          <svg class="w-3.5 h-3.5 shrink-0" viewBox={icon.viewBox} fill="currentColor">
            <path d={icon.path}/>
          </svg>
        </button>
      {/each}
      <button
        class="w-7 h-7 flex items-center justify-center rounded-md transition-colors {actionBtnBase}"
        onclick={handleOpenTerminal}
        title="Terminal"
        data-testid="action-open-terminal"
      >
        <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z" />
        </svg>
      </button>
    </div>
  </div>
  {#if selectedProject.description}
    <p class="mt-0.5 text-[13px] {t.textTertiary}">{selectedProject.description}</p>
  {/if}
</div>

<!-- Scrollable content -->
<div class="flex-1 overflow-y-auto">
  <div class="max-w-3xl px-7 pb-8">

    <!-- 1. README — project identity, immediately below header -->
    {#if readmeContent}
      <section class="pb-5 border-b {t.keyline}" data-testid="overview-readme">
        <span class="text-[11px] {t.textTertiary} mb-3 block">README</span>
        <MarkdownRenderer source={readmeForOverview} {dark} {codeTheme} projectId={selectedProject?.id} onNavigate={onMarkdownNavigate} />
      </section>
    {/if}

    <!-- 2. Recent Activity (commits) -->
    <section class="py-5 border-b {t.keyline}">
      <div class="flex items-center justify-between mb-3">
        <span class="text-[11px] {t.textTertiary}">Recent activity</span>
        {#if recentCommits.length > 0}
          <span class="text-[11px] {t.textTertiary}">{recentCommits.length} commit{recentCommits.length !== 1 ? 's' : ''}</span>
        {/if}
      </div>
      {#if commitsLoading}
        <div class="space-y-1" data-testid="commits-loading">
          {#each Array(3) as _}
            <div class="flex items-center h-[30px]">
              <div class="h-2.5 w-12 rounded bg-zinc-200 dark:bg-zinc-800 animate-pulse"></div>
              <div class="h-2.5 flex-1 rounded bg-zinc-100 dark:bg-zinc-800/50 animate-pulse ml-3"></div>
            </div>
          {/each}
        </div>
      {:else if commitsRevealKey}
        {#key commitsRevealKey}
          <div class="content-enter">
            {#if recentCommits.length === 0}
              <p class="text-[13px] {t.textMuted}">No commits found.</p>
            {:else}
              <div>
                {#each recentCommits as commit}
                  <button
                    class="w-full flex items-center h-[30px] text-[13px] text-left {t.hoverRow} -mx-2 px-2 rounded transition-colors cursor-pointer"
                    onclick={() => handleNavigateToCommit(commit.hash)}
                    data-testid="overview-commit-row"
                  >
                    <span class="font-mono text-[11px] {hashColor} w-[58px] shrink-0">{commit.hash}</span>
                    <span class="{t.textBody} truncate flex-1">{commit.message}</span>
                    <span class="text-[11px] {timeColor} shrink-0 ml-3">{commit.date}</span>
                  </button>
                {/each}
              </div>
              {#if !showAllCommits}
                <button
                  data-testid="view-all-commits"
                  class="mt-1 text-[11px] {t.textTertiary} hover:underline"
                  onclick={handleViewAllCommits}
                >View all &rarr;</button>
              {/if}
            {/if}
          </div>
        {/key}
      {/if}
    </section>

    <!-- 3. Sessions — combined, hidden when empty -->
    {#if sessionLoading || latestSession || sessionHistory.length > 0}
      <section class="py-5 border-b {t.keyline}" data-testid="overview-sessions">
        <div class="flex items-center justify-between mb-3">
          <span class="text-[11px] {t.textTertiary}">Sessions</span>
          {#if sessionHistory.length > 0}
            <span class="text-[11px] {t.textTertiary}">{sessionHistory.length} session{sessionHistory.length !== 1 ? 's' : ''}</span>
          {/if}
        </div>
        {#if sessionLoading}
          <div class="space-y-2 animate-pulse" data-testid="sessions-loading">
            <div class="h-3 w-3/4 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
            <div class="h-3 w-1/2 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
          </div>
        {:else if sessionsRevealKey}
          {#key sessionsRevealKey}
            <div class="content-enter">
              {#if latestSession}
                <div class="border-l-[3px] {sessionBorder} pl-5 py-3 -ml-0.5 rounded-r-sm {sessionTint} mb-3">
                  <div class="flex items-center justify-between mb-1">
                    <span class="text-[11px] font-medium {t.textTertiary}">Latest</span>
                    <span class="text-[11px] {t.textTertiary}">{formatSessionDate(latestSession.date)}</span>
                  </div>
                  <p class="text-[13px] {t.textBody}">{latestSession.summary}</p>
                  {#if latestSession.next_steps && latestSession.next_steps.length > 0}
                    <div class="mt-3">
                      <span class="text-[11px] {t.textTertiary}">Next steps</span>
                      <ul class="mt-1 space-y-0.5">
                        {#each latestSession.next_steps as step}
                          <li class="text-[13px] {t.textBody} flex items-start gap-2">
                            <span class="text-[10px] {t.textTertiary} mt-1 shrink-0">&#9656;</span>
                            <span>{step}</span>
                          </li>
                        {/each}
                      </ul>
                    </div>
                  {/if}
                  {#if latestSession.open_questions && latestSession.open_questions.length > 0}
                    <div class="mt-3">
                      <span class="text-[11px] {t.textTertiary}">Open questions</span>
                      <ul class="mt-1 space-y-0.5">
                        {#each latestSession.open_questions as question}
                          <li class="text-[13px] {t.textBody} flex items-start gap-2">
                            <span class="text-[10px] {t.questionMark} mt-1 shrink-0">?</span>
                            <span>{question}</span>
                          </li>
                        {/each}
                      </ul>
                    </div>
                  {/if}
                </div>
              {/if}
              {#if sessionHistory.length > 0}
                <div>
                  {#each sessionHistory as session}
                    <div class="flex items-start gap-3 py-1.5 {t.hoverRow} -mx-2 px-2 rounded">
                      <span class="text-[11px] {t.textTertiary} shrink-0 w-[72px] pt-0.5">{formatSessionDate(session.date)}</span>
                      <span class="text-[13px] {t.textBody} flex-1">{session.summary}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/key}
        {/if}
      </section>
    {/if}

    <!-- 4. Workflow runs — hidden when the project's sessions ran none -->
    <WorkflowRunsPanel projectId={selectedProject?.id ?? ''} sessions={projectSessions} {dark} />

    <!-- 5. Relationships — hidden when empty -->
    {#if relationshipsLoading || relationships.length > 0}
      <section class="py-5 border-b {t.keyline}" data-testid="overview-relationships">
        <div class="flex items-center justify-between mb-3">
          <span class="text-[11px] {t.textTertiary}">Relationships</span>
          {#if relationships.length > 0}
            <span class="text-[11px] {t.textTertiary}">{relationships.length} connection{relationships.length !== 1 ? 's' : ''}</span>
          {/if}
        </div>
        {#if relationshipsLoading}
          <div class="space-y-1" data-testid="relationships-loading">
            {#each Array(2) as _}
              <div class="flex items-center h-[30px]">
                <div class="h-2.5 w-4 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
                <div class="h-2.5 w-24 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-3"></div>
                <div class="h-2.5 w-16 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-3"></div>
              </div>
            {/each}
          </div>
        {:else if relationshipsRevealKey}
          {#key relationshipsRevealKey}
            <div class="content-enter">
              {#each relationships as rel}
                {@const direction = getRelationshipDirection(rel)}
                {@const projectName = getRelatedProjectName(rel)}
                {@const typeLabel = RELATIONSHIP_TYPE_LABELS[rel.relationship_type] || rel.relationship_type}
                {@const sourceLabel = DETECTION_SOURCE_LABELS[rel.detection_source] || rel.detection_source}
                <div class="flex items-center h-[30px] text-[13px] {t.hoverRow} -mx-2 px-2 rounded group" data-testid="relationship-row">
                  <span class="w-5 text-center shrink-0 {t.textTertiary}" title={direction === 'outgoing' ? 'outgoing' : 'incoming'}>{direction === 'outgoing' ? '\u2192' : '\u2190'}</span>

                  <button
                    class="text-[13px] {t.linkColor} truncate transition-colors"
                    onclick={() => {
                      const otherId = direction === 'outgoing' ? rel.target_project_id : rel.source_project_id
                      const p = projects.find(pr => pr.id === otherId)
                      if (p) handleSelectProject(p)
                    }}
                  >{projectName}</button>

                  <span class="ml-2 px-1.5 py-0.5 text-[10px] rounded {tagBg} shrink-0">{typeLabel}</span>

                  <span class="ml-2 text-[10px] {t.textTertiary} shrink-0">{sourceLabel}</span>

                  {#if rel.detection_source !== 'manual'}
                    <button
                      class="ml-auto opacity-0 group-hover:opacity-100 w-5 h-5 flex items-center justify-center rounded {t.textMuted} hover:{t.textSecondary} transition-all shrink-0"
                      onclick={() => onDismissRelationship(rel.id)}
                      aria-label="Dismiss relationship"
                      data-testid="dismiss-relationship"
                    >
                      <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/>
                      </svg>
                    </button>
                  {/if}
                </div>
              {/each}
            </div>
          {/key}
        {/if}
      </section>
    {/if}

    <!-- 6. Project Info -->
    <section class="py-5 pb-10">
      <span class="text-[11px] {t.textTertiary}">Project info</span>
      <div class="mt-2 space-y-1 text-[13px]">
        <div class="flex items-center gap-3">
          <span class="{t.textTertiary} w-14">Path</span>
          <span class="font-mono text-[12px] {t.textMuted}">{selectedProject.path}</span>
        </div>
        {#if selectedProject.createdAt}
          <div class="flex items-center gap-3">
            <span class="{t.textTertiary} w-14">Created</span>
            <span class="text-[12px] {t.textMuted}">{new Date(selectedProject.createdAt).toLocaleDateString()}</span>
          </div>
        {/if}
      </div>
    </section>

  </div>
</div>
