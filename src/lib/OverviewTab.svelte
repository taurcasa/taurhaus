<script>
  import MarkdownRenderer from './MarkdownRenderer.svelte'
  import { themeTokens } from './themeTokens.js'

  let {
    dark,
    codeTheme,
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
    onNavigateToCommit,
    onViewAllCommits,
    onDismissRelationship,
    onSelectProject,
    onMarkdownNavigate,
  } = $props()

  const t = $derived(themeTokens(dark))

  const statusColor    = $derived(dark ? 'text-success-400' : 'text-success-600')
  const dangerColor    = $derived(dark ? 'text-danger-400/70 hover:text-danger-400' : 'text-danger-600/60 hover:text-danger-600')
  const hashColor      = $derived(dark ? 'text-zinc-600' : 'text-zinc-400')
  const timeColor      = $derived(dark ? 'text-zinc-700' : 'text-zinc-300')
  const dashBorder     = $derived(dark ? 'border-zinc-800' : 'border-zinc-200')
  const sessionTint    = $derived(dark ? 'bg-brand-500/[0.03]' : 'bg-brand-50/40')
  const sessionBorder  = $derived(dark ? 'border-brand-400' : 'border-brand-500')
  const tagBg          = $derived(dark ? 'bg-zinc-800 text-zinc-400' : 'bg-zinc-100 text-zinc-600')

  let heroMode = $state('auto')
  let showAllCommits = $state(false)

  const showSession = $derived(
    heroMode === 'session' ||
    (heroMode === 'auto' && latestSession && isSessionFresh(latestSession.date))
  )
  const showReadme = $derived(!showSession)
  const hasToggle = $derived(latestSession && readmeContent)

  const readmeForOverview = $derived.by(() => {
    if (!readmeContent?.content) return ''
    return readmeContent.content.replace(/^#\s+[^\n]*\n?/, '')
  })

  function isSessionFresh(dateStr) {
    if (!dateStr) return false
    const sessionDate = new Date(dateStr)
    const now = new Date()
    const diffDays = (now - sessionDate) / (1000 * 60 * 60 * 24)
    return diffDays < 7
  }

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
    onViewAllCommits()
  }
</script>

