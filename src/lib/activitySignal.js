/**
 * Presented activity signal — the single derivation every surface uses.
 *
 * The sidebar, the hover card and the mesh canvas each used to re-derive
 * "is this thing working?" from a different subset of the same record, so a
 * session could read green in one panel and grey in another. This module is
 * the one place that turns a session or team-member record into what we show.
 *
 * Levels, strongest evidence first:
 *
 * - `working`   reported active **and** attributed to this session by the
 *               scanner (`activity_attribution === 'attributed'`).
 * - `active`    reported active (or starting) with no per-session evidence —
 *               a team member row whose status came from the roster.
 * - `idle`      alive, waiting for input.
 * - `uncertain` something is moving but we cannot pin it on this record:
 *               project-scoped activity we could not attribute, a retained
 *               (stale) presence, or a degraded scan that observed nothing.
 * - `offline`   not running — including a tmux pane that a foreign process
 *               has taken over (pane ids are reused across tmux restarts).
 *
 * `confidence` ('high' | 'medium' | 'low') is the quality of the evidence
 * behind the level and `source` names that evidence, so a caller can explain
 * the level instead of re-deriving it.
 *
 * Only change-gated evidence is read. `recent_io` and `last_output_age_secs`
 * flip on every scan poll, which is why the daemon leaves them out of its
 * change signature (`daemon/session_activity.rs`) — the app therefore holds
 * whatever value rode the last real event, frozen. Reading `recent_io` would
 * make the indicator flicker at the scan cadence; reading the output age would
 * be worse than flicker, because a frozen "3 seconds ago" stays inside any
 * recency window forever. Confidence comes from `activity_confidence`, which
 * the daemon does version.
 */

/** Every level this module can return, strongest first. */
export const ACTIVITY_LEVELS = Object.freeze([
  'working',
  'active',
  'idle',
  'uncertain',
  'offline',
])

const LEVEL_LABELS = {
  working: 'Working',
  active: 'Active',
  idle: 'Idle',
  uncertain: 'Uncertain',
  offline: 'Offline',
}

const REPORTED_CONFIDENCE = new Set(['high', 'medium', 'low'])

function firstString(...values) {
  for (const value of values) {
    if (value === null || value === undefined) continue
    const text = String(value).trim().toLowerCase()
    if (text) return text
  }
  return ''
}

/**
 * Reported status, normalized to a base token.
 * `status` is read ahead of `sessionStatus` so a node that already carries a
 * derived level keeps it (the derivation is idempotent).
 */
function baseStatus(record) {
  const raw = firstString(
    record?.state,
    record?.status,
    record?.sessionStatus,
    record?.session_status
  )
  if (raw === 'active' || raw === 'working' || raw === 'idle' || raw === 'uncertain') return raw
  if (raw === 'starting') return 'starting'
  return 'offline'
}

function attribution(record) {
  return firstString(record?.activity_attribution, record?.activityAttribution)
}

function reportedConfidence(record) {
  const value = firstString(record?.activity_confidence, record?.activityConfidence)
  return REPORTED_CONFIDENCE.has(value) ? value : ''
}

function isUnattributed(record) {
  return (
    attribution(record) === 'unattributed' ||
    record?.project_unattributed_active === true ||
    record?.projectUnattributedActive === true
  )
}

function isStalePresence(record) {
  return record?._presenceStale === true || record?._presenceStatus === 'stale'
}

function signal(level, source, confidence, label = LEVEL_LABELS[level]) {
  return { level, label, confidence, source }
}

/**
 * Derive the presented activity signal for a session or team-member record.
 *
 * @param {object|null|undefined} record session (`state`, `activity_*`) or
 *   team member (`sessionStatus`, `paneAlive`) shaped record.
 * @returns {{level: string, label: string, confidence: string, source: string}}
 */
export function activitySignal(record) {
  if (record?.pane_foreign === true || record?.paneForeign === true) {
    return signal('offline', 'pane_foreign', 'high')
  }
  if (record?.pane_alive === false || record?.paneAlive === false) {
    return signal('offline', 'pane_dead', 'high')
  }

  const base = baseStatus(record)
  if (base === 'offline') return signal('offline', 'none', 'medium')

  if (record?.degraded === true) return signal('uncertain', 'degraded', 'low')
  if (isStalePresence(record)) return signal('uncertain', 'stale', 'low')
  if (base === 'uncertain') return signal('uncertain', 'status', 'low')
  if (isUnattributed(record)) return signal('uncertain', 'project', 'low')

  const attributed = attribution(record) === 'attributed'
  const source = attributed ? 'session' : 'status'
  // `activity_confidence` grades the *activity* evidence. The scanner
  // hard-codes it to `low` for a session it saw no activity from
  // (`classification.rs`), where it means "nothing to attribute", not "we are
  // unsure this is idle" — so an idle reading keeps its own default.
  const confidence = base === 'idle' ? 'medium' : reportedConfidence(record) || 'medium'

  if (base === 'idle') return signal('idle', source, confidence)
  if (base === 'starting') return signal('active', source, confidence, 'Starting')
  if (base === 'working' || attributed) return signal('working', source, confidence)
  return signal('active', source, confidence)
}

/** Convenience for call sites that only need the level. */
export function activityLevel(record) {
  return activitySignal(record).level
}

/** True while the record still exists — anything but `offline`. */
export function isLiveLevel(level) {
  return level !== 'offline' && ACTIVITY_LEVELS.includes(level)
}

/** True while the record is doing work — `working` or `active`. */
export function isActiveLevel(level) {
  return level === 'working' || level === 'active'
}

/**
 * True when the signal describes a *retained* reading rather than an observed
 * one: the daemon bridge went down (`stale`) or its scanner went blind
 * (`degraded`). Both mean the record is the last thing we saw, not the current
 * truth, so both wear the same tone and the same wording.
 */
export function isRetainedSignal(signal) {
  return signal?.source === 'stale' || signal?.source === 'degraded'
}
