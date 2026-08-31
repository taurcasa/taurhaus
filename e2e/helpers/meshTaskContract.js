/**
 * The mesh side of the assignment contract, as records rather than as prose.
 *
 * A managed stage is `mesh task create` + `mesh task assign` with an effort, a
 * first step, a deliverable and a completion signal; the stage is finished when
 * the member sends the lead one message that starts with `RESULT <task-id>` and
 * carries a JSON block. Everything a lane needs to assert about that lives in
 * files mesh owns:
 *
 *   - `mesh task get <id> --json` carries `pendingEffort`, mesh's own answer to
 *     "is this notice still held because the member has not reported the level
 *     the assignment asks for".
 *   - `teams/<team>/state/projections/attention/<id>.json` carries
 *     `deliveryState` / `deliveredAt`, which is when the member daemon actually
 *     put the notice in the pane. The daemon's stdout is discarded by taurhaus
 *     (`spawn_system_command` gives it `Stdio::null`), so this projection — not
 *     a log line — is the delivery record a lane can read.
 *   - `teams/<team>/inboxes/<member>.json` is the inbox the result arrives in.
 *
 * Every call is explicit about the Claude root it works in: the lane runs
 * against an isolated `TAURHAUS_CLAUDE_DIR`, and a mesh command that fell back
 * to `~/.claude` would bootstrap a team in the operator's real home.
 */

import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

/** Run one mesh command against an explicit Claude root; throws on failure. */
function runMesh(claudeDir, args, { timeout = 60_000 } = {}) {
  return execFileSync('mesh', ['--claude-dir', claudeDir, ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout,
  }).trim()
}

/** `mesh task create --json`, returning the created record. */
export function createTask({
  claudeDir, team, actor, subject, description, effort, why, deadline, firstStep, deliverable,
}) {
  const args = [
    '--team', team,
    '--name', actor,
    'task', 'create',
    '--subject', subject,
    '--description', description,
    '--effort', effort,
    '--why', why,
    '--first-step', firstStep,
    '--deliverable', deliverable,
  ]
  if (deadline != null) args.push('--deadline', String(deadline))
  args.push('--json')
  const raw = runMesh(claudeDir, args)
  return JSON.parse(raw)
}

/**
 * `mesh task assign`, with the effort and the completion signal on the
 * assignment itself.
 *
 * The completion signal names the task id, which only exists once the task has
 * been created — so it is passed here and not at creation.
 */
export function assignTask({
  claudeDir, team, actor, taskId, owner, status, effort, why, deadline, firstStep, deliverable,
  completionSignal,
}) {
  const args = [
    '--team', team,
    '--name', actor,
    'task', 'assign', String(taskId),
    '--owner', owner,
    '--effort', effort,
    '--why', why,
    '--first-step', firstStep,
    '--deliverable', deliverable,
    '--completion-signal', completionSignal,
  ]
  if (status) args.push('--status', status)
  if (deadline != null) args.push('--deadline', String(deadline))
  return runMesh(claudeDir, args)
}

/**
 * `mesh task get <id> --json`.
 *
 * Also the cheapest way to make mesh rebuild its projections, which is why the
 * attention record below is read after a call to this.
 */
export function taskRecord({ claudeDir, team, actor, taskId }) {
  return JSON.parse(runMesh(claudeDir, [
    '--team', team, '--name', actor, 'task', 'get', String(taskId), '--json',
  ]))
}

/**
 * The timeout branch used by the managed `stage()` courier.
 *
 * The task record is the canonical authority: inbox state and a local elapsed
 * timer cannot turn a still-open task into a stage timeout. Returning null for
 * every non-stale record lets callers keep polling without inventing another
 * terminal state.
 */
export function stagePollVerdict(record) {
  return record?.status === 'stale' ? { status: 'timeout' } : null
}

/**
 * Classify the operational record available after a mesh task becomes stale.
 *
 * The deadline pass first writes a stale marker, but the task importer removes
 * non-resumable tasks from the member snapshot. Either record can therefore be
 * the first one a polling lane observes after the durable stale event.
 */
