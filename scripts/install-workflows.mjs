// Installs the versioned procedures in .claude/workflows to user scope, so a lead can run them from
// any project on this account: `<CLAUDE_CONFIG_DIR>/workflows/<name>.js`.
//
// It follows the hook-installer discipline for anything under a user's config directory: resolve the
// account directory, prove ownership before writing, write through a temporary file plus an atomic
// rename, preserve the permissions a file already has, write through a symlink instead of replacing
// it, and change nothing it does not manage.
//
//   bun scripts/install-workflows.mjs [--account-dir <dir>] [--dry-run] [--uninstall]
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const WORKFLOWS_SUBDIR = 'workflows'
export const DEFAULT_SOURCE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '.claude/workflows')

const DIR_MODE = 0o700
const NEW_FILE_MODE = 0o600

export function resolveAccountDir({ argv = [], env = {}, home = os.homedir() } = {}) {
  const flag = argv.indexOf('--account-dir')
  if (flag !== -1) {
    const value = argv[flag + 1]
    if (!value) throw new Error('--account-dir needs a directory')
    return path.resolve(value)
  }
  if (env.CLAUDE_CONFIG_DIR) return path.resolve(env.CLAUDE_CONFIG_DIR)
  return path.join(home, '.claude')
}

// A config directory or file owned by someone else is never ours to rewrite.
export function ownershipProblem(stats, uid) {
  if (uid === null || uid === undefined) return ''
  if (!stats || stats.uid === undefined) return ''
  if (stats.uid === uid) return ''
  return `owned by uid ${stats.uid}, this process runs as uid ${uid}`
}

function currentUid() {
  return typeof process.getuid === 'function' ? process.getuid() : null
}

function assertOwned(target, uid) {
  let stats
  try {
    stats = fs.statSync(target)
  } catch {
    return
  }
  const problem = ownershipProblem(stats, uid)
  if (problem) throw new Error(`refusing to write ${target}: ${problem}`)
}

function readIfPresent(file) {
  try {
    return fs.readFileSync(file, 'utf8')
  } catch {
    return null
  }
}

// tempfile + rename, so a killed run never leaves a half-written procedure behind. The destination is
// resolved through a symlink first: the link is the user's choice and stays.
let tempCounter = 0
function writeAtomic(destination, contents, uid) {
  let real = destination
  try {
    if (fs.lstatSync(destination).isSymbolicLink()) real = fs.realpathSync(destination)
  } catch {
    // no destination yet — the plain path is the real one
  }
  assertOwned(real, uid)
  const existingMode = (() => {
    try {
      return fs.statSync(real).mode & 0o777
    } catch {
      return NEW_FILE_MODE
    }
  })()
  tempCounter += 1
  const temp = path.join(path.dirname(real), `${path.basename(real)}.tmp.${process.pid}.${tempCounter}`)
  let handle
  try {
    handle = fs.openSync(temp, 'wx', existingMode)
    fs.writeFileSync(handle, contents)
    fs.fsyncSync(handle)
    fs.closeSync(handle)
    handle = undefined
    fs.chmodSync(temp, existingMode)
    fs.renameSync(temp, real)
  } catch (error) {
    if (handle !== undefined) {
      try {
        fs.closeSync(handle)
      } catch {
        // already closed
      }
    }
    fs.rmSync(temp, { force: true })
    throw error
  }
}

function sourceFiles(source) {
  return fs
    .readdirSync(source)
    .filter((file) => file.endsWith('.js') || file === 'README.md')
    .sort()
}

export function installWorkflows({ source = DEFAULT_SOURCE, accountDir, dryRun = false, uninstall = false, uid = currentUid() } = {}) {
  if (!accountDir) throw new Error('installWorkflows needs an accountDir')
  if (!fs.existsSync(source)) throw new Error(`workflow source not found: ${source}`)
  const target = path.join(accountDir, WORKFLOWS_SUBDIR)
  assertOwned(accountDir, uid)
  assertOwned(target, uid)

  const files = sourceFiles(source)
  const actions = []

  if (uninstall) {
    for (const file of files) {
      const destination = path.join(target, file)
      const installed = readIfPresent(destination)
      if (installed === null) {
        actions.push({ file, action: 'absent' })
        continue
      }
      // Only a copy that still matches ours is ours to remove; an edited one is the user's.
      if (installed !== fs.readFileSync(path.join(source, file), 'utf8')) {
        actions.push({ file, action: 'kept-modified' })
        continue
      }
      if (!dryRun) {
        assertOwned(destination, uid)
        fs.rmSync(destination, { force: true })
      }
      actions.push({ file, action: 'removed' })
    }
    return { target, actions }
  }

  if (!dryRun && !fs.existsSync(target)) fs.mkdirSync(target, { recursive: true, mode: DIR_MODE })
  for (const file of files) {
    const contents = fs.readFileSync(path.join(source, file), 'utf8')
    const destination = path.join(target, file)
    const installed = readIfPresent(destination)
    if (installed === contents) {
      actions.push({ file, action: 'unchanged' })
      continue
    }
    if (!dryRun) writeAtomic(destination, contents, uid)
    actions.push({ file, action: installed === null ? 'installed' : 'updated' })
  }
  return { target, actions }
}

function main() {
  const argv = process.argv.slice(2)
  const dryRun = argv.includes('--dry-run')
  const uninstall = argv.includes('--uninstall')
  const accountDir = resolveAccountDir({ argv, env: process.env })
  const { target, actions } = installWorkflows({ accountDir, dryRun, uninstall })
  const counted = actions.reduce((tally, a) => ({ ...tally, [a.action]: (tally[a.action] || 0) + 1 }), {})
  for (const action of actions) console.log(`${action.action.padEnd(13)} ${action.file}`)
  const summary = Object.entries(counted)
    .map(([action, count]) => `${count} ${action}`)
    .join(', ')
  console.log(`${dryRun ? 'would ' : ''}${uninstall ? 'uninstall' : 'install'} in ${target}: ${summary || 'nothing to do'}`)
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (import.meta.main === true || invokedPath === fileURLToPath(import.meta.url)) {
  main()
}
