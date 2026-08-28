/**
 * A scratch `CODEX_HOME` for the live Codex compaction lane.
 *
 * The lane drives a real Codex subscription, so it needs the real credentials —
 * and nothing else. Only `auth.json` and `config.toml` are copied: sessions,
 * history and the local databases stay in the operator's own home, and nothing
 * is ever written back to it. The managed `hooks.json`, the rollout transcripts
 * and every mutation the run causes land in the scratch copy.
 */

import { appendFileSync, copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

/** The only files copied out of the operator's real Codex home. */
export const CODEX_SCRATCH_FILES = ['auth.json', 'config.toml']

/**
 * Create `targetHome` and copy the credential files into it.
 * Returns which files were copied and which were absent — a missing
 * `auth.json` is the signal that the lane cannot run, not an error to throw.
 */
export function createCodexScratchHome(sourceHome, targetHome) {
  mkdirSync(targetHome, { recursive: true })

  const copied = []
  const missing = []
  for (const name of CODEX_SCRATCH_FILES) {
    try {
      copyFileSync(join(sourceHome, name), join(targetHome, name))
      copied.push(name)
    } catch {
      missing.push(name)
    }
  }

  return { home: targetHome, copied, missing }
}

/**
 * Set Codex's auto-compaction threshold in a scratch `config.toml`.
 *
 * This is what bounds the automatic case: instead of paying for the ~250k
 * tokens a default context window needs, the run lowers the threshold and
 * reaches Codex's own auto-compaction after a couple of turns. TOML puts
 * top-level keys above the first table, so the key is inserted there.
 */
export function setAutoCompactTokenLimit(configPath, limit) {
  const key = 'model_auto_compact_token_limit'
  const assignment = `${key} = ${limit}`

  let lines = []
  try {
    lines = readFileSync(configPath, 'utf8').split('\n')
  } catch {
    lines = []
  }

  const existing = lines.findIndex((line) => line.trimStart().startsWith(`${key} `) || line.trimStart().startsWith(`${key}=`))
  if (existing >= 0) {
    lines[existing] = assignment
  } else {
    const firstTable = lines.findIndex((line) => line.trimStart().startsWith('['))
    const insertAt = firstTable < 0 ? lines.length : firstTable
    lines.splice(insertAt, 0, assignment)
  }

  const contents = lines.join('\n')
  writeFileSync(configPath, contents.endsWith('\n') ? contents : `${contents}\n`)
}

/**
 * Mark a directory trusted in a scratch `config.toml`.
 *
 * Codex stops at an interactive "Do you trust the contents of this directory?"
 * prompt for any unknown workspace — `--yolo` is an approval policy and does
 * not answer it — so a managed member launched in a fresh fixture project never
 * reaches its first turn. The operator's own projects carry this entry; the
 * scratch home needs it for the fixture path.
 */
export function trustProject(configPath, projectPath) {
  const table = `[projects.${JSON.stringify(projectPath)}]`

  let contents = ''
  try {
    contents = readFileSync(configPath, 'utf8')
  } catch {
    contents = ''
  }
  if (contents.includes(table)) return

  const prefix = contents.length === 0 || contents.endsWith('\n') ? '' : '\n'
  appendFileSync(configPath, `${prefix}\n${table}\ntrust_level = "trusted"\n`)
}
