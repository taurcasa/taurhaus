import { execFileSync, spawn } from 'node:child_process'

/** tmux session this helper creates when the runner started without a client. */
const OWNED_SESSION = 'taurhaus-e2e-focus'

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

/**
 * The tmux session of the most recently active attached client, or null.
 *
 * This is the client the daemon hub reads focus from: `focus_from_clients`
 * prefers a client whose terminal reports `focused` and otherwise falls back to
 * the most recently active attached client, so a test only needs *an attached*
 * client — not the OS focus, which the app window holds while E2E runs.
 */
function attachedTmuxSession() {
  const attached = listAttachedClients()
  if (!attached) return null
  return attached.sort((a, b) => b.activity - a.activity)[0]?.session ?? null
}

function listAttachedClients() {
  const clients = runTmux(['list-clients', '-F', '#{client_flags}\t#{session_name}\t#{client_activity}'])
  if (!clients.ok) return null

  return String(clients.output ?? '')
    .split('\n')
    .map((line) => line.split('\t'))
    .filter(([flags, session]) => Boolean(session?.trim()) && String(flags).split(',').includes('attached'))
    .map(([, session, activity]) => ({ session: session.trim(), activity: Number(activity) || 0 }))
}

/** Block the calling thread — these helpers are synchronous by design. */
function waitSync(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}

/**
 * An attached tmux session the focus hub can read, creating one when needed.
 *
 * The hub resolves focus from `tmux list-clients`, so a test that asserts on
 * the foreground indicator needs a client to exist. A developer machine already
 * has one; a clean E2E runner has none, and skipping there would quietly retire
 * the regression. Returns `{ session, created, cleanup }`, or `null` only when
 * tmux (or a pty for it) genuinely is not available.
 */
export function ensureAttachedTmuxSession({ cwd = process.cwd(), name = OWNED_SESSION } = {}) {
  const existing = attachedTmuxSession()
  if (existing) return { session: existing, created: false, cleanup() {} }
  return createAttachedTmuxSession({ cwd, name })
}

/**
 * Create a tmux session and attach a client to it inside a pseudo-terminal.
 *
 * `tmux attach` needs a terminal, which a WebdriverIO runner does not have;
 * `script` (util-linux) allocates one and runs the attach inside it. The client
 * only counts once `list-clients` reports it, so this waits for that.
 */
function createAttachedTmuxSession({ cwd = process.cwd(), name = OWNED_SESSION } = {}) {
  runTmux(['kill-session', '-t', name])
  const created = runTmux(['new-session', '-d', '-s', name, '-c', cwd, 'sh -c "while :; do sleep 3600; done"'])
  if (!created.ok) return null

  let client = null
  try {
    client = spawn('script', ['-qfc', `tmux attach-session -t ${name}`, '/dev/null'], {
      cwd,
      detached: true,
      stdio: 'ignore',
    })
    client.unref()
  } catch {
    runTmux(['kill-session', '-t', name])
    return null
  }

  const cleanup = () => {
    runTmux(['kill-session', '-t', name])
    try {
      if (client?.pid) process.kill(-client.pid, 'SIGTERM')
    } catch {
      // The client exits on its own when the session dies.
    }
  }

  const deadline = Date.now() + 10_000
  while (Date.now() < deadline) {
    if ((listAttachedClients() ?? []).some((entry) => entry.session === name)) {
      return { session: name, created: true, cleanup }
    }
    waitSync(150)
  }

  cleanup()
  return null
}

/**
 * Create a window in `session` and select it, returning its pane/window ids.
 *
 * `new-window` without `-d` selects the window it creates, so the attached
 * client's current pane becomes this one — which is what the hub reports.
 */
export function openTmuxWindow({ session, cwd, command, name = 'e2e' }) {
  const created = runTmux([
    'new-window',
    '-t',
    session,
    '-n',
    name,
    '-c',
    cwd,
    '-P',
    '-F',
    '#{pane_id}\t#{window_index}',
    command,
  ])
  if (!created.ok) return null

  const [paneId, windowIndex] = String(created.output ?? '').trim().split('\t')
  if (!/^%\d+$/.test(String(paneId).trim())) return null

  return { session, paneId: paneId.trim(), windowIndex: String(windowIndex ?? '').trim() }
}

export function killTmuxPane(paneId) {
  if (!paneId) return
  runTmux(['kill-pane', '-t', paneId])
}
