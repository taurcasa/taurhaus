import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

export const E2E_RUN_TOKEN_ENV = 'TAURHAUS_E2E_RUN_TOKEN'

/** Linux `/proc/<pid>/stat` field 22, which disambiguates reused PIDs. */
function readProcStartTime(pid, { procRoot = '/proc', readFile = readFileSync } = {}) {
  if (!Number.isInteger(pid) || pid <= 0) return null
  try {
    const stat = String(readFile(join(procRoot, String(pid), 'stat'), 'utf8'))
    const commandEnd = stat.lastIndexOf(')')
    if (commandEnd < 0) return null
    // After `(comm)`, index 0 is field 3 (`state`), so starttime (field 22)
    // is index 19. Using the final `)` handles process names containing spaces.
    const fields = stat.slice(commandEnd + 1).trim().split(/\s+/)
    return fields[19] || null
  } catch {
    return null
  }
}

function ownedProcessRecord(pid, { processGroup = false, readStartTime = readProcStartTime } = {}) {
  const startTime = readStartTime(pid)
  if (!startTime) return null
  return { pid, startTime: String(startTime), processGroup: Boolean(processGroup) }
}

function ownedProcessRecordMatches(record, { readStartTime = readProcStartTime } = {}) {
  if (!record || !Number.isInteger(record.pid) || record.pid <= 0 || !record.startTime) return false
  return String(readStartTime(record.pid) ?? '') === String(record.startTime)
}

/** Kill only while the process still has the identity the run recorded. */
export function killOwnedProcessRecord(
  record,
  { readStartTime = readProcStartTime, kill = process.kill, signal = 'SIGKILL' } = {}
) {
  if (!ownedProcessRecordMatches(record, { readStartTime })) return false
  const target = record.processGroup ? -record.pid : record.pid
  try {
    kill(target, signal)
    return true
  } catch {
    if (!record.processGroup || !ownedProcessRecordMatches(record, { readStartTime })) return false
    try {
      kill(record.pid, signal)
      return true
    } catch {
      return false
    }
  }
}

function checkoutLedgerDir(checkoutRoot, registryRoot = tmpdir()) {
  const checkoutId = createHash('sha256').update(resolve(checkoutRoot)).digest('hex').slice(0, 16)
  return join(registryRoot, `taurhaus-e2e-processes-${checkoutId}`)
}

function writeLedger(path, value) {
  const temporary = `${path}.${process.pid}.tmp`
  writeFileSync(temporary, `${JSON.stringify(value)}\n`, { mode: 0o600 })
  renameSync(temporary, path)
}

/** Live processes carrying this worker's inherited run token. */
export function findRunTokenProcessRecords(
  runToken,
  { procRoot = '/proc', readDir = readdirSync, readFile = readFileSync } = {}
) {
  if (!runToken) return []
  const marker = `${E2E_RUN_TOKEN_ENV}=${runToken}`
  const records = []
  let entries
  try {
    entries = readDir(procRoot)
  } catch {
    return records
  }

  for (const entry of entries) {
    if (!/^\d+$/.test(entry)) continue
    const pid = Number(entry)
    try {
      const environment = String(readFile(join(procRoot, entry, 'environ')))
      if (!environment.split('\0').includes(marker)) continue
    } catch {
      continue
    }
    const record = ownedProcessRecord(pid, {
      readStartTime: (candidate) => readProcStartTime(candidate, { procRoot, readFile }),
    })
    if (record) records.push(record)
  }
  return records
}

/** Persistent ownership for one WDIO worker. */
export function createOwnedProcessLedger({
  checkoutRoot,
  runToken,
  ownerPid = process.pid,
  registryRoot = tmpdir(),
  readStartTime = readProcStartTime,
  logger = console,
}) {
  const root = resolve(checkoutRoot)
  const directory = checkoutLedgerDir(root, registryRoot)
  mkdirSync(directory, { recursive: true, mode: 0o700 })
  const path = join(directory, `${runToken}.json`)
  const owner = ownedProcessRecord(ownerPid, { readStartTime })
  if (!owner) {
    throw new Error(
      `cannot record E2E owner process ${ownerPid}: the ownership ledger needs Linux /proc; E2E is Linux-only`
    )
  }
  const processes = new Map()

  function snapshot() {
    return {
      version: 1,
      checkoutRoot: root,
      runToken,
      owner,
      processes: [...processes.values()],
    }
  }

  function persist() {
    writeLedger(path, snapshot())
  }

  function record(record) {
    if (!record) return false
    const current = processes.get(record.pid)
    const next = {
      ...record,
      processGroup: Boolean(record.processGroup || current?.processGroup),
    }
    if (
      current?.startTime === next.startTime &&
      current?.processGroup === next.processGroup
    ) {
      return true
    }
    processes.set(record.pid, next)
    persist()
    return true
  }

  persist()
  return {
    path,
    record,
    recordPid(pid, options = {}) {
      return record(ownedProcessRecord(pid, { ...options, readStartTime }))
    },
    cleanup({ kill = process.kill } = {}) {
      const ordered = [...processes.values()].sort(
        (left, right) => Number(right.processGroup) - Number(left.processGroup)
      )
      for (const processRecord of ordered) {
        killOwnedProcessRecord(processRecord, { readStartTime, kill })
      }
    },
    remove() {
      try {
        rmSync(path, { force: true })
      } catch (error) {
        logger.warn(`[e2e] failed to remove process ledger ${path}: ${error?.message ?? error}`)
      }
    },
  }
}

