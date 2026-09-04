<script>
  import { isContextMenuKey } from './a11y.js'
  import { workflowBadge } from './workflowRuns.js'

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
    foregroundProjectId = null,
    utilityOpen = false,
    dark = false,
    ctxMenuProjectId = null,
    getSessionsForProject = () => [],
    toolIndicators = () => [],
    rowTintForSessions = () => '',
    onProjectClick = () => {},
    onProjectContextMenu = () => {},
    onProjectContextMenuKey = () => {},
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

  function handleGroupedSessionJump(event, indicator, project) {
    event.stopPropagation()
    const target = groupedSessionTarget(indicator)
    if (target) {
      onSessionJump(event, target, project)
    }
  }

  function normalizedBranch(branch) {
    return typeof branch === 'string' ? branch.trim() : ''
  }

  function isDefaultBranch(branch) {
    const value = normalizedBranch(branch).toLowerCase()
    return value === 'main' || value === 'master' || value === 'develop'
  }

  function branchLine(project) {
    const branch = normalizedBranch(project?.branch)
    if (!branch || isDefaultBranch(branch)) {
      return null
    }
    return branch
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
      <!-- Drawer guide card: a sentence-case tab chip sitting on a hairline
           that runs to the rail edge, carrying the group's project count.
           Fixed 42px tall — the virtualized layout counts on it. -->
      <div class="h-[42px] flex items-end pl-1 -mr-1.5" data-testid="sidebar-group-header">
        <div class="flex-1 flex items-end border-b border-white/[0.07]">
          <span class="sidebar-guide-tab">
            <span>{row.group.label}</span>
            <span class="sidebar-guide-count">{row.group.items.length}</span>
          </span>
        </div>
      </div>
    {:else}
      {@const project = row.project}
      {@const selected = selectedProject && project.id === selectedProject.id}
      {@const pulled = Boolean(selected && !utilityOpen)}
      {@const foregroundActive = foregroundProjectId && project.id === foregroundProjectId}
      {@const projectSessions = getSessionsForProject(project.path)}
      {@const indicators = toolIndicators(projectSessions)}
      {@const workflow = workflowBadge(projectSessions)}
      {@const secondaryBranch = branchLine(project)}
      <!-- Selected rows speak the drawer language: pulled (panel material,
           edge scoops) while the panel shows this project; held (quiet fill)
           while a utility surface occupies the panel. The 3px selection
           handle survives both. The transition is colors-only so the pulled
           width/radius and the scoops change in the same instant instead of
           the corners popping in over an animating edge. -->
      <button
        data-testid="project-item"
        data-project-id={project.id}
        class="w-full px-3 rounded-md text-left transition-colors duration-75 cursor-pointer
          focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-brand-500/70 focus-visible:ring-inset
          {secondaryBranch ? 'h-[50px] py-1.5' : 'h-[36px]'}
          {pulled
            ? 'sidebar-row-pulled'
            : selected
              ? 'bg-white/[0.06] overflow-hidden'
              : ctxMenuProjectId === project.id
                ? 'bg-white/[0.08] overflow-hidden'
                : `hover:bg-white/[0.04] overflow-hidden ${rowTintForSessions(projectSessions)}`} relative"
        onclick={() => onProjectClick(project)}
        oncontextmenu={(e) => onProjectContextMenu(e, project, projectSessions)}
        onkeydown={(e) => {
          if (!isContextMenuKey(e)) return
          onProjectContextMenuKey(e, project, projectSessions, e.currentTarget)
        }}
        onmouseenter={(e) => onProjectMouseEnter(project, projectSessions, e.currentTarget)}
        onmouseleave={onProjectMouseLeave}
      >
        <span class="flex w-full min-w-0 items-start gap-2">
          {#if selected}
            <span
              data-testid="sidebar-selection-indicator"
              class="mt-2 w-[3px] h-3.5 bg-brand-400 rounded-full shrink-0 -ml-1 mr-0.5"
            ></span>
          {/if}
          <span class="min-w-0 flex-1">
            <span class="flex min-w-0 items-center gap-2">
              <span class="sidebar-row-name text-[14px] truncate flex-1 min-w-0 {selected ? 'font-medium text-white' : 'text-white/75'}">{project.name}</span>
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
                        onclick={(event) => handleGroupedSessionJump(event, ind, project)}
                        onkeydown={(event) => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault()
                            handleGroupedSessionJump(event, ind, project)
                          }
                        }}
                      >
                        {#if ind.layout === 'stack'}
                          <span class="sidebar-session-team-stack-logos" aria-hidden="true">
                            {#each ind.tools as tool (tool.tool)}
                              <span
                                class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {tool.colorClass} {tool.toneClass ?? (tool.isActive ? 'session-pill-active' : 'session-pill-idle')}"
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
                              class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {memberTool.colorClass} {memberTool.toneClass ?? (memberTool.isActive ? 'session-pill-active' : 'session-pill-idle')}"
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
                        class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center cursor-pointer {ind.colorClass} {ind.toneClass ?? (ind.isActive ? 'session-pill-active' : 'session-pill-idle')}"
                        role="button"
                        tabindex="0"
                        aria-label={ind.ariaLabel}
                        onclick={(e) => onSessionJump(e, ind.session, project)}
                        onkeydown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault()
                            onSessionJump(e, ind.session, project)
                          }
                        }}
                      >
                        <svg class="w-[12px] h-[12px]" viewBox={ind.icon.viewBox} fill="currentColor" aria-hidden="true">
                          <path d={ind.icon.path}></path>
                        </svg>
                      </span>
                    {:else}
                      <span
                        class="w-[14px] h-[14px] shrink-0 inline-flex items-center justify-center {ind.colorClass} {ind.toneClass ?? (ind.isActive ? 'session-pill-active' : 'session-pill-idle')}"
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
              {#if workflow.visible}
                <span
                  class="sidebar-workflow-badge shrink-0"
                  data-testid="sidebar-workflow-badge"
                  aria-label={workflow.ariaLabel}
                  title={workflow.title}
                >
                  <svg class="w-[8px] h-[8px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" aria-hidden="true">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 3v6a3 3 0 0 0 3 3h9M6 21v-6a3 3 0 0 1 3-3h9"/>
                  </svg>
                  {workflow.label}
                </span>
              {/if}
              {#if project.isDirty}
                <span class="sidebar-dirty-dot w-[5px] h-[5px] rounded-full bg-warning-400 shrink-0"></span>
              {/if}
            </span>
            {#if secondaryBranch}
              <span
                data-testid="sidebar-branch-line"
                class="sidebar-branch-line mt-0.5 block min-w-0 truncate pl-0.5 text-[10px] font-mono {selected ? 'text-white/35' : 'text-white/20'}"
              >⑂ {secondaryBranch}</span>
            {/if}
          </span>
        </span>
        {#if foregroundActive}
          <span
            data-testid="sidebar-foreground-indicator"
            class="sidebar-foreground-lines pointer-events-none absolute left-2 right-2 top-0 h-[2px] bg-brand-400"
            aria-hidden="true"
          ></span>
          <span
            class="sidebar-foreground-lines pointer-events-none absolute left-2 right-2 bottom-0 h-[2px] bg-brand-400"
            aria-hidden="true"
          ></span>
        {/if}
        {#if pulled}
          <span class="sidebar-row-scoop sidebar-row-scoop-top" aria-hidden="true"></span>
          <span class="sidebar-row-scoop sidebar-row-scoop-bottom" aria-hidden="true"></span>
        {/if}
      </button>
    {/if}
  {/each}

  {#if useVirtualizedSidebar}
    <div style="height: {sidebarWindow.paddingBottom}px;"></div>
  {/if}
{/if}
