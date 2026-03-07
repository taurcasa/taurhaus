<script>
  let {
    sidebarLoading = false,
    sidebarError = null,
    projects = [],
    filteredProjects = [],
    filterQuery = '',
    useVirtualizedSidebar = false,
    sidebarRows = [],
    sidebarWindow = { start: 0, end: 0, paddingTop: 0, paddingBottom: 0 },
    selectedProject = null,
    ctxMenuProjectId = null,
    getSessionsForProject = () => [],
    toolIndicators = () => [],
    rowTintForSessions = () => '',
    onProjectClick = () => {},
    onProjectContextMenu = () => {},
    onProjectMouseEnter = () => {},
    onProjectMouseLeave = () => {},
    onSessionJump = () => {},
    onRetry = () => {},
    onOpenManageProjects = () => {},
  } = $props()

  function hasTmuxTarget(session) {
    return Boolean(session?.tmux_session && session?.tmux_window && session?.tmux_pane)
  }

  function isLeadMember(member) {
    return String(member?.role ?? '').trim().toLowerCase() === 'lead'
  }

  function groupedSessionTarget(indicator) {
    if (!Array.isArray(indicator?.members) || indicator.members.length === 0) {
      return null
    }

    return indicator.members.find(member => isLeadMember(member) && hasTmuxTarget(member))
      || indicator.members.find(hasTmuxTarget)
      || null
  }

  function handleGroupedSessionJump(event, indicator) {
    event.stopPropagation()
    const target = groupedSessionTarget(indicator)
    if (target) {
      onSessionJump(event, target)
    }
  }
</script>

