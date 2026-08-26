<script>
  import { getLatestSession, getRecentCommits, getRelationships } from './ipc.js'
  import { formatDuration } from './format.js'
  import { groupedSessionIndicators, hasLiveSession, sessionBadge, toolIcon } from './sessionIndicator.js'
  import { activitySignal, isActiveLevel } from './activitySignal.js'

  const FRESH_SESSION_WINDOW_MS = 7 * 24 * 60 * 60 * 1000
  const DEFAULT_WIDTH = 312
  const COMPACT_WIDTH = 288
  const COMPACT_BREAKPOINT = 1180
  const VIEWPORT_INSET = 12
  const CARD_GAP = 10

  let {
    project = null,
    sessions = [],
    anchorEl = null,
    dark = false,
    visible = true,
  } = $props()

  let cardEl = $state(null)
  let posX = $state(0)
  let posY = $state(0)
  let compactWidth = $state(false)
  let latestSession = $state(null)
  let latestCommit = $state(null)
  let relationships = $state([])
  let entered = $state(false)

  /** Presented activity level → hover-card tone. */
  const LEVEL_TONE = {
    working: 'active',
    active: 'active',
    idle: 'waiting',
    uncertain: 'recent',
    offline: 'quiet',
  }

  /**
   * Team header pill, keyed by the aggregate tone `sessionIndicator.js` derives.
   * A team whose members are all retained-stale is not an idle team: it wears
   * the same uncertain/info tone its member rows do.
   */
  const teamBadgeClasses = $derived.by(() => ({
    active: 'bg-success-300/18 text-success-300 border border-success-300/55',
    stale: 'bg-info-300/14 text-info-300 border border-info-300/45',
    idle: 'bg-warning-300/18 text-warning-300 border border-warning-300/65',
  }))

  const toneClasses = $derived.by(() => ({
    active: dark ? 'text-success-400' : 'text-success-600',
    waiting: dark ? 'text-warning-300' : 'text-warning-600',
    recent: dark ? 'text-info-300' : 'text-info-600',
    quiet: dark ? 'text-zinc-400' : 'text-zinc-500',
    neutral: dark ? 'text-zinc-100' : 'text-zinc-900',
  }))

  const ui = $derived.by(() => ({
    card: dark
      ? 'border-white/[0.08] bg-brand-950/96 shadow-[0_14px_34px_rgba(0,0,0,0.34)]'
      : 'border-brand-900/10 bg-white/96 shadow-[0_12px_28px_rgba(15,23,42,0.14)]',
    branchChip: dark
      ? 'bg-white/[0.04] text-zinc-400 border border-white/[0.08]'
      : 'bg-zinc-50 text-zinc-500 border border-zinc-200',
    dirtyChip: dark
      ? 'bg-warning-400/12 text-warning-300 border border-warning-400/20'
      : 'bg-warning-50 text-warning-600 border border-warning-500/20',
    evidenceRow: dark
      ? 'border-white/[0.06] bg-white/[0.03]'
      : 'border-brand-900/10 bg-brand-50/35',
    relationshipDivider: dark ? 'border-white/[0.07]' : 'border-brand-900/10',
    relationshipChip: dark
      ? 'bg-brand-400/10 text-brand-300 border border-brand-400/20'
      : 'bg-brand-50 text-brand-700 border border-brand-600/15',
    secondaryText: dark ? 'text-zinc-400' : 'text-zinc-500',
    mutedText: dark ? 'text-zinc-500' : 'text-zinc-400',
    bodyText: dark ? 'text-zinc-300' : 'text-zinc-700',
    quietText: dark ? 'text-zinc-500' : 'text-zinc-400',
    titleText: dark ? 'text-zinc-100' : 'text-zinc-900',
    relationshipBodyClamp: compactWidth ? 'line-clamp-1' : 'line-clamp-2',
    whyNowClamp: compactWidth ? 'line-clamp-1' : 'line-clamp-2',
  }))

  const liveSessions = $derived.by(() => (sessions || []).filter((session) => hasLiveSession(session)))
  const prioritizedSessions = $derived.by(() => [...liveSessions].sort(compareSessions))
  const groupedTeams = $derived.by(() => groupedSessionIndicators(prioritizedSessions))
  const primarySession = $derived.by(() => prioritizedSessions[0] ?? null)
  const extraSessionCount = $derived.by(() => Math.max(0, prioritizedSessions.length - 1))
  const freshLatestSession = $derived.by(() => isFreshSession(latestSession))
  const unresolvedItem = $derived.by(() => buildUnresolvedItem(latestSession))
  const latestChange = $derived.by(() => buildLatestChange(latestSession, latestCommit, freshLatestSession))
  const relationshipCue = $derived.by(() => summarizeRelationship(relationships, project?.id))
  const motionRow = $derived.by(() => buildMotionRow(primarySession, extraSessionCount))
  const verdict = $derived.by(() => buildVerdict({
    project,
    primarySession,
    latestSession,
    freshLatestSession,
    unresolvedItem,
  }))
  const verdictToneClass = $derived.by(() => toneClasses[verdict.tone] ?? toneClasses.neutral)
  const motionToneClass = $derived.by(() => toneClasses[motionRow.tone] ?? toneClasses.neutral)
  const cardWidthClass = $derived.by(() => (compactWidth ? 'w-[288px]' : 'w-[312px]'))

  $effect(() => {
    if (!project?.id) {
      latestSession = null
      latestCommit = null
      relationships = []
      return
    }

    latestSession = null
    latestCommit = null
    relationships = []
    const projectId = project.id
    let cancelled = false

    getLatestSession(projectId)
      .then((session) => {
        if (!cancelled && project?.id === projectId) latestSession = session
      })
      .catch(() => {
        if (!cancelled && project?.id === projectId) latestSession = null
      })

    getRecentCommits(projectId, 1)
      .then((commits) => {
        if (!cancelled && project?.id === projectId) {
          latestCommit = Array.isArray(commits) ? commits[0] ?? null : null
        }
      })
      .catch(() => {
        if (!cancelled && project?.id === projectId) latestCommit = null
      })

    getRelationships(projectId)
      .then((rels) => {
        if (!cancelled && project?.id === projectId) {
          relationships = Array.isArray(rels) ? rels : []
        }
      })
      .catch(() => {
        if (!cancelled && project?.id === projectId) relationships = []
      })

    return () => {
      cancelled = true
    }
  })

  $effect(() => {
    entered = false
    const timer = window.setTimeout(() => {
      entered = visible
    }, 0)
    return () => window.clearTimeout(timer)
  })

  $effect(() => {
    if (!anchorEl || !project) return

    const updatePosition = () => {
      const vw = window.innerWidth
      const vh = window.innerHeight
      const anchor = anchorEl.getBoundingClientRect()
      const desiredCompact = vw < COMPACT_BREAKPOINT
        || (
          anchor.right + CARD_GAP + DEFAULT_WIDTH > vw - VIEWPORT_INSET
          && anchor.left - CARD_GAP - DEFAULT_WIDTH < VIEWPORT_INSET
        )
      compactWidth = desiredCompact

      const desiredWidth = desiredCompact ? COMPACT_WIDTH : DEFAULT_WIDTH
      const cardWidth = Math.min(desiredWidth, Math.max(0, vw - VIEWPORT_INSET * 2))
      const cardHeight = cardEl?.getBoundingClientRect().height ?? 176
      const fitsRight = anchor.right + CARD_GAP + cardWidth <= vw - VIEWPORT_INSET
      const fitsLeft = anchor.left - CARD_GAP - cardWidth >= VIEWPORT_INSET

      let x = anchor.right + CARD_GAP
      if (!fitsRight && fitsLeft) {
        x = anchor.left - cardWidth - CARD_GAP
      } else if (!fitsRight) {
        x = Math.max(VIEWPORT_INSET, vw - cardWidth - VIEWPORT_INSET)
      }

      let y = anchor.top + anchor.height / 2 - cardHeight / 2
      if (y + cardHeight > vh - VIEWPORT_INSET) {
        y = vh - cardHeight - VIEWPORT_INSET
      }
      if (y < VIEWPORT_INSET) y = VIEWPORT_INSET

      posX = x
      posY = y
    }

    updatePosition()
    window.addEventListener('resize', updatePosition)
    return () => window.removeEventListener('resize', updatePosition)
  })

  function compareSessions(left, right) {
    return sessionPriority(right) - sessionPriority(left)
  }

  function sessionPriority(session) {
    if (!session) return 0
    const level = activitySignal(session).level
    if (isActiveLevel(level)) return 3
    if (level === 'uncertain') return 2
    if (level === 'idle') return 1
    return 0
  }

  function isFreshSession(session) {
    if (!session) return false
    const date = getSessionDate(session)
    if (!date) return false
    return Date.now() - date.getTime() < FRESH_SESSION_WINDOW_MS
  }

  function getSessionDate(session) {
    const raw = session?.date ?? session?.created_at ?? null
    if (!raw) return null
    const parsed = new Date(raw)
    return Number.isNaN(parsed.getTime()) ? null : parsed
  }

  function formatSessionAge(session) {
    const date = getSessionDate(session)
    if (!date) return ''
    const diffMs = Date.now() - date.getTime()
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))
    if (diffDays <= 0) return 'today'
    if (diffDays === 1) return 'yesterday'
    if (diffDays < 7) return `${diffDays}d ago`
    return date.toLocaleDateString()
  }

  function timeSince(timestamp) {
    return formatDuration(Date.now() - timestamp)
  }

  function buildMotionRow(session, extraCount) {
    if (!session) {
      return {
        tone: 'quiet',
        body: 'No live session',
        meta: 'Nothing running right now',
        icon: null,
      }
    }

    const badge = sessionBadge(session)
    const icon = toolIcon(session)
    const extraSuffix = extraCount > 0 ? ` +${extraCount} more` : ''
    const signal = activitySignal(session)

    if (isActiveLevel(signal.level)) {
      return {
        tone: LEVEL_TONE[signal.level],
        body: `${badge.toolLabel} is working now${extraSuffix}`,
        meta: session._duration != null ? `active ${formatDuration(session._duration)}` : 'Session is running',
        icon,
      }
    }

    if (signal.level === 'uncertain') {
      const retained = signal.source === 'stale' || signal.source === 'degraded'
      return {
        tone: LEVEL_TONE[signal.level],
        body: retained
          ? `${badge.toolLabel} was live — the last reading is stale${extraSuffix}`
          : `${badge.toolLabel} is active but we can't tell what it's doing${extraSuffix}`,
        meta: session._duration != null ? `active ${formatDuration(session._duration)}` : 'Something\'s happening',
        icon,
      }
    }

    const idleMeta = session._lastTransition
      ? `idle ${timeSince(session._lastTransition)}`
      : (session._duration != null ? `active ${formatDuration(session._duration)}` : 'Waiting for user input')

    return {
      tone: LEVEL_TONE[signal.level] ?? 'waiting',
      body: `${badge.toolLabel} is waiting on input${extraSuffix}`,
      meta: idleMeta,
      icon,
    }
  }

  function buildLatestChange(session, commit, isFresh) {
    if (session?.summary && isFresh) {
      return {
        body: `Session: ${session.summary}`,
        meta: formatSessionAge(session),
      }
    }

    if (commit?.message) {
      return {
        body: `Commit: ${commit.message}`,
        meta: commit.date ?? '',
      }
    }

    return {
      body: 'No recent session or commit yet',
      meta: '',
    }
  }

  function buildUnresolvedItem(session) {
    if (Array.isArray(session?.open_questions) && session.open_questions.length > 0) {
      return {
        label: 'Follow-up',
        body: `Open question: ${session.open_questions[0]}`,
      }
    }

    if (Array.isArray(session?.next_steps) && session.next_steps.length > 0) {
      return {
        label: 'Follow-up',
        body: `Next: ${session.next_steps[0]}`,
      }
    }

    return null
  }

  function buildVerdict({ project, primarySession, latestSession, freshLatestSession, unresolvedItem }) {
    const primaryLevel = primarySession ? activitySignal(primarySession).level : 'offline'

    if (isActiveLevel(primaryLevel)) {
      return {
        tone: 'active',
        label: 'Active work in progress',
        whyNow: project?.isDirty
          ? 'There\'s an active session and uncommitted changes.'
          : 'An agent is actively working here right now.',
      }
    }

    if (primaryLevel === 'uncertain') {
      return {
        tone: 'recent',
        label: 'Activity unconfirmed',
        whyNow: 'A session is here, but the latest activity could not be attributed to it.',
      }
    }

    if (primaryLevel === 'idle') {
      return {
        tone: 'waiting',
        label: 'Waiting on user input',
        whyNow: project?.isDirty
          ? 'The session is paused but there are uncommitted changes.'
          : 'A session is waiting — it probably needs your input.',
      }
    }

    if (freshLatestSession && unresolvedItem) {
      return {
        tone: 'recent',
        label: 'Recent handoff needs review',
        whyNow: 'The last session left open questions worth checking.',
      }
    }

    if (project?.isDirty) {
      return {
        tone: 'waiting',
        label: 'Uncommitted changes present',
        whyNow: 'There are uncommitted changes but no session running.',
      }
    }

    if (project?.activityState === 'recent') {
      return {
        tone: 'recent',
        label: 'Recent change, no live session',
        whyNow: 'Work happened recently but nothing\'s running now.',
      }
    }

    if (project?.activityState === 'stale') {
      return {
        tone: 'waiting',
        label: 'Project may need attention',
        whyNow: 'This project has been quiet for a while.',
      }
    }

    if (project?.activityState === 'dormant') {
      return {
        tone: 'quiet',
        label: 'Quiet project',
        whyNow: 'Nothing happening here recently.',
      }
    }

    return {
      tone: 'quiet',
      label: 'Project status unavailable',
      whyNow: 'Not enough data to show a preview.',
    }
  }

  function summarizeRelationship(relationshipsList, projectId) {
    if (!Array.isArray(relationshipsList) || relationshipsList.length === 0) return null

    const ordered = [...relationshipsList].sort((left, right) => relationshipRank(right) - relationshipRank(left))
    const relationship = ordered[0]
    if (!relationship) return null

    const outgoing = relationship.source_project_id === projectId
    const extraCount = Math.max(0, ordered.length - 1)
    const moreSuffix = extraCount > 0 ? ` +${extraCount} more` : ''
    const sourceLabel = detectionSourceLabel(relationship.detection_source)

    if (relationship.relationship_type === 'depends_on') {
      return {
        chip: 'Depends on',
        body: `${outgoing ? 'This project depends on another project' : 'Another project depends on this one'}${moreSuffix}${sourceLabel}`,
      }
    }

    if (relationship.relationship_type === 'references') {
      return {
        chip: 'Referenced',
        body: `${outgoing ? 'This project references another' : 'Another project references this one'}${moreSuffix}${sourceLabel}`,
      }
    }

    if (relationship.relationship_type === 'mentioned_in_session') {
      return {
        chip: 'Mentioned',
        body: `Mentioned in a recent session alongside another project${moreSuffix}${sourceLabel}`,
      }
    }

    if (relationship.relationship_type === 'workspace_sibling') {
      return {
        chip: 'Workspace',
        body: `Part of the same workspace${moreSuffix}${sourceLabel}`,
      }
    }

    if (relationship.relationship_type === 'includes') {
      return {
        chip: 'Includes',
        body: `Includes other linked projects${moreSuffix}${sourceLabel}`,
      }
    }

    return null
  }

  function relationshipRank(relationship) {
    switch (relationship?.relationship_type) {
      case 'depends_on':
        return 5
      case 'references':
        return 4
      case 'mentioned_in_session':
        return 3
      case 'workspace_sibling':
        return 2
      case 'includes':
        return 1
      default:
        return 0
    }
  }

  function detectionSourceLabel(source) {
    if (!source || source === 'manual') return ''
    if (source === 'session_mention') return ' via session context'
    if (source === 'cargo_toml') return ' via Cargo.toml'
    if (source === 'package_json') return ' via package.json'
    if (source === 'claude_md') return ' via CLAUDE.md'
    if (source === 'gitmodules') return ' via .gitmodules'
    return ''
  }