export function operationalStaleEvidence(snapshot, taskId) {
  const task = snapshot?.task
  if (!task) return null

  const expectedTaskId = String(taskId)
  const observedTaskId = String(task.id ?? '').trim()
  const status = String(task.status ?? '').trim()
  if (observedTaskId !== expectedTaskId) {
    return {
      state: 'task-cleared',
      observedTaskId: observedTaskId || null,
      status: status || null,
      staleAt: null,
    }
  }

  if (status !== 'stale' || !Number.isFinite(Date.parse(task.stale_at))) return null
  return {
    state: 'marked',
    observedTaskId,
    status,
    staleAt: task.stale_at,
  }
}

/**
 * Build the active negative path around the production self-heal cadence.
 *
 * The heartbeat must span half the deadline plus one complete pass cadence,
 * and its output must be dense enough for Codex's `/proc` read-rate signal.
 * What remains before the full deadline is the two-turn completion allowance.
 */
export function activeDeadlineHeartbeatPlan({
  deadlineMinutes,
  passCadenceMs,
  intervalMs,
  payloadBytes,
}) {
  const deadlineMs = Number(deadlineMinutes) * 60_000
  const numericInputs = [deadlineMs, passCadenceMs, intervalMs, payloadBytes]
  if (numericInputs.some((value) => !Number.isFinite(value) || value <= 0)) {
    throw new Error('active deadline heartbeat inputs must be positive finite numbers')
  }

  const neededActiveMs = deadlineMs / 2 + passCadenceMs
  const iterations = Math.ceil(neededActiveMs / intervalMs)
  const durationMs = iterations * intervalMs
  const completionSlackMs = deadlineMs - durationMs
  if (completionSlackMs <= 0) {
    throw new Error('active deadline heartbeat must leave time to complete before stale')
  }

  return {
    command:
      `bun -e 'const payload = "x".repeat(${payloadBytes}); ` +
      `for (let i = 0; i < ${iterations}; i += 1) { console.log(payload); await Bun.sleep(${intervalMs}) }'`,
    deadlineMs,
    neededActiveMs,
    iterations,
    durationMs,
    completionSlackMs,
    outputBytesPerSecond: payloadBytes * 1_000 / intervalMs,
  }
}

/**
 * Join a half-deadline self-heal pass to the fresh active snapshot it read.
 *
 * The production deadline pass accepts `active` and `likely_working` snapshots
 * no more than 120 seconds old. A later activity record cannot explain an
 * earlier pass, so this chooses the newest snapshot at or before each eligible
 * pass and only then applies the production freshness and confidence checks.
 * Any committed deadline action makes the negative path false rather than
 * letting a later active sample cover it up.
 */
export function activeDeadlinePassEvidence({
  assignedAt,
  deadlineMinutes,
  activitySnapshots = [],
  passEvents = [],
  deadlineEvents = [],
}) {
  if (deadlineEvents.length > 0) return null
  const assignedAtMs = Date.parse(assignedAt)
  const deadlineMs = Number(deadlineMinutes) * 60_000
  if (!Number.isFinite(assignedAtMs) || !Number.isFinite(deadlineMs) || deadlineMs <= 0) return null
  const halfDueMs = assignedAtMs + deadlineMs / 2

  const observed = activitySnapshots
    .map((snapshot) => ({ snapshot, observedAtMs: Date.parse(snapshot?.observed_at) }))
    .filter(({ observedAtMs }) =>
      Number.isFinite(observedAtMs) &&
      observedAtMs >= assignedAtMs
    )

  for (const pass of passEvents) {
    const passAtMs = Date.parse(pass?.ts)
    if (!Number.isFinite(passAtMs) || passAtMs < halfDueMs) continue
    const snapshot = observed
      .filter(({ observedAtMs }) => observedAtMs <= passAtMs)
      .sort((left, right) => right.observedAtMs - left.observedAtMs)[0]
    if (
      !snapshot ||
      passAtMs - snapshot.observedAtMs > 120_000 ||
      !['active', 'likely_working'].includes(snapshot.snapshot?.activity_confidence)
    ) continue
    return {
      halfDueAt: new Date(halfDueMs).toISOString(),
      passAt: pass.ts,
      activityObservedAt: snapshot.snapshot.observed_at,
      activityConfidence: snapshot.snapshot.activity_confidence,
    }
  }
  return null
}