{#if sidebarLoading}
  <div class="px-3 pt-3 space-y-1" data-testid="sidebar-skeleton">
    {#each Array(5) as _}
      <div class="flex items-center gap-2 h-[34px] px-3">
        <div class="w-[7px] h-[7px] rounded-full bg-white/[0.06] animate-pulse"></div>
        <div class="h-3 rounded bg-white/[0.06] animate-pulse flex-1"></div>
      </div>
    {/each}
  </div>
{:else if sidebarError}
  <div class="px-4 pt-6 text-center" data-testid="sidebar-error">
    <p class="text-[12px] text-white/40">{sidebarError}</p>
    <button
      class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors"
      onclick={onRetry}
    >Retry</button>
  </div>
{:else if projects.length === 0}
  <div class="px-4 pt-8 text-center" data-testid="sidebar-empty">
    <svg class="w-10 h-10 text-white/10 mx-auto" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"/></svg>
    <p class="mt-2 text-[12px] text-white/40">No projects yet</p>
    <button
      class="mt-2 text-[12px] text-brand-400 hover:text-brand-300 transition-colors"
      onclick={onOpenManageProjects}
      data-testid="sidebar-empty-scan"
    >Scan for projects</button>
  </div>
{:else if filteredProjects.length === 0 && filterQuery}
  <div class="px-4 pt-6 text-center" data-testid="sidebar-no-matches">
    <p class="text-[12px] text-white/30">No matching projects</p>
  </div>
{:else}
  {#if useVirtualizedSidebar}
    <div style="height: {sidebarWindow.paddingTop}px;"></div>
  {/if}

  {#each sidebarRows.slice(sidebarWindow.start, sidebarWindow.end) as row (row.key)}
    {#if row.type === 'header'}
      <div class="px-3.5 pt-8 pb-1.5">
        <span class="text-[10px] font-semibold uppercase tracking-[0.06em] text-white/35">{row.group.label}</span>
      </div>
    {:else}
      {@const project = row.project}
      {@const selected = selectedProject && project.id === selectedProject.id}
      {@const projectSessions = getSessionsForProject(project.path)}
      {@const indicators = toolIndicators(projectSessions)}
      <button
        data-testid="project-item"
        class="w-full flex items-center gap-2 px-3 h-[36px] rounded-md text-left transition-all duration-75 cursor-pointer
          {selected ? 'bg-white/[0.08]' : ctxMenuProjectId === project.id ? 'bg-white/[0.08]' : `hover:bg-white/[0.04] ${rowTintForSessions(projectSessions)}`}"
        onclick={() => onProjectClick(project)}
        oncontextmenu={(e) => onProjectContextMenu(e, project, projectSessions)}
        onmouseenter={(e) => onProjectMouseEnter(project, projectSessions, e.currentTarget)}
        onmouseleave={onProjectMouseLeave}
      >
        {#if selected}
          <span class="w-[3px] h-3.5 bg-brand-400 rounded-full shrink-0 -ml-1 mr-0.5"></span>
        {/if}
        <span class="text-[14px] truncate flex-1 {selected ? 'font-medium text-white' : 'text-white/75'}">{project.name}</span>
        {#if indicators.length > 0}
          <span class="flex items-center gap-1 shrink-0">
            {#each indicators as ind}
              {#if ind.kind === 'team'}
                <span
                  class={ind.layout === 'stack'
                    ? 'sidebar-session-team sidebar-session-team-stack shrink-0'
                    : 'sidebar-session-team sidebar-session-team-rail shrink-0'}
                  data-activity={ind.tone}
                  aria-label={ind.ariaLabel}
                  data-testid="sidebar-team-indicator"
                  role="button"
                  tabindex="0"
                  onclick={(event) => handleGroupedSessionJump(event, ind)}
                  onkeydown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault()
                      handleGroupedSessionJump(event, ind)
                    }
                  }}
                >
                  {#if ind.layout === 'stack'}
                    <span class="sidebar-session-team-stack-logos" aria-hidden="true">
                      {#each ind.tools as tool (tool.tool)}
                        <span
                          class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {tool.colorClass} {tool.isActive ? 'session-pill-active' : 'session-pill-idle'}"
                        >
                          <svg class="w-[12px] h-[12px]" viewBox={tool.icon.viewBox} fill="currentColor">
                            <path d={tool.icon.path}></path>
                          </svg>
                        </span>
                      {/each}
                    </span>
                    <span class="sidebar-session-team-count" aria-hidden="true">{ind.count}</span>
                  {:else}
                    {#each ind.memberTools as memberTool, index (`${ind.groupId}:${memberTool.tool}:${index}`)}
                      <span
                        class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {memberTool.colorClass} {memberTool.isActive ? 'session-pill-active' : 'session-pill-idle'}"
                        aria-hidden="true"
                      >
                        <svg class="w-[12px] h-[12px]" viewBox={memberTool.icon.viewBox} fill="currentColor">
                          <path d={memberTool.icon.path}></path>
                        </svg>
                      </span>
                    {/each}
                  {/if}
                </span>
              {:else if ind.interactive}
                <span
                  class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center cursor-pointer {ind.colorClass} {ind.isActive ? 'session-pill-active' : 'session-pill-idle'}"
                  role="button"
                  tabindex="0"
                  aria-label={ind.ariaLabel}
                  onclick={(e) => onSessionJump(e, ind.session)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      onSessionJump(e, ind.session)
                    }
                  }}
                >
                  <svg class="w-[12px] h-[12px]" viewBox={ind.icon.viewBox} fill="currentColor" aria-hidden="true">
                    <path d={ind.icon.path}></path>
                  </svg>
                </span>
              {:else}
                <span
                  class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {ind.colorClass} {ind.isActive ? 'session-pill-active' : 'session-pill-idle'}"
                  aria-label={ind.ariaLabel}
                >
                  <svg class="w-[12px] h-[12px]" viewBox={ind.icon.viewBox} fill="currentColor" aria-hidden="true">
                    <path d={ind.icon.path}></path>
                  </svg>
                </span>
              {/if}
            {/each}
          </span>
        {/if}
        {#if project.branch}
          <span class="text-[10px] font-mono shrink-0 px-1.5 py-0.5 rounded {selected ? 'text-white/50 bg-white/10' : 'text-white/30 bg-white/[0.07]'}">{project.branch}</span>
        {/if}
        {#if project.isDirty}
          <span class="w-[5px] h-[5px] rounded-full bg-warning-400 shrink-0"></span>
        {/if}
      </button>
    {/if}
  {/each}

  {#if useVirtualizedSidebar}
    <div style="height: {sidebarWindow.paddingBottom}px;"></div>
  {/if}
{/if}