/** Clean only abandoned ledgers produced by this checkout. */
export function cleanupStaleProcessLedgers(
  checkoutRoot,
  {
    registryRoot = tmpdir(),
    readStartTime = readProcStartTime,
    readFile = readFileSync,
    readDir = readdirSync,
    kill = process.kill,
    logger = console,
  } = {}
) {
  const root = resolve(checkoutRoot)
  const directory = checkoutLedgerDir(root, registryRoot)
  let files
  try {
    files = readDir(directory).filter((name) => name.endsWith('.json'))
  } catch {
    return
  }

  for (const file of files) {
    const path = join(directory, file)
    try {
      const ledger = JSON.parse(String(readFile(path, 'utf8')))
      if (ledger.checkoutRoot !== root || ownedProcessRecordMatches(ledger.owner, { readStartTime })) {
        continue
      }
      const records = Array.isArray(ledger.processes) ? ledger.processes : []
      records.sort((left, right) => Number(right.processGroup) - Number(left.processGroup))
      for (const record of records) {
        killOwnedProcessRecord(record, { readStartTime, kill })
      }
      rmSync(path, { force: true })
    } catch (error) {
      logger.warn(`[e2e] removed unreadable process ledger ${path}: ${error?.message ?? error}`)
      try {
        rmSync(path, { force: true })
      } catch {
        // Nothing recoverable: an unreadable ledger carries no identities.
      }
    }
  }
}

/**
 * Host state a lane changed and has to hand back, on every path out.
 *
 * A long, expensive spec is the one an operator interrupts, and an interrupted
 * WebdriverIO run never reaches Mocha's `after` hook: `wdio.conf.js` turns
 * SIGINT and SIGTERM into "clean up the session, then `process.exit`". Anything
 * a lane changed outside its own temp root is therefore left changed unless the
 * undo sits on the signal path.
 *
 * So a lane takes on each undo as it makes the change (`owe`), drops it once the
 * normal teardown has done it (`settled`), and `install` puts the whole set in
 * front of the handler that exits. Undos run synchronously — an `exit` handler
 * cannot await — and each one runs at most once.
 */
export function createLaneCleanup({ logger = console } = {}) {
  const owed = new Map()

  function run() {
    for (const [name, undo] of [...owed]) {
      owed.delete(name)
      try {
        undo()
      } catch (error) {
        logger.warn(`[e2e] lane cleanup step "${name}" failed: ${error?.message ?? error}`)
      }
    }
  }

  return {
    /** Take on an undo for a change just made. A repeated name replaces the old one. */
    owe(name, undo) {
      owed.set(name, undo)
    },
    /** Drop an undo the normal teardown has already carried out. */
    settled(name) {
      owed.delete(name)
    },
    /** Names still owed — for assertions and for reporting a partial teardown. */
    owed() {
      return [...owed.keys()]
    },
    /** Run every remaining undo now. Safe to call again; nothing runs twice. */
    run,
    /**
     * Put the undos in front of the handler that exits.
     *
     * `prependListener` is the point: `wdio.conf.js` registers its own
     * SIGINT/SIGTERM handler when the config module loads — before any spec —
     * and that handler deletes the session temp root and exits without
     * returning, so a listener added after it never runs.
     *
     * A crash leaves by a different door. `wdio.conf.js` also handles
     * `uncaughtException` and `unhandledRejection`, and having a listener at all
     * is what stops Node terminating on one: its handler deletes the session
     * temp root and returns, so the run carries on over roots that are gone and
     * nothing hands the host back what the lane changed. The undos go in front
     * of those two as well — but only where something already listens. A crash
     * nobody handles still terminates the process and still emits `exit` on the
     * way out, which the undos are already on; listening ourselves would
     * suppress that termination and turn a crash into a hang.
     */
    install(proc = process) {
      proc.prependListener('SIGINT', run)
      proc.prependListener('SIGTERM', run)
      proc.prependListener('exit', run)
      for (const crash of ['uncaughtException', 'unhandledRejection']) {
        if (proc.listenerCount(crash) > 0) proc.prependListener(crash, run)
      }
    },
  }
}