/** The attention projection for one task, or null before mesh has written it. */
export function attentionRecord({ claudeDir, team, taskId }) {
  try {
    return JSON.parse(readFileSync(
      join(claudeDir, 'teams', team, 'state', 'projections', 'attention', `${taskId}.json`),
      'utf8'
    ))
  } catch {
    return null
  }
}

/** A member's mesh inbox, as the array mesh stores it. */
export function readInbox({ claudeDir, team, member }) {
  try {
    const parsed = JSON.parse(readFileSync(join(claudeDir, 'teams', team, 'inboxes', `${member}.json`), 'utf8'))
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

/**
 * The first complete JSON object in `text`.
 *
 * A fenced block wins where the member wrote one; otherwise the first `{` is
 * matched to its own closing brace, so a nested object or a brace inside a
 * string does not end the scan early. Returns null rather than throwing: "the
 * member did not send parseable JSON" is a result the lane reports, not an
 * exception it dies on.
 */
export function extractJsonBlock(text) {
  const body = String(text ?? '')
  const fenced = body.match(/```(?:json)?\s*\n([\s\S]*?)```/i)
  const candidates = []
  if (fenced) candidates.push(fenced[1])
  const bare = balancedObject(body)
  if (bare) candidates.push(bare)

  for (const candidate of candidates) {
    try {
      const parsed = JSON.parse(candidate)
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) return parsed
    } catch {
      // Try the next candidate; a fenced block with prose in it is not fatal.
    }
  }
  return null
}

/** The substring from the first `{` to the brace that closes it, or null. */
function balancedObject(body) {
  const start = body.indexOf('{')
  if (start < 0) return null

  let depth = 0
  let inString = false
  let escaped = false
  for (let index = start; index < body.length; index += 1) {
    const character = body[index]
    if (inString) {
      if (escaped) escaped = false
      else if (character === '\\') escaped = true
      else if (character === '"') inString = false
      continue
    }
    if (character === '"') inString = true
    else if (character === '{') depth += 1
    else if (character === '}') {
      depth -= 1
      if (depth === 0) return body.slice(start, index + 1)
    }
  }
  return null
}

/**
 * Whether one message is the completion signal for `taskId`.
 *
 * The contract is deliberately strict about the opening token: "starts with
 * `RESULT <task-id>`" is what the lead was promised, and a message that merely
 * mentions the word later is a progress report, not a result.
 */
export function parseResultMessage(text, taskId) {
  return parseSignal(text, taskId, 'RESULT', { requireJson: true })
}

/** The `BLOCKED <task-id> <reason>` counterpart, so a lane can fail fast. */
export function findBlockedMessage(messages, taskId) {
  for (const message of messages ?? []) {
    const parsed = parseSignal(message?.text, taskId, 'BLOCKED', { requireJson: false })
    if (parsed.ok) return { message, reason: parsed.rest }
  }
  return null
}

function parseSignal(text, taskId, keyword, { requireJson }) {
  const body = String(text ?? '').trim()
  const match = body.match(new RegExp(`^${keyword}\\s+#?(\\S+)`))
  if (!match) return { ok: false, reason: `message does not open with ${keyword} <task-id>` }
  if (match[1].replace(/[^0-9A-Za-z_-]+$/, '') !== String(taskId)) {
    return { ok: false, reason: `${keyword} names task ${match[1]}, not ${taskId}` }
  }

  const rest = body.slice(match[0].length).trim()
  if (!requireJson) return { ok: true, rest }

  const payload = extractJsonBlock(rest)
  if (!payload) return { ok: false, reason: `${keyword} #${taskId} carried no parseable JSON block` }
  return { ok: true, rest, payload }
}

/** The first inbox message that is a complete result for `taskId`, or null. */
export function findResultMessage(messages, taskId) {
  for (const message of messages ?? []) {
    const parsed = parseResultMessage(message?.text, taskId)
    if (parsed.ok) return { message, payload: parsed.payload }
  }
  return null
}

/**
 * What the completion signal asked for and the payload did not carry.
 *
 * `RESULT <task-id>` plus *any* JSON object is not the contract: the assignment
 * names `{commit, files, validation}`, and a payload that answers `{"noop":
 * true}` has not reported a deliverable however well-formed it is. A symbolic
 * `commit` is rejected here for the same reason `commitExists` rejects one —
 * `HEAD` names whatever the repo happens to point at, not what the stage wrote.
 */
export function resultContractViolations(payload) {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return ['the result carried no JSON object']
  }

  const violations = []
  const commit = String(payload.commit ?? '').trim()
  if (!/^[0-9a-f]{7,40}$/i.test(commit)) {
    violations.push(`"commit" is ${commit ? `"${commit}"` : 'missing'}, not a commit sha`)
  }
  const files = payload.files
  if (!Array.isArray(files) || files.length === 0 || files.some((file) => !String(file ?? '').trim())) {
    violations.push('"files" is not a non-empty list of paths')
  }
  if (!String(payload.validation ?? '').trim()) {
    violations.push('"validation" is missing or blank')
  }
  return violations
}

