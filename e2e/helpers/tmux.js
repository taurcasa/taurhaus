import { execFileSync } from 'node:child_process'

function runTmux(args) {
  try {
    const output = execFileSync('tmux', args, {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 3_000,
      maxBuffer: 1024 * 1024,
    })
    return { ok: true, output: output ?? '' }
  } catch (error) {
    return {
      ok: false,
      message: error?.message ?? String(error),
      status: typeof error?.status === 'number' ? error.status : null,
    }
  }
}

function parsePaneIds(raw) {
  return Array.from(
    new Set(
      String(raw ?? '')
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => /^%\d+$/.test(line))
    )
  )
}

export function snapshotTmuxPanes() {
  const list = runTmux(['list-panes', '-a', '-F', '#{pane_id}'])
  if (!list.ok) {
    return {
      available: false,
      paneIds: [],
      reason: list.message,
    }
  }

  return {
    available: true,
    paneIds: parsePaneIds(list.output),
    reason: '',
  }
}

export function cleanupNewTmuxPanes(snapshot) {
  if (!snapshot?.available) {
    return {
      attempted: false,
      cleaned: [],
      failed: [],
      skippedReason: snapshot?.reason || 'tmux snapshot unavailable',
    }
  }

  const current = runTmux(['list-panes', '-a', '-F', '#{pane_id}'])
  if (!current.ok) {
    return {
      attempted: false,
      cleaned: [],
      failed: [],
      skippedReason: current.message,
    }
  }

  const before = new Set(Array.isArray(snapshot.paneIds) ? snapshot.paneIds : [])
  const after = parsePaneIds(current.output)
  const created = after.filter((paneId) => !before.has(paneId))
  const cleaned = []
  const failed = []

  for (const paneId of created) {
    const kill = runTmux(['kill-pane', '-t', paneId])
    if (kill.ok) {
      cleaned.push(paneId)
    } else {
      failed.push({ paneId, reason: kill.message })
    }
  }

  return {
    attempted: true,
    cleaned,
    failed,
    skippedReason: '',
  }
}