</script>

{#if project}
  <div
    bind:this={cardEl}
    class="fixed z-[90] max-w-[calc(100vw-24px)] rounded-xl border px-4 py-3.5 pointer-events-none backdrop-blur-[6px] transition-[opacity,transform] ease-out {cardWidthClass} {ui.card} {entered ? 'opacity-100 translate-y-0 scale-100 duration-120' : (visible ? 'opacity-0 translate-y-[2px] scale-[0.985] duration-120' : 'opacity-0 translate-y-0 scale-100 duration-70')}"
    style="left: {posX}px; top: {posY}px;"
    role="tooltip"
    data-testid="hovercard"
  >
    <div class="flex items-start gap-2" data-testid="hovercard-header">
      <div class="min-w-0 flex-1">
        <div class="text-[14px] font-semibold tracking-[-0.01em] leading-[1.15] font-sans truncate {ui.titleText}">{project.name}</div>
      </div>
      <div class="flex items-center gap-1.5 shrink-0 pt-0.5">
        {#if project.branch}
          <span class="max-w-[88px] truncate rounded-md px-1.5 py-1 text-[10px] font-mono leading-none {ui.branchChip}">
            {project.branch}
          </span>
        {/if}
        {#if project.isDirty}
          <span class="rounded-md px-1.5 py-1 text-[10px] font-medium leading-none {ui.dirtyChip}">Dirty</span>
        {/if}
      </div>
    </div>

    <div class="mt-2" data-testid="hovercard-verdict">
      <div class="text-[13px] font-medium leading-[1.25] {verdictToneClass}">{verdict.label}</div>
      <p class="mt-1 text-[11px] leading-[1.3] {ui.secondaryText} {ui.whyNowClamp}">
        {verdict.whyNow}
      </p>
    </div>

    <div class="mt-3 grid gap-1.5">
      <section class="rounded-lg px-2.5 py-2 border {ui.evidenceRow}" data-testid="hovercard-motion">
        <div class="text-[10px] uppercase tracking-[0.08em] font-medium {ui.mutedText}">Live session</div>
        <div class="mt-0.5 flex items-start gap-2">
          {#if motionRow.icon}
            <svg class="mt-[2px] h-[11px] w-[11px] shrink-0 {motionToneClass}" viewBox={motionRow.icon.viewBox} fill="currentColor" aria-hidden="true">
              <path d={motionRow.icon.path}/>
            </svg>
          {/if}
          <div class="min-w-0 flex-1">
            <div class="text-[12px] leading-[1.35] {ui.bodyText}">{motionRow.body}</div>
            <div class="mt-0.5 text-[11px] leading-[1.3] {ui.secondaryText}">{motionRow.meta}</div>
          </div>
        </div>
      </section>

      <section class="rounded-lg px-2.5 py-2 border {ui.evidenceRow}" data-testid="hovercard-latest-change">
        <div class="text-[10px] uppercase tracking-[0.08em] font-medium {ui.mutedText}">Last update</div>
        <div class="mt-0.5 text-[12px] leading-[1.35] line-clamp-2 {ui.bodyText}">{latestChange.body}</div>
        {#if latestChange.meta}
          <div class="mt-0.5 text-[11px] leading-[1.3] {ui.secondaryText}">{latestChange.meta}</div>
        {/if}
      </section>

      {#if unresolvedItem}
        <section class="rounded-lg px-2.5 py-2 border {ui.evidenceRow}" data-testid="hovercard-unresolved">
          <div class="text-[10px] uppercase tracking-[0.08em] font-medium {ui.mutedText}">{unresolvedItem.label}</div>
          <div class="mt-0.5 text-[12px] leading-[1.35] line-clamp-2 {ui.bodyText}">{unresolvedItem.body}</div>
        </section>
      {/if}
    </div>

    {#if groupedTeams.length > 0}
      <div class="mt-3 pt-2.5 border-t {ui.relationshipDivider}" data-testid="hovercard-team-roster">
        {#each groupedTeams as team}
          <section class="rounded-lg px-2.5 py-2 border {ui.evidenceRow}">
            <div class="flex items-center justify-between gap-2">
              <div class="text-[10px] uppercase tracking-[0.08em] font-medium {ui.mutedText}">Mesh team</div>
              <span
                class="inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-semibold {teamBadgeClasses[team.tone] ?? (team.isActive ? teamBadgeClasses.active : teamBadgeClasses.idle)}"
                data-testid="hovercard-team-badge"
              >
                T{team.count}
              </span>
            </div>
            <div class="mt-1 text-[12px] font-medium leading-[1.3] {ui.bodyText}">{team.groupLabel}</div>
            <ul class="mt-2 space-y-1.5">
              {#each team.members as member}
                {@const badge = sessionBadge(member)}
                {@const memberSignal = activitySignal(member)}
                <li class="flex items-center justify-between gap-3 text-[11px] leading-[1.3]">
                  <div class="min-w-0 flex items-center gap-2">
                    <span class="truncate {ui.bodyText}">{member.member_name || badge.toolLabel}</span>
                    <span class="{ui.secondaryText}">{badge.toolLabel}</span>
                  </div>
                  <span class="{toneClasses[LEVEL_TONE[memberSignal.level]] ?? toneClasses.waiting}">
                    {memberSignal.label}
                  </span>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    {/if}

    {#if relationshipCue}
      <div class="mt-3 pt-2.5 border-t {ui.relationshipDivider}" data-testid="hovercard-relationship">
        <span class="inline-flex items-center rounded-md px-1.5 py-1 text-[10px] font-medium {ui.relationshipChip}">
          {relationshipCue.chip}
        </span>
        <div class="mt-1 text-[11px] leading-[1.3] {ui.secondaryText} {ui.relationshipBodyClamp}">
          {relationshipCue.body}
        </div>
      </div>
    {/if}
  </div>
{/if}