/**
 * Where an assignment stands between mesh's effort gate and mesh's delivery.
 *
 * The gate's promise is an ordering: the member reaches the level the
 * assignment asks for, *then* the notice is delivered. Its failure mode is not
 * "no delivery" but a delivery that came too early — mesh's effort wait
 * expiring mid-relaunch and handing the notice to a member still running at the
 * old level. Both readings are cheap and neither is conclusive alone, so they
 * are judged together: a `deliveredAt` seen while the runtime record still
 * reports the old level is that expiry, and nothing else looks like it.
 */
export function effortDeliveryVerdict({ appliedEffort, requiredEffort, deliveredAt = null }) {
  const applied = String(appliedEffort ?? '').trim().toLowerCase()
  const required = String(requiredEffort ?? '').trim().toLowerCase()
  if (applied && applied === required) return 'in-force'
  return deliveredAt ? 'delivered-early' : 'holding'
}

/** mesh's own default effort-wait bound, in seconds (`daemon.rs`). */
const DEFAULT_EFFORT_WAIT_SECS = 180

/**
 * How long mesh holds a notice before giving up on the effort, in ms.
 *
 * `MESH_EFFORT_WAIT_SECS` as mesh parses it: a `u64` count of seconds, and
 * anything else — blank, negative, fractional, not a number — is the default.
 * Read from an environment rather than from `process.env` so the value can be
 * asserted, and so a lane can report the bound it judged against.
 */
export function effortWaitBoundMs(environment = {}) {
  const raw = String(environment?.MESH_EFFORT_WAIT_SECS ?? '').trim()
  if (!/^\d+$/.test(raw)) return DEFAULT_EFFORT_WAIT_SECS * 1_000
  return Number(raw) * 1_000
}

/**
 * Why a delivered notice cannot be read as "the effort gate opened", or `''`.
 *
 * mesh delivers a held assignment notice for exactly two reasons: the member
 * reported the level the assignment asks for, or the wait ran out. The second
 * one is a pure function of the clock — `decide_notice_effort_gate` expires the
 * wait when `now - assigned_at >= bound` and nothing else re-arms it — so
 * mesh's own attention record settles which happened. A delivery strictly
 * inside the bound cannot be an expiry, and that is the ordering the gate
 * promises.
 *
 * It is worth checking separately from the runtime record because an expiry is
 * invisible a moment later: mesh hands the notice to a member still at the old
 * level, the relaunch lands seconds afterwards, and every reading taken after
 * that — `appliedEffort`, `pendingEffort`, a `deliveredAt` after the resume
 * began — is exactly what a gate that closed properly leaves behind. mesh says
 * so in its own log, but taurhaus spawns the member daemon with `Stdio::null`
 * (`coordination/runtime/process.rs`), so that line reaches nobody.
 */
export function expiredEffortWaitProblem({ assignedAtMs, deliveredAtMs, boundMs }) {
  if (!Number.isFinite(assignedAtMs) || !Number.isFinite(deliveredAtMs)) {
    return `mesh's delivery record carries no readable timestamp pair (assigned ${assignedAtMs}, delivered ${deliveredAtMs})`
  }

  const heldMs = deliveredAtMs - assignedAtMs
  if (heldMs >= boundMs) {
    return (
      `mesh held the notice ${heldMs}ms, at or past its ${boundMs}ms effort wait: the wait expired and the ` +
      'notice was released rather than the gate opening on the level'
    )
  }
  return ''
}
