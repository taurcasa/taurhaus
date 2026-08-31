import { existsSync, readdirSync } from 'node:fs'

/** Inspect the current Codex state database without creating or opening it. */
export function codexStateDatabaseDiagnostic(codexHome) {
  const filenames = existsSync(codexHome)
    ? readdirSync(codexHome, { withFileTypes: true })
        .filter((entry) => entry.isFile() && /^state_\d+\.sqlite$/.test(entry.name))
        .map((entry) => entry.name)
        .sort()
    : []
  return { directory: codexHome, filenames, exists: filenames.length > 0 }
}

/** Preserve the pane's bounded tail when a managed session bind wait fails. */
export async function waitWithPaneTail({ memberName, paneId, wait, capturePane, tailLines = 40 }) {
  try {
    return await wait()
  } catch (error) {
    let captured = ''
    try {
      captured = (await capturePane(paneId)).trimEnd()
    } catch {
      captured = ''
    }
    const tail = captured ? captured.split('\n').slice(-tailLines).join('\n') : '(pane capture empty)'
    throw new Error(
      `${String(error?.message ?? error)}\n${memberName} pane ${paneId} capture tail:\n${tail}`,
      { cause: error }
    )
  }
}

/** Launch the first managed member with its team, then hot-add the rest. */
export async function launchManagedMembersSerially({ members, initialize, add, waitForBinding }) {
  const [first, ...remaining] = members
  if (!first) return

  await initialize(first)
  await waitForBinding(first)

  for (const member of remaining) {
    await add(member)
    await waitForBinding(member)
  }
}
