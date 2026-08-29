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
export function createTask({ claudeDir, team, actor, subject, description, effort, why, firstStep, deliverable }) {
  const raw = runMesh(claudeDir, [
    '--team', team,
    '--name', actor,
    'task', 'create',
    '--subject', subject,
    '--description', description,
    '--effort', effort,
    '--why', why,
    '--first-step', firstStep,
    '--deliverable', deliverable,
    '--json',
  ])
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
  claudeDir, team, actor, taskId, owner, effort, why, firstStep, deliverable, completionSignal,
}) {
  return runMesh(claudeDir, [
    '--team', team,
    '--name', actor,
    'task', 'assign', String(taskId),
    '--owner', owner,
    '--effort', effort,
    '--why', why,
    '--first-step', firstStep,
    '--deliverable', deliverable,
    '--completion-signal', completionSignal,
  ])
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