<!-- Project header -->
<div class="px-7 pt-5 pb-4 shrink-0 content-enter">
  <div class="flex items-baseline gap-3">
    <h1 class="text-[18px] font-semibold {t.textPrimary} tracking-[-0.02em]">{selectedProject.name}</h1>
    <span class="text-[11px] font-mono {t.textTertiary}">{selectedProject.branch || ''}</span>
    {#if selectedProject.activity_state}
      <span class="text-[11px] {statusColor} font-medium capitalize">{selectedProject.activity_state}</span>
    {/if}
  </div>
  {#if selectedProject.description}
    <p class="mt-0.5 text-[13px] {t.textTertiary}">{selectedProject.description}</p>
  {/if}
</div>

<!-- Scrollable content -->
<div class="flex-1 overflow-y-auto content-enter">
  <div class="max-w-3xl px-7 pb-8">

    <!-- Hero area: Session / README toggle (ADR-006) -->
    <section class="pb-6 border-b {t.keyline}">
      <div class="flex items-center justify-between mb-3">
        {#if hasToggle}
          <!-- Segmented control -->
          <div class="flex items-center gap-0.5 rounded-md p-0.5 {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'}">
            <button
              class="px-2.5 py-0.5 text-[11px] rounded transition-colors
                {showSession ? `font-medium ${dark ? 'bg-zinc-700 text-zinc-200' : 'bg-white text-zinc-700 shadow-sm'}` : `${t.textTertiary} hover:${t.textSecondary}`}"
              onclick={() => heroMode = 'session'}
            >Session</button>
            <button
              class="px-2.5 py-0.5 text-[11px] rounded transition-colors
                {showReadme ? `font-medium ${dark ? 'bg-zinc-700 text-zinc-200' : 'bg-white text-zinc-700 shadow-sm'}` : `${t.textTertiary} hover:${t.textSecondary}`}"
              onclick={() => heroMode = 'readme'}
            >README</button>
          </div>
        {:else}
          <span class="text-[11px] {t.textTertiary}">{latestSession ? 'Latest session' : readmeContent ? 'README' : 'Latest session'}</span>
        {/if}
        {#if latestSession}
          <span class="text-[11px] {t.textTertiary}">{formatSessionDate(latestSession.date)}</span>
        {/if}
      </div>

      {#if sessionLoading}
        <div class="border-l-[3px] {sessionBorder} pl-5 py-3 -ml-0.5 rounded-r-sm {sessionTint}">
          <div class="space-y-2 animate-pulse">
            <div class="h-3 w-3/4 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
            <div class="h-3 w-1/2 rounded {dark ? 'bg-zinc-700' : 'bg-zinc-200'}"></div>
          </div>
        </div>
      {:else if hasToggle}
        {#if showSession}
          <div class="border-l-[3px] {sessionBorder} pl-5 py-3 -ml-0.5 rounded-r-sm {sessionTint}">
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
                      <span class="text-[10px] text-amber-500 mt-1 shrink-0">?</span>
                      <span>{question}</span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          </div>
        {/if}
        <div class:hidden={showSession}>
          <MarkdownRenderer source={readmeForOverview} {dark} {codeTheme} projectId={selectedProject?.id} onNavigate={onMarkdownNavigate} />
        </div>
      {:else if latestSession}
        <div class="border-l-[3px] {sessionBorder} pl-5 py-3 -ml-0.5 rounded-r-sm {sessionTint}">
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
                    <span class="text-[10px] text-amber-500 mt-1 shrink-0">?</span>
                    <span>{question}</span>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        </div>
      {:else if readmeContent}
        <MarkdownRenderer source={readmeForOverview} {dark} {codeTheme} projectId={selectedProject?.id} onNavigate={onMarkdownNavigate} />
      {:else}
        <div class="border-l-[3px] {dashBorder} pl-5 py-3 -ml-0.5 rounded-r-sm">
          <p class="text-[13px] {t.textMuted}">No sessions or README found for this project.</p>
        </div>
      {/if}
    </section>

    <!-- Recent Activity (commits) -->
    <section class="py-6 border-b {t.keyline}">
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
      {:else if recentCommits.length === 0}
        <p class="text-[13px] {t.textMuted}">No commits found.</p>
      {:else}
        <div>
          {#each recentCommits as commit}
            <button
              class="w-full flex items-center h-[30px] text-[13px] text-left {t.hoverRow} -mx-2 px-2 rounded transition-colors cursor-pointer"
              onclick={() => onNavigateToCommit(commit.hash)}
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
            class="mt-1 text-[11px] {t.textTertiary} hover:underline"
            onclick={handleViewAllCommits}
          >View all &rarr;</button>
        {/if}
      {/if}
    </section>

    <!-- Relationships -->
    <section class="py-6 border-b {t.keyline}">
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
      {:else if relationships.length === 0}
        <p class="text-[13px] {t.textMuted}">No connections detected yet.</p>
      {:else}
        <div>
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
                  if (p) onSelectProject(p)
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
      {/if}
    </section>

    <!-- Session History -->
    <section class="py-6 border-b {t.keyline}">
      <div class="flex items-center justify-between mb-3">
        <span class="text-[11px] {t.textTertiary}">Session history</span>
        {#if sessionHistory.length > 0}
          <span class="text-[11px] {t.textTertiary}">{sessionHistory.length} session{sessionHistory.length !== 1 ? 's' : ''}</span>
        {/if}
      </div>
      {#if sessionLoading}
        <div class="space-y-1" data-testid="sessions-loading">
          {#each Array(3) as _}
            <div class="flex items-center h-[30px]">
              <div class="h-2.5 w-16 rounded {dark ? 'bg-zinc-800' : 'bg-zinc-200'} animate-pulse"></div>
              <div class="h-2.5 flex-1 rounded {dark ? 'bg-zinc-800/50' : 'bg-zinc-100'} animate-pulse ml-3"></div>
            </div>
          {/each}
        </div>
      {:else if sessionHistory.length === 0}
        <p class="text-[13px] {t.textMuted}">No sessions imported yet.</p>
      {:else}
        <div>
          {#each sessionHistory as session}
            <div class="flex items-start gap-3 py-1.5 {t.hoverRow} -mx-2 px-2 rounded">
              <span class="text-[11px] {t.textTertiary} shrink-0 w-[72px] pt-0.5">{formatSessionDate(session.date)}</span>
              <span class="text-[13px] {t.textBody} flex-1">{session.summary}</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Project Info -->
    <section class="py-6 pb-10">
      <span class="text-[11px] {t.textTertiary}">Project info</span>
      <div class="mt-2 space-y-1 text-[13px]">
        <div class="flex items-center gap-3">
          <span class="{t.textTertiary} w-14">Path</span>
          <span class="font-mono text-[12px] {t.textMuted}">{selectedProject.path}</span>
        </div>
        {#if selectedProject.created_at}
          <div class="flex items-center gap-3">
            <span class="{t.textTertiary} w-14">Created</span>
            <span class="text-[12px] {t.textMuted}">{new Date(selectedProject.created_at).toLocaleDateString()}</span>
          </div>
        {/if}
      </div>
      <div class="mt-3 flex gap-3">
        <button class="text-[11px] {t.textTertiary}">Edit</button>
        <button class="text-[11px] {dangerColor}">Remove</button>
      </div>
    </section>

  </div>
</div>
